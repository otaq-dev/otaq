//! What an agent may do (CLI-02, CLI-03; d0031). Agents run `otaq` without permission prompts, so the
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
//! terminals, the Project terminal, Shell and Run) passes every check (CLI-03), except reporting a verdict.
//!
//! A **validator** caller (d0063) reads anything and reports the verdict of its own ticket
//! (`validate pass|fail`); every change is refused: "SHOP-29's validator can't change tickets". Only a
//! validator reports a verdict: "Only a validator can report a verdict".
//!
//! A **Project agent** caller (d0068, PAG-02) reads anything, creates tickets (always in Backlog, incl. epics,
//! children, External ID and priority), and edits, links and prioritises tickets **still in Backlog** (never
//! started); it asks a ticket's agent and the user with its own commands, and names itself. Everything else
//! (move, start, approve, merge, close, delete, edit a started ticket) is refused: "Project agents can only change
//! Backlog tickets".
//!
//! [`authorize`] is the one policy function; the service maps every request to an [`Action`] before running it.

use crate::board::{BoardError, Refusal};
use crate::caller::Caller;
use crate::domain::Actor;

/// The refusal of `validate pass|fail` from anyone but a validator.
pub const ONLY_A_VALIDATOR: &str = "Only a validator can report a verdict";

/// Every refusal of a Project agent (PAG-02).
pub const BACKLOG_ONLY: &str = "Project agents can only change Backlog tickets";

/// The facts about a ticket the policy needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TicketFacts {
    pub id: String,
    /// How the ticket is named in a refusal: its key, or "the new ticket \"…\"" in a batch.
    pub label: String,
    pub created_by: Actor,
    /// In Backlog and never started: a Project agent may edit it (d0068).
    pub backlog: bool,
}

/// What a request does, as far as permissions go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Reads anything: tickets, the board, history, settings, sessions' output.
    Read,
    /// Changes this ticket: its review submission, its questions and answers, its PR and branch.
    ChangeTicket(TicketFacts),
    /// Edits this ticket's fields (title, description, priority, External ID): for agents like
    /// [`Action::ChangeTicket`]; a Project agent edits Backlog tickets.
    EditTicket(TicketFacts),
    /// Creates a ticket or epic; new tickets always land in Backlog.
    CreateTicket,
    /// Adds or removes the link "`child` depends on `parent`".
    Dependency {
        child: TicketFacts,
        parent: TicketFacts,
    },
    /// Only the user may do this; `what` completes "SHOP-18's agent can't …" ("move tickets").
    UserOnly(&'static str),
    /// `validate pass|fail` for this ticket: its validator only.
    Verdict(TicketFacts),
    /// Several actions, all needed (a batch that creates tickets and links them).
    All(Vec<Action>),
}

