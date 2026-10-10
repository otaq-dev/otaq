//! Approve ▾ and Local merge (GIT-03, REV-08, UI-02; d0015, d0038, d0050, d0058; *07 → Local merge · confirm,
//! success, rebasing, conflict, refused*; *03 → Approve choice on drop*): the choice (Local merge, Open PR from
//! M10 on delivery units only), the confirmation's texts, and one state per Local merge result. Nothing here
//! pushes, and no state offers to stash or discard.

use crewboard_core::board::review::{ApproveCheck, changed_message};
use crewboard_core::board::tickets::{Base, Ticket};
use crewboard_core::domain::{Status, TicketKey};
use crewboard_core::review::LocalMergeOutcome;

/// One choice in the Approve menu ("1 Local merge", "2 Open PR on GitHub").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApproveOption {
    pub choice: ApproveChoice,
    pub title: String,
    pub detail: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApproveChoice {
    LocalMerge,
    OpenPr,
}

/// "Approve SHOP-18 Checkout API", "Approve epic SHOP-40 Wishlist".
pub fn menu_title(ticket: &Ticket) -> String {
    if ticket.is_epic() {
        format!("Approve epic {} {}", ticket.key, ticket.title)
    } else {
        format!("Approve {} {}", ticket.key, ticket.title)
    }
}

/// The line under the choices.
pub fn menu_note(ticket: &Ticket) -> &'static str {
    if ticket.epic.is_some() {
        "Open PR isn\u{2019}t offered for epic children: the epic opens one PR for all of them."
    } else {
        "Both open a confirmation step before anything runs."
    }
}

/// Tickets that depend on `ticket` and still wait for another parent after it merges: "SHOP-20 still waits for
/// SHOP-19".
pub fn still_waiting(
    ticket: &Ticket,
    tickets: &[Ticket],
) -> Vec<(TicketKey, String, Vec<TicketKey>)> {
    let mut out: Vec<(TicketKey, String, Vec<TicketKey>)> = tickets
        .iter()
        .filter(|t| t.depends_on.contains(&ticket.key) && t.status.is_queued())
        .filter_map(|t| {
            let others: Vec<TicketKey> = t
                .depends_on
                .iter()
                .filter(|p| *p != &ticket.key)
                .filter(|p| {
                    tickets
                        .iter()
                        .find(|x| &x.key == *p)
                        .is_none_or(|x| x.status != Status::Merged)
                })
                .cloned()
                .collect();
            (!others.is_empty()).then(|| (t.key.clone(), t.title.clone(), others))
        })
        .collect();
    out.sort();
    out
}

