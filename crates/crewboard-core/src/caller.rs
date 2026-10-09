//! Who is calling the service: one of its agent sessions, or the user (CLI-01, CLI-04; d0031, d0032).
//!
//! The service decides by **where the process runs**: it walks the calling process's ancestry up to the root
//! process of one of its agent sessions. `$CREWBOARD_TICKET` alone decides nothing, so unsetting it inside an
//! agent session doesn't grant the user's full access (d0031). The calling PID is the socket's peer PID, which
//! the kernel reports; a PID claimed in the message must match it.
//!
//! Only sessions that run `claude` for a ticket count (the agent, and later the answer-only and epic-delivery
//! sessions of M10). Shell, Project terminal and Run sessions are the user's own terminals: their callers are the
//! user (CLI-03).
//!
//! Limit (implementation choice): a process that leaves the session's process tree on purpose (double fork plus
//! `setsid`, reparented to `launchd`) is no longer recognised. Claude Code runs its tools, hooks and background
//! tasks as descendants of the session, so its commands always are.

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
}

impl Caller {
    pub fn is_agent(&self) -> bool {
        matches!(self, Self::Agent { .. })
    }

    /// The ticket of an agent caller.
    pub fn ticket(&self) -> Option<&TicketKey> {
        match self {
            Self::Agent { ticket, .. } => Some(ticket),
            Self::User => None,
        }
    }

    /// Who the change is recorded as (`--actor`, CLI-04). An agent is always `agent:<its KEY>`: an explicit actor
    /// can't make it the user or another agent. The user's commands use the requested actor, else the OS user;
    /// the CLI requests `agent:<KEY>` itself when `$CREWBOARD_TICKET` is set (d0032), so the plugin also works by
    /// hand.
    pub fn actor(&self, requested: Option<&str>) -> Actor {
        match self {
            Self::Agent { ticket, .. } => {
                let own = Actor::Agent(ticket.clone());
                if let Some(requested) = requested
                    && requested.parse::<Actor>().ok().as_ref() != Some(&own)
                {
                    tracing::warn!(%own, requested, "an agent's --actor was replaced by its own");
                }
                own
            }
            Self::User => requested
                .filter(|r| !r.trim().is_empty())
                .map(|r| r.parse().expect("infallible"))
                .unwrap_or_else(Actor::current_user),
        }
    }
}

/// The caller for a process: an agent when the process or any of its ancestors is the root process of a running
/// agent session of `sessions`, else the user.
pub fn resolve(pid: Option<u32>, sessions: &Sessions) -> Caller {
    let Some(pid) = pid else {
        return Caller::User;
    };
    let roots = sessions.agent_roots();
    if roots.is_empty() {
        return Caller::User;
    }
    for ancestor in process::ancestry(pid) {
        if let Some(root) = roots.iter().find(|r| r.pid == ancestor) {
            return Caller::Agent {
                ticket: root.ticket.clone(),
                ticket_id: root.ticket_id.clone(),
                session: root.session.clone(),
            };
        }
    }
    Caller::User
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
