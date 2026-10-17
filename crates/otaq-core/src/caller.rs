//! Who is calling the service: one of its agent sessions, or the user (CLI-01, CLI-04; d0031, d0032).
//!
//! The service decides by **where the process runs**: it walks the calling process's ancestry up to the root
//! process of one of its agent sessions. `$OTAQ_TICKET` alone decides nothing, so unsetting it inside an
//! agent session doesn't grant the user's full access (d0031). The calling PID is the socket's peer PID, which
//! the kernel reports; a PID claimed in the message must match it.
//!
//! Only sessions that run an agent tool count: a ticket's (the agent, the answer-only and epic-delivery sessions of
//! M10, the validator of M12) and a Project agent's (d0068), an epic lead's among them (d0083). Shell, Project terminal and Run sessions are the
//! user's own terminals: their callers are the user (CLI-03).
//!
//! Limit (implementation choice): a process that leaves the session's process tree on purpose (double fork plus
//! `setsid`, reparented to `launchd`) is no longer recognised. Claude Code runs its tools, hooks and background
//! tasks as descendants of the session, so its commands always are.
//!
//! Windows: Git Bash (which Claude Code runs commands with) replaces a process by starting a new one and ending
//! the old, so the Windows parent chain can break inside a session. There a session's processes are also found
//! by its job object, which holds everything started inside it ([`crate::process::Group::contains`]); the
//! ancestry walk stays the first check.

use serde::{Deserialize, Serialize};

use crate::domain::{Actor, TicketKey};
use crate::process;
use crate::session::Sessions;

/// The resolved caller of one request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Caller {
    /// The user: the app, a terminal of their own, the Project terminal, Shell or Run (CLI-03).
    User,
    /// A process inside an agent session.
    Agent {
        ticket: TicketKey,
        /// The ticket's id.
        ticket_id: String,
        /// The session the caller runs in.
        session: String,
    },
    /// A process inside a ticket's validator session (d0063): it reads, and reports its verdict.
    Validator {
        ticket: TicketKey,
        ticket_id: String,
        session: String,
    },
    /// A process inside a Project agent's session (d0068): it investigates and files Backlog tickets.
    ProjectAgent {
        /// The project's key.
        project: String,
        /// The Project agent's id.
        agent: String,
        session: String,
    },
    /// A process inside an epic lead's session (d0083): a Project agent session whose row has an epic. Its rules
    /// are its own (CLI-07, [`crate::policy`]).
    EpicLead {
        /// The project's key.
        project: String,
        /// The lead's Project agent id.
        agent: String,
        /// The epic's key and id.
        epic: TicketKey,
        epic_id: String,
        session: String,
    },
}

impl Caller {
    pub fn is_agent(&self) -> bool {
        matches!(self, Self::Agent { .. })
    }

    /// The ticket of an agent or validator caller.
    pub fn ticket(&self) -> Option<&TicketKey> {
        match self {
            Self::Agent { ticket, .. } | Self::Validator { ticket, .. } => Some(ticket),
            Self::User | Self::ProjectAgent { .. } | Self::EpicLead { .. } => None,
        }
    }

    /// The Project agent id and session of a Project agent or an epic lead (both run as Project agent sessions).
    pub fn project_agent(&self) -> Option<(&str, &str)> {
        match self {
            Self::ProjectAgent { agent, session, .. } | Self::EpicLead { agent, session, .. } => {
                Some((agent, session))
            }
            Self::User | Self::Agent { .. } | Self::Validator { .. } => None,
        }
    }

    /// Who the change is recorded as (`--actor`, CLI-04). An agent is always `agent:<its KEY>`: an explicit actor
    /// can't make it the user or another agent. The user's commands use the requested actor, else the OS user;
    /// the CLI requests `agent:<KEY>` itself when `$OTAQ_TICKET` is set (d0032), so the plugin also works by
    /// hand.
    pub fn actor(&self, requested: Option<&str>) -> Actor {
        match self {
            Self::Agent { ticket, .. } | Self::Validator { ticket, .. } => {
                let own = Actor::Agent(ticket.clone());
                if let Some(requested) = requested
                    && requested.parse::<Actor>().ok().as_ref() != Some(&own)
                {
                    tracing::warn!(%own, requested, "an agent's --actor was replaced by its own");
                }
                own
            }
            // A Project agent is always itself too (PAG-05): cards name it.
            Self::ProjectAgent { agent, .. } | Self::EpicLead { agent, .. } => {
                Actor::ProjectAgent(agent.clone())
            }
            Self::User => requested
                .filter(|r| !r.trim().is_empty())
                .map(|r| r.parse().expect("infallible"))
                .unwrap_or_else(Actor::current_user),
        }
    }
}

