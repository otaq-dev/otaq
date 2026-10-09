//! The ticket editor (UI-05, TKT-03; d0019; *08 · Ticket editor → Quick-create in Backlog, Full form · filled,
//! Validation · empty form*): Title, Type and Epic (when creating), What and why (Markdown), and the "Done when"
//! checklist (add, edit, reorder, remove). A missing title is the only hard rule; an empty "Done when" is
//! allowed here (the warning comes when starting, TKT-04). Saving an existing ticket sends only what changed,
//! and Done-when edits keep who added each item.

use crewboard_core::board::tickets::{DoneWhenEdit, NewTicketInput, Ticket, TicketUpdate};
use crewboard_core::domain::{Actor, Kind, TicketKey};

/// One criterion while editing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Criterion {
    pub text: String,
    /// The item's index in the saved ticket; `None` for a new one.
    pub origin: Option<usize>,
    /// Added by an agent (shown with a marker, T-0042).
    pub by_agent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorMode {
    /// A new ticket in this project (and epic, when chosen).
    Create {
        project: String,
    },
    Edit {
        key: TicketKey,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorModel {
    pub mode: EditorMode,
    pub title: String,
    pub body: String,
    pub criteria: Vec<Criterion>,
    pub kind: Kind,
    /// Create the ticket inside this epic.
    pub epic: Option<TicketKey>,
    /// Direct parents. For a new ticket they are linked right after it is created.
    pub deps: Vec<TicketKey>,
    /// "Start from and merge into" for standalone tickets and epics; `None` for epic children.
    pub base_branch: Option<String>,
    pub base_locked: bool,
    pub external_id: String,
    /// A refused link, shown under Depends on: (code, the service's message).
    pub dep_error: Option<(Option<String>, String)>,
    /// A refused base change, shown under the base field.
    pub base_error: Option<String>,
    /// Save was tried: show the title error.
    pub tried: bool,
    pub saving: bool,
    pub error: Option<String>,
    original: Option<Ticket>,
}

/// The title error, under the field.
pub const TITLE_REQUIRED: &str = "A title is required";

impl EditorModel {
    /// A new ticket; `base` is the project's base branch (the default).
    pub fn create(project: &str, title: &str, base: &str) -> Self {
        Self {
            mode: EditorMode::Create {
                project: project.to_owned(),
            },
            title: title.to_owned(),
            body: String::new(),
            criteria: Vec::new(),
            kind: Kind::Ticket,
            epic: None,
            deps: Vec::new(),
            base_branch: Some(base.to_owned()),
            base_locked: false,
            external_id: String::new(),
            dep_error: None,
            base_error: None,
            tried: false,
            saving: false,
            error: None,
            original: None,
        }
    }

    pub fn edit(ticket: &Ticket) -> Self {
        Self {
            mode: EditorMode::Edit {
                key: ticket.key.clone(),
            },
            title: ticket.title.clone(),
            body: ticket.body.clone(),
            criteria: ticket
                .done_when
                .iter()
                .enumerate()
                .map(|(i, item)| Criterion {
                    text: item.text.clone(),
                    origin: Some(i),
                    by_agent: matches!(item.added_by, Actor::Agent(_)),
                })
                .collect(),
            kind: ticket.kind,
            epic: ticket.epic.clone(),
            deps: ticket.depends_on.clone(),
            base_branch: match &ticket.base {
                crewboard_core::board::tickets::Base::Branch(b) => Some(b.clone()),
                crewboard_core::board::tickets::Base::Epic(_) => None,
            },
            base_locked: ticket.base_locked,
            external_id: ticket.external_id.clone().unwrap_or_default(),
            dep_error: None,
            base_error: None,
            tried: false,
            saving: false,
            error: None,
            original: Some(ticket.clone()),
        }
    }

    pub fn is_create(&self) -> bool {
        matches!(self.mode, EditorMode::Create { .. })
    }

    pub fn title_error(&self) -> Option<&'static str> {
        (self.tried && self.title.trim().is_empty()).then_some(TITLE_REQUIRED)
    }

    /// Save/Create is enabled once there is a title.
    pub fn can_save(&self) -> bool {
        !self.title.trim().is_empty() && !self.saving
    }

    pub fn add_criterion(&mut self, text: &str) -> bool {
        let text = text.trim();
        if text.is_empty() {
            return false;
        }
        self.criteria.push(Criterion {
            text: text.to_owned(),
            origin: None,
            by_agent: false,
        });
        true
    }

    pub fn edit_criterion(&mut self, index: usize, text: &str) {
        if let Some(c) = self.criteria.get_mut(index) {
            c.text = text.to_owned();
        }
    }

    pub fn remove_criterion(&mut self, index: usize) {
        if index < self.criteria.len() {
            self.criteria.remove(index);
        }
    }

    /// Drag to reorder: the item at `from` lands at `to`.
    pub fn move_criterion(&mut self, from: usize, to: usize) {
        if from < self.criteria.len() && to < self.criteria.len() && from != to {
            let item = self.criteria.remove(from);
            self.criteria.insert(to, item);
        }
    }

    /// The `ticket.create` input.
    pub fn create_input(&self) -> Option<NewTicketInput> {
        let EditorMode::Create { project } = &self.mode else {
            return None;
        };
        Some(NewTicketInput {
            project: project.clone(),
            kind: self.kind,
            title: self.title.trim().to_owned(),
            body: self.body.clone(),
            done_when: self
                .criteria
                .iter()
                .map(|c| c.text.trim().to_owned())
                .filter(|t| !t.is_empty())
                .collect(),
            epic: self.epic.as_ref().map(ToString::to_string),
            base_branch: self
                .epic
                .is_none()
                .then(|| self.base_branch.clone())
                .flatten(),
            external_id: (self.epic.is_none() && !self.external_id.trim().is_empty())
                .then(|| self.external_id.trim().to_owned()),
        })
    }

    /// The `ticket.update` changes against the saved ticket, or `None` when nothing changed.
    pub fn update_input(&self) -> Option<TicketUpdate> {
        let original = self.original.as_ref()?;
        let mut update = TicketUpdate::default();
        if self.title.trim() != original.title {
            update.title = Some(self.title.trim().to_owned());
        }
        if self.body != original.body {
            update.body = Some(self.body.clone());
        }
        if let (Some(new), crewboard_core::board::tickets::Base::Branch(old)) =
            (&self.base_branch, &original.base)
            && new != old
        {
            update.base_branch = Some(new.clone());
        }
        if self.external_id.trim() != original.external_id.as_deref().unwrap_or_default() {
            update.external_id = Some(self.external_id.trim().to_owned());
        }
        update.done_when = done_when_edits(
            &original
                .done_when
                .iter()
                .map(|i| i.text.clone())
                .collect::<Vec<_>>(),
            &self.criteria,
        );
        (update != TicketUpdate::default()).then_some(update)
    }
}

/// Edits that turn the saved list into the edited one, keeping item identity (and so who added each): removals
/// (from the end), text edits, additions, then moves into the final order.
pub fn done_when_edits(saved: &[String], edited: &[Criterion]) -> Vec<DoneWhenEdit> {
    let mut edits = Vec::new();
    // The list as the service will hold it, by origin (`None` = new item, numbered after the saved ones).
    let kept: Vec<usize> = edited.iter().filter_map(|c| c.origin).collect();
    let mut current: Vec<Option<usize>> = (0..saved.len()).map(Some).collect();
    for i in (0..saved.len()).rev() {
        if !kept.contains(&i) {
            edits.push(DoneWhenEdit::Remove { index: i });
            current.remove(i);
        }
    }
    for c in edited {
        if let Some(o) = c.origin
            && saved.get(o).is_some_and(|s| s.trim() != c.text.trim())
            && !c.text.trim().is_empty()
        {
            let index = current.iter().position(|x| *x == Some(o)).expect("kept");
            edits.push(DoneWhenEdit::Edit {
                index,
                text: c.text.trim().to_owned(),
            });
        }
    }
    // New items get identities after the saved ones.
    let mut target: Vec<Option<usize>> = Vec::new();
    let mut next_new = saved.len();
    for c in edited {
        match c.origin {
            Some(o) => target.push(Some(o)),
            None if c.text.trim().is_empty() => {}
            None => {
                edits.push(DoneWhenEdit::Add {
                    text: c.text.trim().to_owned(),
                });
                current.push(Some(next_new));
                target.push(Some(next_new));
                next_new += 1;
            }
        }
    }
    for (i, want) in target.iter().enumerate() {
        let at = current.iter().position(|x| x == want).expect("present");
        if at != i {
            edits.push(DoneWhenEdit::Move { from: at, to: i });
            let item = current.remove(at);
            current.insert(i, item);
        }
    }
    edits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use crewboard_core::board::tickets::DoneWhenItem;
    use crewboard_core::domain::Status;

    /// Applies edits the way the service does, to check the result.
    fn apply(saved: &[&str], edits: &[DoneWhenEdit]) -> Vec<String> {
        let mut list: Vec<String> = saved.iter().map(|s| (*s).to_owned()).collect();
        for e in edits {
            match e {
                DoneWhenEdit::Add { text } => list.push(text.clone()),
                DoneWhenEdit::Edit { index, text } => list[*index] = text.clone(),
                DoneWhenEdit::Move { from, to } => {
                    let item = list.remove(*from);
                    list.insert(*to, item);
                }
                DoneWhenEdit::Remove { index } => {
                    list.remove(*index);
                }
            }
        }
        list
    }

    fn saved_ticket() -> Ticket {
        let mut t = ticket("SHOP-20", "Checkout page", Status::Backlog);
        t.done_when = [
            "Posts the cart",
            "Shows 422 errors inline",
            "Works with an empty cart",
        ]
        .iter()
        .map(|s| DoneWhenItem {
            text: (*s).into(),
            added_by: Actor::User("me".into()),
        })
        .collect();
        t
    }

    #[test]
    fn create_needs_a_title_only() {
        let mut m = EditorModel::create("SHOP", "", "main");
        assert!(!m.can_save());
        assert_eq!(m.title_error(), None, "no error before trying");
        m.tried = true;
        assert_eq!(m.title_error(), Some(TITLE_REQUIRED));
        m.title = "  Add order history page ".into();
        assert!(m.can_save());
        assert!(m.add_criterion("Shows past orders"));
        assert!(!m.add_criterion("   "));
        let input = m.create_input().unwrap();
        assert_eq!(input.title, "Add order history page");
        assert_eq!(input.done_when, ["Shows past orders"]);
        assert_eq!(input.project, "SHOP");
        assert_eq!(input.base_branch.as_deref(), Some("main"));
        assert_eq!(input.external_id, None);
        assert!(m.update_input().is_none());
        // A child of an epic sends neither base nor External ID.
        m.epic = Some("SHOP-40".parse().unwrap());
        m.external_id = "PAY-1".into();
        let input = m.create_input().unwrap();
        assert_eq!((input.base_branch, input.external_id), (None, None));
    }

    #[test]
    fn base_and_external_id_changes() {
        let mut m = EditorModel::edit(&saved_ticket());
        m.base_branch = Some("release/2.3".into());
        m.external_id = " PAY-123 ".into();
        let update = m.update_input().unwrap();
        assert_eq!(update.base_branch.as_deref(), Some("release/2.3"));
        assert_eq!(update.external_id.as_deref(), Some("PAY-123"));
    }

    #[test]
    fn reorder_and_remove() {
        let mut m = EditorModel::edit(&saved_ticket());
        m.move_criterion(2, 0);
        let texts: Vec<_> = m.criteria.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Works with an empty cart",
                "Posts the cart",
                "Shows 422 errors inline"
            ]
        );
        m.remove_criterion(1);
        assert_eq!(m.criteria.len(), 2);
        m.move_criterion(5, 0);
        assert_eq!(m.criteria.len(), 2, "out of range is ignored");
    }

    #[test]
    fn saving_an_edit_sends_only_the_changes_and_reaches_the_edited_list() {
        let saved = saved_ticket();
        let mut m = EditorModel::edit(&saved);
        assert_eq!(m.update_input(), None, "nothing changed");
        m.title = "Checkout page v2".into();
        m.edit_criterion(1, "Shows 422 errors next to the field");
        m.remove_criterion(0);
        m.add_criterion("Redirects to Stripe");
        m.move_criterion(2, 0);
        let update = m.update_input().unwrap();
        assert_eq!(update.title.as_deref(), Some("Checkout page v2"));
        assert_eq!(update.body, None);
        let saved_texts = [
            "Posts the cart",
            "Shows 422 errors inline",
            "Works with an empty cart",
        ];
        let result = apply(&saved_texts, &update.done_when);
        let edited: Vec<_> = m.criteria.iter().map(|c| c.text.clone()).collect();
        assert_eq!(result, edited);
        assert_eq!(
            result,
            [
                "Redirects to Stripe",
                "Shows 422 errors next to the field",
                "Works with an empty cart"
            ]
        );
    }

    #[test]
    fn many_reorders_round_trip() {
        let saved: Vec<String> = (0..5).map(|i| format!("item {i}")).collect();
        let mut criteria: Vec<Criterion> = saved
            .iter()
            .enumerate()
            .map(|(i, t)| Criterion {
                text: t.clone(),
                origin: Some(i),
                by_agent: false,
            })
            .collect();
        criteria.reverse();
        criteria.remove(2);
        criteria.insert(
            1,
            Criterion {
                text: "new".into(),
                origin: None,
                by_agent: false,
            },
        );
        let edits = done_when_edits(&saved, &criteria);
        let refs: Vec<&str> = saved.iter().map(String::as_str).collect();
        let expected: Vec<String> = criteria.iter().map(|c| c.text.clone()).collect();
        assert_eq!(apply(&refs, &edits), expected);
    }
}
