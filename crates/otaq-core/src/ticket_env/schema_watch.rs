//! The schema-path warning (M19-e, d0087): two tickets in Review that both **add** files under one of the
//! project's schema paths (`db/migrations/**`, `prisma/schema.prisma`) are warned about each other, because the
//! second to merge usually needs its migration renumbered or regenerated.
//!
//! - Only tickets in [`FREED_IN`] are scanned, and only when the project names schema paths: with none, no `git`
//!   runs at all.
//! - A ticket's added files are `git diff --diff-filter=A <base_sha>...<branch>` in the project's repository,
//!   limited to the schema paths. "Same path" means the same schema-path glob.
//! - The overlaps are stored on the ticket (`tickets.schema_overlap`, shown through `Work.schema_overlap`). A new
//!   overlap is also recorded in the ticket's history ([`EventKind::SchemaOverlap`]); one that is gone is cleared.
//! - Files are never touched: this only reads git.
//!
//! [`TicketEnv::run`](super::TicketEnv::run) scans a project again when one of its tickets enters or leaves
//! [`FREED_IN`], and the sweep scans every project.

use std::collections::HashMap;
use std::path::Path;

use serde_json::json;

use crate::board::BoardError;
use crate::board::tickets::{EventKind, Ticket, record_event};
use crate::domain::{Actor, SchemaOverlap, TicketKey};
use crate::git::Git;
use crate::protocol::Event;
use crate::store::projects::Project;
use crate::ticket_env::TicketEnv;
use crate::ticket_env::free::FREED_IN;

/// What each scanned ticket adds: per schema-path glob, the added files under it (sorted).
pub(crate) type Added = Vec<(TicketKey, Vec<(String, Vec<String>)>)>;

/// Pure: given each Review ticket's added files per glob, the overlaps per ticket: for every other ticket that adds
/// a file under the same glob, one [`SchemaOverlap`] naming it, the glob and the other's files. Sorted by other
/// ticket, then glob. A ticket with no overlap is absent.
pub(crate) fn overlaps(added: &Added) -> HashMap<TicketKey, Vec<SchemaOverlap>> {
    let mut found: HashMap<TicketKey, Vec<SchemaOverlap>> = HashMap::new();
    for (key, globs) in added {
        for (other, other_globs) in added {
            if other == key {
                continue;
            }
            for (glob, files) in globs {
                if files.is_empty() {
                    continue;
                }
                let theirs = other_globs.iter().find(|(g, f)| g == glob && !f.is_empty());
                if let Some((_, theirs)) = theirs {
                    found.entry(key.clone()).or_default().push(SchemaOverlap {
                        other: other.clone(),
                        path: glob.clone(),
                        files: theirs.clone(),
                    });
                }
            }
        }
    }
    for list in found.values_mut() {
        list.sort_by(|a, b| (&a.other, &a.path).cmp(&(&b.other, &b.path)));
        list.dedup_by(|a, b| a.other == b.other && a.path == b.path);
    }
    found
}

/// The project's schema paths that can be used: trimmed, non-empty and well formed ([`crate::glob::problem`]).
fn usable(paths: &[String]) -> Vec<String> {
    let mut globs: Vec<String> = paths
        .iter()
        .map(|p| p.trim().to_owned())
        .filter(|p| crate::glob::problem(p).is_none())
        .collect();
    globs.sort();
    globs.dedup();
    globs
}

/// Whether `file` falls under `glob`: it matches, or one of its folders does (`db/migrations` names the folder's
/// files, as the `git diff` pathspec does).
fn under(glob: &str, file: &str) -> bool {
    let mut at = Some(file);
    while let Some(path) = at {
        if crate::glob::matches(glob, path) {
            return true;
        }
        at = path.rfind('/').map(|i| &path[..i]);
    }
    false
}

/// Git: the files `ticket`'s branch adds since its `base_sha` under `globs`, per glob (one `git diff` in the
/// project's repository). Empty for a ticket without a branch or base, or when git fails (logged).
pub(crate) async fn added_files(
    git: &Git,
    project: &Project,
    ticket: &Ticket,
    globs: &[String],
) -> Vec<(String, Vec<String>)> {
    let Some((branch, base)) = ticket
        .work
        .as_ref()
        .and_then(|w| Some((w.branch.as_deref()?, w.base_sha.as_deref()?)))
    else {
        return Vec::new();
    };
    let range = format!("{base}...{branch}");
    let pathspecs: Vec<String> = globs.iter().map(|g| format!(":(glob){g}")).collect();
    let mut args = vec![
        "diff",
        "--name-only",
        "-z",
        "--no-renames",
        "--diff-filter=A",
        range.as_str(),
        "--",
    ];
    args.extend(pathspecs.iter().map(String::as_str));
    let out = match git.run(Path::new(&project.repo_path), &args).await {
        Ok(out) => out,
        Err(e) => {
            tracing::debug!(ticket = %ticket.key, error = %e, "can't read the files the ticket adds");
            return Vec::new();
        }
    };
    let mut files: Vec<&str> = out.split('\0').filter(|f| !f.is_empty()).collect();
    files.sort_unstable();
    globs
        .iter()
        .map(|glob| {
            let under = files
                .iter()
                .filter(|f| under(glob, f))
                .map(|f| (*f).to_owned())
                .collect();
            (glob.clone(), under)
        })
        .collect()
}