fn keys(keys: &[TicketKey]) -> String {
    keys.iter()
        .map(|k| k.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The menu: Local merge always; Open PR only on delivery units (standalone tickets and epics, never epic
/// children, d0059), shown disabled until M10 (T-0149).
pub fn options(ticket: &Ticket, target: &str, tickets: &[Ticket]) -> Vec<ApproveOption> {
    let (title, mut detail) = match &ticket.base {
        // An epic child squashes back into its epic's branch (*Epic child · Approve offers only Local merge*).
        Base::Epic(epic) => (
            format!("Local merge into {epic}"),
            format!("Squash into {target} \u{b7} local, never pushed"),
        ),
        Base::Branch(_) if ticket.is_epic() => (
            format!("Local merge into {target}"),
            format!("Squash the whole feature into {target} \u{b7} never pushes"),
        ),
        Base::Branch(_) => ("Local merge".to_owned(), format!("Squash into {target}")),
    };
    if let Some((key, _, others)) = still_waiting(ticket, tickets).first() {
        detail.push_str(&format!(" \u{b7} {key} still waits for {}", keys(others)));
    }
    let mut out = vec![ApproveOption {
        choice: ApproveChoice::LocalMerge,
        title,
        detail,
        enabled: true,
    }];
    if ticket.epic.is_none() {
        out.push(ApproveOption {
            choice: ApproveChoice::OpenPr,
            title: "Open PR".to_owned(),
            detail: "Comes in M10".to_owned(),
            enabled: false,
        });
    }
    out
}

/// "Local merge SHOP-18 Checkout API".
pub fn confirm_title(ticket: &Ticket) -> String {
    format!("Local merge {} {}", ticket.key, ticket.title)
}

/// "Into (SHOP-18’s base branch)", or the epic's branch for a child.
pub fn into_label(ticket: &Ticket) -> String {
    match &ticket.base {
        Base::Epic(epic) => format!("Into (the epic {epic}\u{2019}s branch)"),
        Base::Branch(_) => format!("Into ({}\u{2019}s base branch)", ticket.key),
    }
}

/// The note under the message: who keeps waiting, and what happens to the worktree.
pub fn confirm_note(ticket: &Ticket, target: &str, tickets: &[Ticket]) -> String {
    let mut note = String::new();
    for (key, title, others) in still_waiting(ticket, tickets) {
        let both = if others.len() == 1 {
            "both parents are"
        } else {
            "all its parents are"
        };
        note.push_str(&format!(
            "{key} {title} still waits for {} to merge; it starts from {target} once {both} merged. ",
            keys(&others)
        ));
    }
    note.push_str(&format!(
        "The worktree for {} is removed unless it has uncommitted changes.",
        ticket.key
    ));
    note
}

/// The steps the "Merging…" state lists (*Local merge · rebasing onto main…*).
pub fn merging_steps(target: &str, into_epic: bool) -> Vec<String> {
    let mut steps = Vec::new();
    if !into_epic {
        steps.push(format!("Fetch {target} and bring it up to date"));
    }
    steps.push(format!("Rebase onto {target}"));
    steps.push(format!("Squash into {target}"));
    steps.push("Start waiting tickets whose dependencies are all merged".to_owned());
    steps
}

/// What the panel shows after the click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeView {
    /// Done: the toast's title and text, and the started-tickets toast if any.
    Merged {
        title: String,
        text: String,
        started: Option<String>,
    },
    Conflict {
        title: String,
        text: String,
        files: Vec<String>,
    },
    /// "shop-web has uncommitted changes on main. Clean up and retry." (Retry).
    DirtyCheckout { message: String },
    /// The base diverged from its remote (Retry).
    Diverged { message: String },
    /// "SHOP-19 has 2 uncommitted files" · View · Send back.
    Uncommitted { message: String, files: Vec<String> },
    /// "SHOP-18 changed since you reviewed it · 1 new commit": the new commits (short sha, subject) and the
    /// files they changed · Approve anyway / Send back (REV-06).
    Changed {
        title: String,
        commits: Vec<(String, String)>,
        files: Vec<String>,
        /// The reviewed head: the new commits' diff starts there.
        reviewed: String,
    },
}

pub fn short(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// "2 waiting tickets started".
pub fn started_label(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("1 waiting ticket started".to_owned()),
        n => Some(format!("{n} waiting tickets started")),
    }
}

pub fn view(key: &TicketKey, outcome: &LocalMergeOutcome) -> MergeView {
    match outcome {
        LocalMergeOutcome::Merged {
            target,
            commit,
            started,
            ..
        } => MergeView::Merged {
            title: format!("Local merge done \u{b7} {key}"),
            text: format!("Squashed into {target} as {}", short(commit)),
            started: started_label(started.len()),
        },
        LocalMergeOutcome::Conflict { target, files } => MergeView::Conflict {
            title: format!("Rebase onto {target} stopped \u{2014} conflict"),
            text: format!(
                "{target} moved since {key} was built. The rebase onto the latest {target} conflicts in {}. It \
                 was aborted; {target} and your own checkout are unchanged.",
                match files.len() {
                    1 => "1 file".to_owned(),
                    n => format!("{n} files"),
                }
            ),
            files: files.clone(),
        },
        LocalMergeOutcome::RefusedDirtyCheckout { message } => MergeView::DirtyCheckout {
            message: message.clone(),
        },
        LocalMergeOutcome::RefusedDiverged { message } => MergeView::Diverged {
            message: message.clone(),
        },
        LocalMergeOutcome::RefusedUncommitted { files, message } => MergeView::Uncommitted {
            message: message.clone(),
            files: files.clone(),
        },
        LocalMergeOutcome::Changed { check } => changed_view(key, check),
    }
}

/// The changed-since-review state from the check (REV-06, d0043).
pub fn changed_view(key: &TicketKey, check: &ApproveCheck) -> MergeView {
    match check {
        ApproveCheck::Unchanged => MergeView::Changed {
            title: String::new(),
            commits: Vec::new(),
            files: Vec::new(),
            reviewed: String::new(),
        },
        ApproveCheck::Changed {
            new_commits,
            files,
            reviewed,
            ..
        } => MergeView::Changed {
            reviewed: reviewed.clone(),
            title: changed_message(&key.to_string(), new_commits.len()),
            commits: new_commits
                .iter()
                .map(|c| (short(&c.sha), c.subject.clone()))
                .collect(),
            files: files.iter().map(|f| f.path.clone()).collect(),
        },
    }
}

/// The panel header's strip: "Conflicts with main · 2 files (src/lib/stripe/session.ts, package.json)".
pub fn conflict_line(conflict: &crewboard_core::board::conflicts::Conflict) -> String {
    let mut line = format!("Conflicts with {}", conflict.target);
    match conflict.files.len() {
        0 => {}
        1 => line.push_str(&format!(" \u{b7} 1 file ({})", conflict.files[0])),
        n => line.push_str(&format!(
            " \u{b7} {n} files ({})",
            conflict.files.join(", ")
        )),
    }
    line
}

/// Send to agent to fix is offered on a ticket in Review whose Local merge or test merge conflicted.
pub fn offers_fix(
    ticket: &Ticket,
    conflict: Option<&crewboard_core::board::conflicts::Conflict>,
) -> bool {
    ticket.status == Status::ReadyForReview
        && conflict
            .is_some_and(|c| c.reason == crewboard_core::board::conflicts::ConflictReason::Merge)
}

/// The conflict's way forward: Send to agent to fix, or by hand.
pub fn conflict_help(key: &TicketKey, target: &str) -> String {
    format!(
        "Send to agent to fix: {key} returns to Dev and its agent rebases onto {target}, resolves the conflict and \
         submits again. Or resolve it yourself in the worktree and approve again."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;

    fn sample() -> (Ticket, Vec<Ticket>) {
        let shop18 = ticket("SHOP-18", "Checkout API", Status::ReadyForReview);
        let shop19 = ticket("SHOP-19", "Price table", Status::Working);
        let mut shop20 = ticket("SHOP-20", "Checkout page", Status::Queued);
        shop20.depends_on = vec![shop18.key.clone(), shop19.key.clone()];
        (shop18.clone(), vec![shop18, shop19, shop20])
    }

    #[test]
    fn the_menu_offers_local_merge_and_open_pr_for_m10() {
        let (shop18, tickets) = sample();
        let options = options(&shop18, "main", &tickets);
        assert_eq!(menu_title(&shop18), "Approve SHOP-18 Checkout API");
        assert_eq!(options[0].choice, ApproveChoice::LocalMerge);
        assert_eq!(
            options[0].detail,
            "Squash into main \u{b7} SHOP-20 still waits for SHOP-19"
        );
        assert!(options[0].enabled);
        assert_eq!(options[1].choice, ApproveChoice::OpenPr);
        assert!(!options[1].enabled);
        let mut child = ticket("SHOP-40.1", "Wishlist API", Status::ReadyForReview);
        child.epic = Some("SHOP-40".parse().unwrap());
        child.base = Base::Epic("SHOP-40".parse().unwrap());
        let epic_branch = crewboard_core::naming::branch("SHOP-40");
        let only = super::options(&child, &epic_branch, &[]);
        assert_eq!(only.len(), 1, "epic children only merge locally");
        assert_eq!(only[0].title, "Local merge into SHOP-40");
        assert_eq!(
            only[0].detail,
            format!("Squash into {epic_branch} \u{b7} local, never pushed")
        );
        assert!(menu_note(&child).starts_with("Open PR isn"));
        let mut epic = ticket("SHOP-40", "Wishlist", Status::ReadyForReview);
        epic.kind = crewboard_core::domain::Kind::Epic;
        assert_eq!(menu_title(&epic), "Approve epic SHOP-40 Wishlist");
        let both = super::options(&epic, "main", &[]);
        assert_eq!(both[0].title, "Local merge into main");
        assert_eq!(
            both[0].detail,
            "Squash the whole feature into main \u{b7} never pushes"
        );
        assert_eq!(
            both[1].choice,
            ApproveChoice::OpenPr,
            "the epic offers Open PR (M10)"
        );
        assert_eq!(
            into_label(&child),
            "Into (the epic SHOP-40\u{2019}s branch)"
        );
    }

    #[test]
    fn the_confirmation_names_the_target_and_who_keeps_waiting() {
        let (shop18, tickets) = sample();
        assert_eq!(confirm_title(&shop18), "Local merge SHOP-18 Checkout API");
        assert_eq!(into_label(&shop18), "Into (SHOP-18\u{2019}s base branch)");
        assert_eq!(
            confirm_note(&shop18, "main", &tickets),
            "SHOP-20 Checkout page still waits for SHOP-19 to merge; it starts from main once both parents are \
             merged. The worktree for SHOP-18 is removed unless it has uncommitted changes."
        );
        assert_eq!(merging_steps("main", false).len(), 4);
        assert_eq!(merging_steps("epic-branch", true).len(), 3, "no fetch");
    }

    #[test]
    fn the_conflict_strip_names_the_files() {
        use crewboard_core::board::conflicts::{Conflict, ConflictReason};
        let c = Conflict {
            target: "main".into(),
            files: vec![
                "src/lib/stripe/session.ts".into(),
                "src/lib/cart/validate.ts".into(),
            ],
            reason: ConflictReason::Merge,
        };
        assert_eq!(
            conflict_line(&c),
            "Conflicts with main \u{b7} 2 files (src/lib/stripe/session.ts, src/lib/cart/validate.ts)"
        );
        let review = ticket("SHOP-18", "Checkout API", Status::ReadyForReview);
        assert!(offers_fix(&review, Some(&c)));
        assert!(!offers_fix(&review, None));
        let reopened = Conflict {
            reason: ConflictReason::Reopen,
            ..c.clone()
        };
        assert!(
            !offers_fix(&review, Some(&reopened)),
            "the agent resolves it first"
        );
        let working = ticket("SHOP-18", "Checkout API", Status::Working);
        assert!(!offers_fix(&working, Some(&c)));
    }

    #[test]
    fn every_result_has_its_state() {
        let key: TicketKey = "SHOP-18".parse().unwrap();
        let t = Box::new(ticket("SHOP-18", "Checkout API", Status::Merged));
        assert_eq!(
            view(
                &key,
                &LocalMergeOutcome::Merged {
                    ticket: t,
                    target: "main".into(),
                    commit: "9f3e2c1aaaa".into(),
                    started: vec!["SHOP-20".parse().unwrap(), "SHOP-21".parse().unwrap()],
                }
            ),
            MergeView::Merged {
                title: "Local merge done \u{b7} SHOP-18".into(),
                text: "Squashed into main as 9f3e2c1".into(),
                started: Some("2 waiting tickets started".into()),
            }
        );
        let MergeView::Conflict { title, text, files } = view(
            &key,
            &LocalMergeOutcome::Conflict {
                target: "main".into(),
                files: vec!["package.json".into(), "src/lib/stripe/session.ts".into()],
            },
        ) else {
            panic!()
        };
        assert_eq!(title, "Rebase onto main stopped \u{2014} conflict");
        assert!(text.contains("conflicts in 2 files"));
        assert_eq!(files.len(), 2);
        assert!(matches!(
            view(
                &key,
                &LocalMergeOutcome::RefusedDirtyCheckout {
                    message: "m".into()
                }
            ),
            MergeView::DirtyCheckout { .. }
        ));
        assert!(matches!(
            view(
                &key,
                &LocalMergeOutcome::RefusedDiverged {
                    message: "m".into()
                }
            ),
            MergeView::Diverged { .. }
        ));
        assert!(matches!(
            view(
                &key,
                &LocalMergeOutcome::RefusedUncommitted {
                    files: vec![],
                    message: "SHOP-18 has 2 uncommitted files".into()
                }
            ),
            MergeView::Uncommitted { .. }
        ));
        assert_eq!(started_label(0), None);
        assert_eq!(
            started_label(1).as_deref(),
            Some("1 waiting ticket started")
        );
    }
}
