//! What uses a named agent, for Delete (AGT-02, d0086): "Light is used by 12 tickets, the Validator default and
//! project docs-site". In use means: tickets that haven't finished (Backlog through Ready for Review) whose row
//! points at it, the app-wide role defaults, each project whose override points at it, and being another entry's
//! fallback (M23's column, counted already). Done and Closed tickets don't count; Approved doesn't either.

use serde::{Deserialize, Serialize};

use super::{NamedAgent, Role};
use crate::domain::TicketKey;
use crate::store::{Store, StoreError};

/// A project whose overrides point at the entry, and for which roles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectUse {
    pub key: String,
    pub name: String,
    pub roles: Vec<Role>,
}

/// Everything that uses an entry, as parts for the dialog (`O7UTA`) and as one sentence ([`Self::message`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InUse {
    /// The entry's name.
    pub agent: String,
    /// Unfinished tickets whose row points at it, by key.
    pub tickets: Vec<TicketKey>,
    /// The app-wide role defaults on it.
    pub defaults: Vec<Role>,
    /// The projects whose overrides point at it.
    pub projects: Vec<ProjectUse>,
    /// The entries whose fallback it is, by name (M23).
    pub fallback_of: Vec<String>,
}

impl InUse {
    pub fn is_empty(&self) -> bool {
        self.tickets.is_empty()
            && self.defaults.is_empty()
            && self.projects.is_empty()
            && self.fallback_of.is_empty()
    }

    /// AGT-02's sentence: "Light is used by 12 tickets, the Validator default and project docs-site".
    pub fn message(&self) -> String {
        let mut parts = Vec::new();
        match self.tickets.len() {
            0 => {}
            1 => parts.push("1 ticket".to_owned()),
            n => parts.push(format!("{n} tickets")),
        }
        if !self.defaults.is_empty() {
            let roles: Vec<String> = self.defaults.iter().map(|r| r.label().to_owned()).collect();
            let noun = if roles.len() == 1 {
                "default"
            } else {
                "defaults"
            };
            parts.push(format!("the {} {noun}", join(&roles)));
        }
        if !self.projects.is_empty() {
            let names: Vec<String> = self.projects.iter().map(|p| p.name.clone()).collect();
            let noun = if names.len() == 1 {
                "project"
            } else {
                "projects"
            };
            parts.push(format!("{noun} {}", join(&names)));
        }
        if !self.fallback_of.is_empty() {
            parts.push(format!("the fallback of {}", join(&self.fallback_of)));
        }
        format!("{} is used by {}", self.agent, join(&parts))
    }
}

/// "a", "a and b", "a, b and c".
fn join(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Reads what uses `agent`.
pub async fn in_use(store: &Store, agent: &NamedAgent) -> Result<InUse, StoreError> {
    let projects = store
        .projects()
        .await?
        .into_iter()
        .filter_map(|project| {
            let roles = project.agent_defaults().roles_on(&agent.id);
            (!roles.is_empty()).then_some(ProjectUse {
                key: project.key,
                name: project.name,
                roles,
            })
        })
        .collect();
    Ok(InUse {
        agent: agent.name.clone(),
        tickets: store.unfinished_tickets_on_agent(&agent.id).await?,
        defaults: store.agent_defaults().await?.roles_on(&agent.id),
        projects,
        fallback_of: store.fallback_of(&agent.id).await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uses(tickets: usize, defaults: &[Role], projects: &[&str]) -> InUse {
        InUse {
            agent: "Light".into(),
            tickets: (1..=tickets)
                .map(|n| TicketKey::new(crate::domain::ProjectKey::new("SHOP").unwrap(), n as u32))
                .collect(),
            defaults: defaults.to_vec(),
            projects: projects
                .iter()
                .map(|name| ProjectUse {
                    key: "DOCS".into(),
                    name: (*name).into(),
                    roles: vec![Role::Developer],
                })
                .collect(),
            fallback_of: Vec::new(),
        }
    }

    #[test]
    fn the_message_reads_as_agt_02_says() {
        assert_eq!(
            uses(12, &[Role::Validator], &["docs-site"]).message(),
            "Light is used by 12 tickets, the Validator default and project docs-site"
        );
        assert_eq!(uses(1, &[], &[]).message(), "Light is used by 1 ticket");
        assert_eq!(
            uses(0, &[Role::Developer, Role::Validator, Role::EpicLead], &[]).message(),
            "Light is used by the Developer, Validator and Epic lead defaults"
        );
        assert_eq!(
            uses(0, &[], &["docs-site", "shop-web"]).message(),
            "Light is used by projects docs-site and shop-web"
        );
        let mut fallback = uses(0, &[], &[]);
        fallback.fallback_of = vec!["Strong".into()];
        assert_eq!(
            fallback.message(),
            "Light is used by the fallback of Strong"
        );
        assert!(uses(0, &[], &[]).is_empty());
    }
}