/// Allows or refuses `action` for `caller`. Refusals are [`Refusal::NotAllowed`] with a message for people.
pub fn authorize(caller: &Caller, action: &Action) -> Result<(), BoardError> {
    let refuse = |message: String| Err(BoardError::refused(Refusal::NotAllowed, message));
    if let Caller::Validator {
        ticket, ticket_id, ..
    } = caller
    {
        return match action {
            Action::Read => Ok(()),
            Action::Verdict(target) if target.id == *ticket_id => Ok(()),
            Action::Verdict(target) => refuse(format!(
                "{ticket}'s validator can only report on {ticket}, not {}",
                target.label
            )),
            Action::All(actions) => actions.iter().try_for_each(|a| authorize(caller, a)),
            _ => refuse(format!("{ticket}'s validator can't change tickets")),
        };
    }
    if let Action::Verdict(_) = action {
        return refuse(ONLY_A_VALIDATOR.to_owned());
    }
    if let Caller::ProjectAgent { .. } = caller {
        return match action {
            Action::Read | Action::CreateTicket => Ok(()),
            Action::EditTicket(target) if target.backlog => Ok(()),
            // A Backlog ticket may wait for any ticket ("SHOP-35 waits for SHOP-19").
            Action::Dependency { child, .. } if child.backlog => Ok(()),
            Action::All(actions) => actions.iter().try_for_each(|a| authorize(caller, a)),
            _ => refuse(BACKLOG_ONLY.to_owned()),
        };
    }
    let Caller::Agent {
        ticket, ticket_id, ..
    } = caller
    else {
        return Ok(());
    };
    match action {
        Action::Read | Action::CreateTicket => Ok(()),
        Action::ChangeTicket(target) | Action::EditTicket(target) if target.id == *ticket_id => {
            Ok(())
        }
        Action::ChangeTicket(target) | Action::EditTicket(target) => {
            refuse(format!("{ticket}'s agent can't change {}", target.label))
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
                    child.label
                ))
            }
        }
        Action::UserOnly(what) => refuse(format!("{ticket}'s agent can't {what}; ask the user")),
        Action::Verdict(_) => unreachable!("handled above"),
        Action::All(actions) => actions.iter().try_for_each(|a| authorize(caller, a)),
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
            label: key.into(),
            created_by: created_by.parse().unwrap(),
            backlog: false,
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

    /// T-0200 (PAG-02, CLI-06): a Project agent creates, and edits, links and prioritises Backlog tickets only;
    /// everything else is refused with the one text.
    #[test]
    fn project_agents_change_backlog_tickets_only() {
        let me = Caller::ProjectAgent {
            project: "SHOP".into(),
            agent: "a1".into(),
            session: "s".into(),
        };
        let backlog = TicketFacts {
            backlog: true,
            ..facts("SHOP-35", "t35", "project-agent:a1")
        };
        let started = facts("SHOP-19", "t19", "nevzat");
        let rows: Vec<(&str, Action, bool)> = vec![
            ("show / list / prime", Action::Read, true),
            (
                "create (Backlog, epics, children)",
                Action::CreateTicket,
                true,
            ),
            (
                "update a Backlog ticket",
                Action::EditTicket(backlog.clone()),
                true,
            ),
            (
                "update a started ticket",
                Action::EditTicket(started.clone()),
                false,
            ),
            (
                "dep add from a Backlog ticket to a started one",
                Action::Dependency {
                    child: backlog.clone(),
                    parent: started.clone(),
                },
                true,
            ),
            (
                "dep add from a started ticket",
                Action::Dependency {
                    child: started.clone(),
                    parent: backlog.clone(),
                },
                false,
            ),
            (
                "review submit",
                Action::ChangeTicket(backlog.clone()),
                false,
            ),
            ("move", Action::UserOnly("move tickets"), false),
            ("delete", Action::UserOnly("delete tickets"), false),
            ("approve / merge", Action::UserOnly("merge tickets"), false),
            ("close", Action::UserOnly("close tickets"), false),
        ];
        for (name, action, allowed) in rows {
            let got = message(authorize(&me, &action));
            if allowed {
                assert_eq!(got, None, "{name}");
            } else {
                assert_eq!(got.as_deref(), Some(BACKLOG_ONLY), "{name}");
            }
        }
        assert_eq!(
            message(authorize(&me, &Action::Verdict(backlog))).as_deref(),
            Some(ONLY_A_VALIDATOR)
        );
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

    /// T-0184 (CLI-05, d0063): a validator reads and reports on its own ticket; it changes nothing, and nobody
    /// else reports a verdict.
    #[test]
    fn validators_read_and_report_only() {
        let validator = Caller::Validator {
            ticket: "SHOP-29".parse().unwrap(),
            ticket_id: "t29".into(),
            session: "v".into(),
        };
        let own = facts("SHOP-29", "t29", "nevzat");
        let other = facts("SHOP-12", "t12", "nevzat");
        assert_eq!(message(authorize(&validator, &Action::Read)), None);
        assert_eq!(
            message(authorize(&validator, &Action::Verdict(own.clone()))),
            None
        );
        assert_eq!(
            message(authorize(&validator, &Action::Verdict(other.clone()))).as_deref(),
            Some("SHOP-29's validator can only report on SHOP-29, not SHOP-12")
        );
        // T-0195: it never creates tickets, also not from its own report's notes (the user's Create ticket).
        for action in [
            Action::ChangeTicket(own.clone()),
            Action::CreateTicket,
            Action::UserOnly("create tickets from validation notes"),
            Action::UserOnly("move tickets"),
            Action::Dependency {
                child: own.clone(),
                parent: other.clone(),
            },
        ] {
            assert_eq!(
                message(authorize(&validator, &action)).as_deref(),
                Some("SHOP-29's validator can't change tickets"),
                "{action:?}"
            );
        }
        for caller in [Caller::User, agent("SHOP-29", "t29")] {
            assert_eq!(
                message(authorize(&caller, &Action::Verdict(own.clone()))).as_deref(),
                Some(ONLY_A_VALIDATOR)
            );
        }
    }
}
