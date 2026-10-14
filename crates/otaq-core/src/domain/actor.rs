//! Who made a change (d0032 `--actor`): an agent working on a ticket (`agent:SHOP-18`) or a person (the OS user
//! name). Agent changes raise "Updated by agent" in the UI (d0019).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::TicketKey;

const AGENT_PREFIX: &str = "agent:";
const PROJECT_AGENT_PREFIX: &str = "project-agent:";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Actor {
    /// A person, by OS user name.
    User(String),
    /// The agent working on a ticket.
    Agent(TicketKey),
    /// A Project agent, by its id (d0068): cards name it by its current name ("by Checkout slowness").
    ProjectAgent(String),
}

impl Actor {
    /// The OS user running this process (`$USER`), or `user` if it isn't set.
    pub fn current_user() -> Self {
        Self::User(
            std::env::var("USER")
                .ok()
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| "user".to_owned()),
        )
    }

    /// A ticket's agent or a Project agent: its changes show "created by agent" / "Updated by agent".
    pub fn is_agent(&self) -> bool {
        matches!(self, Self::Agent(_) | Self::ProjectAgent(_))
    }
}

impl fmt::Display for Actor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User(name) => f.write_str(name),
            Self::Agent(key) => write!(f, "{AGENT_PREFIX}{key}"),
            Self::ProjectAgent(id) => write!(f, "{PROJECT_AGENT_PREFIX}{id}"),
        }
    }
}

impl FromStr for Actor {
    type Err = std::convert::Infallible;

    /// `agent:<KEY>` with a valid key is an agent, `project-agent:<id>` a Project agent; anything else is a
    /// person's name.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(id) = s
            .strip_prefix(PROJECT_AGENT_PREFIX)
            .filter(|id| !id.is_empty())
        {
            return Ok(Self::ProjectAgent(id.to_owned()));
        }
        Ok(s.strip_prefix(AGENT_PREFIX)
            .and_then(|key| key.parse().ok())
            .map(Self::Agent)
            .unwrap_or_else(|| Self::User(s.to_owned())))
    }
}

impl Serialize for Actor {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Actor {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <std::borrow::Cow<'de, str>>::deserialize(d)?;
        Ok(s.parse().expect("infallible"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_forms() {
        let agent: Actor = "agent:SHOP-18".parse().unwrap();
        assert_eq!(agent, Actor::Agent("SHOP-18".parse().unwrap()));
        assert!(agent.is_agent());
        assert_eq!(agent.to_string(), "agent:SHOP-18");

        let me: Actor = "nevzat".parse().unwrap();
        assert_eq!(me, Actor::User("nevzat".into()));
        assert!(!me.is_agent());
        assert_eq!(me.to_string(), "nevzat");

        assert_eq!(
            "agent:nonsense".parse::<Actor>().unwrap(),
            Actor::User("agent:nonsense".into())
        );
        let project_agent: Actor = "project-agent:0199a1".parse().unwrap();
        assert_eq!(project_agent, Actor::ProjectAgent("0199a1".into()));
        assert!(project_agent.is_agent());
        assert_eq!(project_agent.to_string(), "project-agent:0199a1");
        assert_eq!(
            "project-agent:".parse::<Actor>().unwrap(),
            Actor::User("project-agent:".into())
        );

        let json = serde_json::to_string(&agent).unwrap();
        assert_eq!(json, "\"agent:SHOP-18\"");
        assert_eq!(serde_json::from_str::<Actor>(&json).unwrap(), agent);
    }
}
