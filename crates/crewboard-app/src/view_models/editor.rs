//! The ticket editor (UI-05, TKT-03; d0019; *08 · Ticket editor → Quick-create in Backlog, Full form · filled,
//! Validation · empty form*): Title, Type and Epic (when creating) and the description (Markdown). A missing
//! title is the only hard rule; a short or empty description is fine (d0067). Saving an existing ticket sends
//! only what changed.

use crewboard_core::board::tickets::{NewTicketInput, Ticket, TicketUpdate};
use crewboard_core::domain::{Kind, TicketKey};

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
        (update != TicketUpdate::default()).then_some(update)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use crewboard_core::domain::Status;

    fn saved_ticket() -> Ticket {
        let mut t = ticket("SHOP-20", "Checkout page", Status::Backlog);
        t.body = "Post the cart; show 422 errors inline.".into();
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
        assert!(m.can_save(), "an empty description is fine");
        m.body = "Shows past orders".into();
        let input = m.create_input().unwrap();
        assert_eq!(input.title, "Add order history page");
        assert_eq!(input.body, "Shows past orders");
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

    /// The editor's payloads, sent to a real background service (temporary data folder), come back with their
    /// title and description: create, then an edit that changes both.
    #[test]
    fn title_and_description_round_trip_through_the_service() {
        use crewboard_core::client::{Client, ClientConfig};
        use crewboard_core::protocol::{KeyParams, Reply, Request, TicketUpdateParams};
        use crewboard_core::service::testing::TestService;

        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let w = TestService::start().await;
            w.add_project("shop-web").await;
            let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
            let get = async |key: &str| match client
                .request(Request::TicketGet(KeyParams { key: key.into() }))
                .await
                .unwrap()
            {
                Reply::Ticket(t) => *t,
                other => panic!("{other:?}"),
            };

            let mut m = EditorModel::create("SHOP", "Checkout page", "main");
            m.body = "Post the cart.".into();
            let Reply::Ticket(created) = client
                .request(Request::TicketCreate(m.create_input().unwrap()))
                .await
                .unwrap()
            else {
                panic!()
            };
            let stored = get(&created.key.to_string()).await;
            assert_eq!(
                (stored.title.as_str(), stored.body.as_str()),
                ("Checkout page", "Post the cart.")
            );

            // The Ticket tab: Edit, change both, Save.
            let mut m = EditorModel::edit(&stored);
            m.title = "Checkout page v2".into();
            m.body = "Post the cart.\n\nShow 422 errors inline.".into();
            client
                .request(Request::TicketUpdate(TicketUpdateParams {
                    key: stored.key.to_string(),
                    update: m.update_input().unwrap(),
                }))
                .await
                .unwrap();
            let stored = get(&stored.key.to_string()).await;
            assert_eq!(
                (stored.title.as_str(), stored.body.as_str()),
                (
                    "Checkout page v2",
                    "Post the cart.\n\nShow 422 errors inline."
                )
            );
            assert_eq!(EditorModel::edit(&stored).update_input(), None);
        });
    }

    #[test]
    fn typed_external_id_is_sent() {
        let mut m = EditorModel::create("SHOP", "Checkout page", "main");
        m.external_id = "PAY-123".into();
        assert_eq!(
            m.create_input().unwrap().external_id.as_deref(),
            Some("PAY-123")
        );
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
    fn saving_an_edit_sends_only_the_changes() {
        let saved = saved_ticket();
        let mut m = EditorModel::edit(&saved);
        assert_eq!(m.update_input(), None, "nothing changed");
        m.title = "Checkout page v2".into();
        let update = m.update_input().unwrap();
        assert_eq!(update.title.as_deref(), Some("Checkout page v2"));
        assert_eq!(update.body, None);
        m.body = "Post the cart.\n\n## Done when\n- Works with an empty cart".into();
        let update = m.update_input().unwrap();
        assert_eq!(update.body.as_deref(), Some(m.body.as_str()));
        assert_eq!((update.base_branch, update.external_id), (None, None));
    }
}
