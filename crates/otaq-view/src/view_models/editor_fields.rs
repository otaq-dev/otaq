//! The editor's remaining fields (UI-05, DEP-02, DEP-04, TKT-03; d0046, d0051, d0059; *08 · Ticket editor → Row ·
//! base branch per ticket, Row · epics and base branches*): which fields a ticket kind shows, the base-branch
//! choices, and how the service's refusals read inline. Dependencies are the sheet's (DPV-04, `dependencies.rs`).

use otaq_core::board::tickets::Base;
use otaq_core::domain::{Kind, TicketKey};

use crate::icons::IconName;
use crate::view_models::menu::MenuItem;

/// The "Start from and merge into" field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseField {
    /// A dropdown of local and remote branches.
    Editable,
    /// Fixed: the ticket (or for an epic, a child) has started.
    Locked(String),
    /// Epic children: "From the epic's branch (SHOP-40)".
    FromEpic(TicketKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldVisibility {
    pub base: BaseField,
    pub external_id: bool,
}

/// Which fields a ticket shows: standalone tickets and epics have a base and an External ID; epic children
/// have neither (they use the epic's branch, and the epic carries the ID).
pub fn visibility(kind: Kind, epic: Option<&TicketKey>, locked: bool) -> FieldVisibility {
    match epic {
        Some(epic) => FieldVisibility {
            base: BaseField::FromEpic(epic.clone()),
            external_id: false,
        },
        None => FieldVisibility {
            base: if locked {
                BaseField::Locked(match kind {
                    Kind::Epic => "locked · a child has started".into(),
                    Kind::Ticket => "locked · the ticket has started".into(),
                })
            } else {
                BaseField::Editable
            },
            external_id: true,
        },
    }
}

/// The base dropdown's choices: local branches (the project default marked), then remote ones without the
/// remote's name (`origin/release/2.2` → `release/2.2`) unless a local branch has that name.
pub fn base_choices(local: &[String], remote: &[String]) -> (Vec<String>, Vec<(String, String)>) {
    let remote = remote
        .iter()
        .filter_map(|r| {
            let (_, name) = r.split_once('/')?;
            (!local.iter().any(|l| l == name)).then(|| (r.clone(), name.to_owned()))
        })
        .collect();
    (local.to_vec(), remote)
}

/// A base-branch dropdown's rows (*Settings*, *Add project*): LOCAL branches, then REMOTE ones by their short name
/// (id `origin/release/2.2`), `current` checked.
pub fn branch_items(local: &[String], remote: &[String], current: &str) -> Vec<MenuItem> {
    let (local, remote) = base_choices(local, remote);
    let mut items = vec![MenuItem::header("LOCAL")];
    for b in local {
        let checked = b == current;
        items.push(
            MenuItem::new(b.clone(), b)
                .icon(IconName::GitBranch)
                .checked(checked),
        );
    }
    if !remote.is_empty() {
        items.push(MenuItem::header("REMOTE"));
        for (full, name) in remote {
            let checked = full == current || name == current;
            items.push(
                MenuItem::new(full, name)
                    .icon(IconName::GitBranch)
                    .checked(checked),
            );
        }
    }
    items
}

/// How a refusal reads in the editor: a title by its code, then the service's exact message.
pub fn refusal_title(code: Option<&str>) -> &'static str {
    match code {
        Some("cycle") => "That would create a loop",
        Some("cross_base") => "Different base branches",
        Some("other_project") => "Another project",
        Some("depends_on_own_epic") => "Part of that epic",
        Some("base_locked") => "The base is locked",
        _ => "Couldn’t save",
    }
}

/// The refusal's icon: a loop for a cycle (*08 → Cycle error*), otherwise the error circle.
pub fn refusal_icon(code: Option<&str>) -> IconName {
    match code {
        Some("cycle") => IconName::RefreshCcw,
        _ => IconName::CircleX,
    }
}

/// The base a ticket shows ("main", or the epic's).
pub fn base_label(base: &Base) -> String {
    match base {
        Base::Branch(b) => b.clone(),
        Base::Epic(e) => e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: &str) -> TicketKey {
        k.parse().unwrap()
    }

    #[test]
    fn fields_per_kind() {
        let standalone = visibility(Kind::Ticket, None, false);
        assert_eq!(standalone.base, BaseField::Editable);
        assert!(standalone.external_id);
        let started = visibility(Kind::Ticket, None, true);
        assert!(matches!(started.base, BaseField::Locked(_)));
        let epic = visibility(Kind::Epic, None, true);
        assert_eq!(
            epic.base,
            BaseField::Locked("locked · a child has started".into())
        );
        assert!(epic.external_id);
        let child = visibility(Kind::Ticket, Some(&key("SHOP-40")), false);
        assert_eq!(child.base, BaseField::FromEpic(key("SHOP-40")));
        assert!(!child.external_id);
    }

    #[test]
    fn base_choices_and_errors() {
        let local = vec!["main".to_owned(), "release/2.3".to_owned()];
        let remote = vec![
            "origin/main".to_owned(),
            "origin/release/2.2".to_owned(),
            "origin/next".to_owned(),
        ];
        let (l, r) = base_choices(&local, &remote);
        assert_eq!(l, local);
        assert_eq!(
            r,
            [
                ("origin/release/2.2".to_owned(), "release/2.2".to_owned()),
                ("origin/next".to_owned(), "next".to_owned())
            ]
        );
        let items = branch_items(&local, &remote, "origin/next");
        let rows: Vec<_> = items
            .iter()
            .map(|i| (i.header, i.id.as_str(), i.label.as_str(), i.checked))
            .collect();
        assert_eq!(
            rows,
            [
                (true, "", "LOCAL", false),
                (false, "main", "main", false),
                (false, "release/2.3", "release/2.3", false),
                (true, "", "REMOTE", false),
                (false, "origin/release/2.2", "release/2.2", false),
                (false, "origin/next", "next", true),
            ]
        );
        assert_eq!(refusal_title(Some("cycle")), "That would create a loop");
        assert_eq!(refusal_icon(Some("cycle")), IconName::RefreshCcw);
        assert_eq!(refusal_icon(Some("cross_base")), IconName::CircleX);
        assert_eq!(refusal_title(Some("cross_base")), "Different base branches");
        assert_eq!(base_label(&Base::Epic(key("SHOP-40"))), "SHOP-40");
    }
}
