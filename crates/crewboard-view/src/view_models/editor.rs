//! The ticket editor (UI-05, TKT-03; d0019; *08 · Ticket editor → Quick-create in Backlog, Full form · filled,
//! Validation · empty form*): Title, Priority, Type and Epic (when creating) and the description (Markdown). A
//! missing title is the only hard rule; a short or empty description is fine (d0067). Saving an existing ticket
//! sends only what changed. Priority is sent only once picked (TKT-09, d0065): a new ticket is Normal unless
//! picked, and an epic child shows and follows its epic's until it gets its own.

use crewboard_core::board::tickets::{NewTicketInput, Ticket, TicketUpdate};
use crewboard_core::domain::{Kind, Priority, TicketKey};

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
    /// The ticket's own priority, picked or saved; `None` until picked (a new ticket is then Normal, an epic
    /// child follows its epic).
    pub priority: Option<Priority>,
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
            priority: None,
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
            priority: (!ticket.priority_inherited).then_some(ticket.priority),
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

    /// The priority the form shows: the ticket's own, else (an epic child) the epic's `epic_priority` or the one
    /// it was saved following, else Normal.
    pub fn shown_priority(&self, epic_priority: Option<Priority>) -> Priority {
        self.priority
            .or_else(|| self.epic.as_ref().and(epic_priority))
            .or_else(|| self.original.as_ref().map(|t| t.priority))
            .unwrap_or_default()
    }

    /// An epic child without a priority of its own: it follows its epic.
    pub fn follows_epic(&self) -> Option<&TicketKey> {
        self.epic.as_ref().filter(|_| self.priority.is_none())
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
            // Only when picked: the service makes the rest Normal, and a child follows its epic.
            priority: self.priority,
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
        let own = (!original.priority_inherited).then_some(original.priority);
        if self.priority.is_some() && self.priority != own {
            update.priority = self.priority;
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

    /// Quick-create and the full form: Normal unless picked; a child follows its epic until it picks one.
    #[test]
    fn priority_is_sent_only_once_picked() {
        let mut m = EditorModel::create("SHOP", "Checkout page", "main");
        assert_eq!(m.shown_priority(None), Priority::Normal);
        assert_eq!(
            m.create_input().unwrap().priority,
            None,
            "the service's Normal"
        );
        m.priority = Some(Priority::High);
        assert_eq!(m.create_input().unwrap().priority, Some(Priority::High));

        let mut child = EditorModel::create("SHOP", "Wishlist button", "main");
        child.epic = Some("SHOP-40".parse().unwrap());
        assert_eq!(
            child.shown_priority(Some(Priority::Urgent)),
            Priority::Urgent
        );
        assert_eq!(
            child.follows_epic().map(ToString::to_string).as_deref(),
            Some("SHOP-40")
        );
        assert_eq!(
            child.create_input().unwrap().priority,
            None,
            "follows the epic"
        );
        child.priority = Some(Priority::Low);
        assert_eq!(child.shown_priority(Some(Priority::Urgent)), Priority::Low);
        assert_eq!(child.follows_epic(), None);
        assert_eq!(child.create_input().unwrap().priority, Some(Priority::Low));
    }

    /// The edit form and the Ticket tab: an inherited priority shows until the child gets its own; only a change
    /// is sent, and picking the inherited value gives the child its own.
    #[test]
    fn priority_changes_are_sent_with_the_update() {
        let mut saved = saved_ticket();
        saved.priority = Priority::High;
        let mut m = EditorModel::edit(&saved);
        assert_eq!(m.priority, Some(Priority::High));
        m.priority = Some(Priority::High);
        assert_eq!(m.update_input(), None, "the same priority is no change");
        m.priority = Some(Priority::Urgent);
        assert_eq!(m.update_input().unwrap().priority, Some(Priority::Urgent));

        let mut child = ticket("SHOP-40.2", "Wishlist button", Status::Backlog);
        child.epic = Some("SHOP-40".parse().unwrap());
        child.base = crewboard_core::board::tickets::Base::Epic("SHOP-40".parse().unwrap());
        child.priority = Priority::High;
        child.priority_inherited = true;
        let mut m = EditorModel::edit(&child);
        assert_eq!(m.priority, None);
        assert_eq!(
            m.shown_priority(None),
            Priority::High,
            "the epic's, as saved"
        );
        assert!(m.follows_epic().is_some());
        assert_eq!(m.update_input(), None);
        m.priority = Some(Priority::High);
        assert_eq!(
            m.update_input().unwrap().priority,
            Some(Priority::High),
            "its own now, even if the same"
        );
    }

    /// Quick-create's and the full form's priority reach a real service: picked, it is the ticket's own; not
    /// picked, a child follows its epic and shows the epic's.
    #[test]
    fn priority_round_trips_through_the_service() {
        use crewboard_core::client::{Client, ClientConfig};
        use crewboard_core::protocol::{Reply, Request, TicketUpdateParams};
        use crewboard_core::service::testing::TestService;

        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let w = TestService::start().await;
            w.add_project("shop-web").await;
            let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
            let create = async |m: &EditorModel| match client
                .request(Request::TicketCreate(m.create_input().unwrap()))
                .await
                .unwrap()
            {
                Reply::Ticket(t) => *t,
                other => panic!("{other:?}"),
            };
            let mut epic = EditorModel::create("SHOP", "Wishlist", "main");
            epic.kind = Kind::Epic;
            epic.priority = Some(Priority::High);
            let epic = create(&epic).await;
            assert_eq!(epic.priority, Priority::High);

            let mut m = EditorModel::create("SHOP", "Wishlist button", "main");
            m.epic = Some(epic.key.clone());
            let child = create(&m).await;
            assert_eq!(
                (child.priority, child.priority_inherited),
                (Priority::High, true)
            );

            let mut m = EditorModel::edit(&child);
            m.priority = Some(Priority::Low);
            let Reply::Ticket(child) = client
                .request(Request::TicketUpdate(TicketUpdateParams {
                    key: child.key.to_string(),
                    update: m.update_input().unwrap(),
                }))
                .await
                .unwrap()
            else {
                panic!()
            };
            assert_eq!(
                (child.priority, child.priority_inherited),
                (Priority::Low, false)
            );
            let plain = create(&EditorModel::create("SHOP", "Footer", "main")).await;
            assert_eq!(plain.priority, Priority::Normal);
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