/// The caller for a process: an agent (or a validator, or a Project agent) when the process or any of its
/// ancestors is the root process of a running agent (or validator, or Project agent) session of `sessions`, else
/// the user.
pub fn resolve(pid: Option<u32>, sessions: &Sessions) -> Caller {
    let Some(pid) = pid else {
        return Caller::User;
    };
    let roots = sessions.agent_roots();
    let project_agents = sessions.project_agent_roots();
    if roots.is_empty() && project_agents.is_empty() {
        return Caller::User;
    }
    let ancestry = process::ancestry(pid);
    let project_agent = ancestry
        .iter()
        .find_map(|ancestor| project_agents.iter().find(|r| r.pid == *ancestor).cloned())
        .or_else(|| {
            (!project_agents.is_empty())
                .then(|| sessions.project_agent_holding(pid))
                .flatten()
        });
    if let Some(root) = project_agent {
        return match root.epic {
            Some((epic_id, epic)) => Caller::EpicLead {
                project: root.project,
                agent: root.agent,
                epic,
                epic_id,
                session: root.session,
            },
            None => Caller::ProjectAgent {
                project: root.project,
                agent: root.agent,
                session: root.session,
            },
        };
    }
    let by_ancestry = ancestry
        .into_iter()
        .find_map(|ancestor| roots.iter().find(|r| r.pid == ancestor).cloned());
    match by_ancestry.or_else(|| sessions.agent_root_holding(pid)) {
        Some(root) if root.kind == crate::session::SessionKind::Validator => Caller::Validator {
            ticket: root.ticket,
            ticket_id: root.ticket_id,
            session: root.session,
        },
        Some(root) => Caller::Agent {
            ticket: root.ticket,
            ticket_id: root.ticket_id,
            session: root.session,
        },
        None => Caller::User,
    }
}

/// The Project terminal a user's process runs in, if any: the process or one of its ancestors is the root process
/// of a running Project terminal (an agent the user started there counts too). Tickets created from it are
/// listed in its summary (T-0167, TKT-08).
pub fn project_terminal_of(pid: Option<u32>, sessions: &Sessions) -> Option<String> {
    let pid = pid?;
    let roots = sessions.project_terminal_roots();
    if roots.is_empty() {
        return None;
    }
    process::ancestry(pid)
        .into_iter()
        .find_map(|ancestor| {
            roots
                .iter()
                .find(|(root, _)| *root == ancestor)
                .map(|(_, session)| session.clone())
        })
        .or_else(|| sessions.project_terminal_holding(pid))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent() -> Caller {
        Caller::Agent {
            ticket: "SHOP-18".parse().unwrap(),
            ticket_id: "t18".into(),
            session: "s1".into(),
        }
    }

    #[test]
    fn agents_are_always_their_own_actor() {
        let own = Actor::Agent("SHOP-18".parse().unwrap());
        assert_eq!(agent().actor(None), own);
        assert_eq!(agent().actor(Some("agent:SHOP-18")), own);
        assert_eq!(agent().actor(Some("nevzat")), own, "can't claim the user");
        assert_eq!(agent().actor(Some("agent:SHOP-12")), own);
    }

    #[test]
    fn users_act_as_requested_or_as_the_os_user() {
        assert_eq!(Caller::User.actor(None), Actor::current_user());
        assert_eq!(Caller::User.actor(Some("")), Actor::current_user());
        assert_eq!(
            Caller::User.actor(Some("agent:SHOP-12")),
            Actor::Agent("SHOP-12".parse().unwrap())
        );
        assert_eq!(Caller::User.actor(Some("ci")), Actor::User("ci".into()));
    }
}