/// Scans `project`'s Review tickets again and stores what changed: the new overlaps (each recorded in the
/// ticket's history), and the ones that are gone (cleared). Clients hear of every ticket whose warning changed.
pub(crate) async fn rescan(env: &TicketEnv, project: &Project) -> Result<(), BoardError> {
    let board = env.board();
    let globs = usable(&board.project_settings(&project.key).await?.schema_paths);
    let tickets = board.tickets(&project.key).await?;
    let mut added: Added = Vec::new();
    if !globs.is_empty() {
        let git = board.git()?;
        for ticket in tickets.iter().filter(|t| FREED_IN.contains(&t.status)) {
            env.counted_schema_scan();
            added.push((
                ticket.key.clone(),
                added_files(git, project, ticket, &globs).await,
            ));
        }
    }
    let mut found = overlaps(&added);
    for ticket in &tickets {
        let now = found.remove(&ticket.key).unwrap_or_default();
        let before = ticket
            .work
            .as_ref()
            .map(|w| w.schema_overlap.clone())
            .unwrap_or_default();
        if now == before {
            continue;
        }
        store(env, ticket, &before, &now).await?;
    }
    Ok(())
}

/// Stores `ticket`'s overlaps `now` (they were `before`) and records each new one.
async fn store(
    env: &TicketEnv,
    ticket: &Ticket,
    before: &[SchemaOverlap],
    now: &[SchemaOverlap],
) -> Result<(), BoardError> {
    let board = env.board();
    let json = if now.is_empty() {
        None
    } else {
        Some(serde_json::to_string(now).expect("overlaps serialise"))
    };
    let mut tx = board.store().begin_write().await?;
    sqlx::query("UPDATE tickets SET schema_overlap = ? WHERE id = ?")
        .bind(json.as_deref())
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
    for new in now.iter().filter(|n| {
        !before
            .iter()
            .any(|b| b.other == n.other && b.path == n.path)
    }) {
        tracing::info!(ticket = %ticket.key, other = %new.other, path = %new.path, "adds files under a schema path another Review ticket adds to");
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::SchemaOverlap,
            &Actor::current_user(),
            json!({ "other": new.other.to_string(), "path": new.path, "files": new.files }),
        )
        .await?;
    }
    tx.commit().await?;
    env.events().publish(Event::TicketChanged {
        project: ticket.key.project.to_string(),
        id: ticket.id.clone(),
        key: ticket.key.clone(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: &str) -> TicketKey {
        k.parse().unwrap()
    }

    fn adds(k: &str, globs: &[(&str, &[&str])]) -> (TicketKey, Vec<(String, Vec<String>)>) {
        (
            key(k),
            globs
                .iter()
                .map(|(g, files)| {
                    (
                        (*g).to_owned(),
                        files.iter().map(|f| (*f).to_owned()).collect(),
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn two_tickets_adding_under_one_glob_overlap_and_others_dont() {
        let migrations = "db/migrations/**";
        let prisma = "prisma/schema.prisma";
        let added = vec![
            adds(
                "SHOP-18",
                &[(migrations, &["db/migrations/0042_a.sql"]), (prisma, &[])],
            ),
            adds(
                "SHOP-21",
                &[
                    (migrations, &["db/migrations/0042_b.sql"]),
                    (prisma, &["prisma/schema.prisma"]),
                ],
            ),
            // Adds only under another glob: no overlap with either.
            adds("SHOP-22", &[(migrations, &[]), (prisma, &[])]),
            adds("SHOP-23", &[(prisma, &["prisma/schema.prisma"])]),
        ];
        let found = overlaps(&added);
        assert_eq!(
            found[&key("SHOP-18")],
            vec![SchemaOverlap {
                other: key("SHOP-21"),
                path: migrations.into(),
                files: vec!["db/migrations/0042_b.sql".into()],
            }]
        );
        assert_eq!(
            found[&key("SHOP-21")],
            vec![
                SchemaOverlap {
                    other: key("SHOP-18"),
                    path: migrations.into(),
                    files: vec!["db/migrations/0042_a.sql".into()],
                },
                SchemaOverlap {
                    other: key("SHOP-23"),
                    path: prisma.into(),
                    files: vec!["prisma/schema.prisma".into()],
                },
            ]
        );
        assert_eq!(
            found[&key("SHOP-23")]
                .iter()
                .map(|o| o.other.to_string())
                .collect::<Vec<_>>(),
            ["SHOP-21"]
        );
        assert!(!found.contains_key(&key("SHOP-22")));
        assert!(
            overlaps(&vec![adds(
                "SHOP-18",
                &[(migrations, &["db/migrations/1.sql"])]
            )])
            .is_empty()
        );
    }

    #[test]
    fn a_folder_schema_path_covers_the_files_inside_it() {
        assert!(under("db/migrations/**", "db/migrations/0042_a.sql"));
        assert!(under("db/migrations", "db/migrations/0042_a.sql"));
        assert!(under("db/migrations/", "db/migrations/sub/0042_a.sql"));
        assert!(under("prisma/schema.prisma", "prisma/schema.prisma"));
        assert!(under("db/*", "db/migrations/0042_a.sql"));
        assert!(!under("db/migrations", "db/migrations_old/0042_a.sql"));
        assert!(!under("db/migrations", "db/seed.sql"));
        assert!(!under("db/*.sql", "db/migrations/0042_a.sql"));
    }

    #[test]
    fn only_well_formed_schema_paths_are_scanned() {
        assert_eq!(
            usable(&[
                " db/migrations/** ".into(),
                String::new(),
                "/abs/**".into(),
                "db/migrations/**".into(),
                "prisma/schema.prisma".into(),
            ]),
            ["db/migrations/**", "prisma/schema.prisma"]
        );
    }
}
