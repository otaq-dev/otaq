//! What an agent may do (CLI-02, CLI-03; d0031). Agents run `crewboard` without permission prompts, so the
//! **service** limits them; the CLI stays a thin client.
//!
//! An agent caller may:
//! - **read anything** (`show`, `list`, `ready`, `blocked`, `dep tree`, `prime`, the board);
//! - **change only its own ticket** (`update`, `review submit`, `ask`, `answer`, and from M10 `pr propose`,
//!   `pr opened`, `branch rename`);
//! - **create tickets**, which always land in Backlog (including epic children with `--parent`);
//! - **add or remove dependencies** only from its own ticket, or between two tickets it created.
//!
//! Everything else is refused, for example "SHOP-18's agent can't change SHOP-12". The user (the app, their own
//! terminals, the Project terminal, Shell and Run) passes every check (CLI-03).
//!
//! [`authorize`] is the one policy function; the service maps every request to an [`Action`] before running it.

use crate::board::{BoardError, Refusal};
use crate::caller::Caller;
use crate::domain::{Actor, TicketKey};

/// The facts about a ticket the policy needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TicketFacts {
    pub id: String,
    pub key: TicketKey,
    pub created_by: Actor,
}

/// What a request does, as far as permissions go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Reads anything: tickets, the board, history, settings, sessions' output.
    Read,
    /// Changes this ticket: its fields, its review submission, its questions and answers.
    ChangeTicket(TicketFacts),
    /// Creates a ticket or epic; new tickets always land in Backlog.
    CreateTicket,
    /// Adds or removes the link "`child` depends on `parent`".
    Dependency {
        child: TicketFacts,
        parent: TicketFacts,
    },
    /// Only the user may do this; `what` completes "SHOP-18's agent can't …" ("move tickets").
    UserOnly(&'static str),
}

/// Allows or refuses `action` for `caller`. Refusals are [`Refusal::NotAllowed`] with a message for people.
pub fn authorize(caller: &Caller, action: &Action) -> Result<(), BoardError> {
    let Caller::Agent {
        ticket, ticket_id, ..
    } = caller
    else {
        return Ok(());
    };
    let refuse = |message: String| Err(BoardError::refused(Refusal::NotAllowed, message));
    match action {
        Action::Read | Action::CreateTicket => Ok(()),
        Action::ChangeTicket(target) if target.id == *ticket_id => Ok(()),
        Action::ChangeTicket(target) => {
            refuse(format!("{ticket}'s agent can't change {}", target.key))
        }
        Action::Dependency { child, parent } => {
            let own = Actor::Agent(ticket.clone());
            let from_own = child.id == *ticket_id;
            let both_created = child.created_by == own && parent.created_by == own;
            if from_own || both_created {
                Ok(())
            } else {
                refuse(format!(
                    "{ticket}'s agent can't change {}: it may link only its own ticket, or tickets it created",
                    child.key
                ))
            }
        }
        Action::UserOnly(what) => refuse(format!("{ticket}'s agent can't {what}; ask the user")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(key: &str, id: &str) -> Caller {
        Caller::Agent {
            ticket: key.parse().unwrap(),
            ticket_id: id.into(),
            session: "s".into(),
        }
    }

    fn facts(key: &str, id: &str, created_by: &str) -> TicketFacts {
        TicketFacts {
            id: id.into(),
            key: key.parse().unwrap(),
            created_by: created_by.parse().unwrap(),
        }
    }

    fn message(result: Result<(), BoardError>) -> Option<String> {
        match result {
            Ok(()) => None,
            Err(e) => {
                assert_eq!(e.refusal(), Some(Refusal::NotAllowed));
                Some(e.to_string())
            }
        }
    }

    /// Every command of CLI-02 for SHOP-18's agent, and the same for the user.
    #[test]
    fn the_cli_02_table() {
        let me = agent("SHOP-18", "t18");
        let own = facts("SHOP-18", "t18", "nevzat");
        let other = facts("SHOP-12", "t12", "nevzat");
        let mine_a = facts("SHOP-21", "t21", "agent:SHOP-18");
        let mine_b = facts("SHOP-22", "t22", "agent:SHOP-18");
        let theirs = facts("SHOP-23", "t23", "agent:SHOP-19");
        let dep = |child: &TicketFacts, parent: &TicketFacts| Action::Dependency {
            child: child.clone(),
            parent: parent.clone(),
        };
        let rows: Vec<(&str, Action, Option<&str>)> = vec![
            ("show / list / prime", Action::Read, None),
            ("update own", Action::ChangeTicket(own.clone()), None),
            ("review submit own", Action::ChangeTicket(own.clone()), None),
            ("ask human own", Action::ChangeTicket(own.clone()), None),
            (
                "update another",
                Action::ChangeTicket(other.clone()),
                Some("SHOP-18's agent can't change SHOP-12"),
            ),
            (
                "update a ticket it created",
                Action::ChangeTicket(mine_a.clone()),
                Some("SHOP-18's agent can't change SHOP-21"),
            ),
            (
                "create (Backlog, incl. --parent)",
                Action::CreateTicket,
                None,
            ),
            ("dep add from own", dep(&own, &other), None),
            (
                "dep add between its own creations",
                dep(&mine_a, &mine_b),
                None,
            ),
            (
                "dep add between user tickets",
                dep(&other, &facts("SHOP-13", "t13", "nevzat")),
                Some(
                    "SHOP-18's agent can't change SHOP-12: it may link only its own ticket, or tickets it created",
                ),
            ),
            (
                "dep add from its creation to another agent's",
                dep(&mine_a, &theirs),
                Some(
                    "SHOP-18's agent can't change SHOP-21: it may link only its own ticket, or tickets it created",
                ),
            ),
            (
                "move",
                Action::UserOnly("move tickets"),
                Some("SHOP-18's agent can't move tickets; ask the user"),
            ),
            (
                "delete",
                Action::UserOnly("delete tickets"),
                Some("SHOP-18's agent can't delete tickets; ask the user"),
            ),
        ];
        for (name, action, expected) in &rows {
            assert_eq!(
                message(authorize(&me, action)).as_deref(),
                *expected,
                "agent: {name}"
            );
            assert_eq!(
                message(authorize(&Caller::User, action)),
                None,
                "user: {name}"
            );
        }
    }

    #[test]
    fn identity_is_by_id_so_aliases_count_as_the_same_ticket() {
        let me = agent("SHOP-50.2", "t51");
        // The ticket was SHOP-51 before Move to epic; the id is the same.
        assert!(
            authorize(
                &me,
                &Action::ChangeTicket(facts("SHOP-51", "t51", "nevzat"))
            )
            .is_ok()
        );
    }
}
