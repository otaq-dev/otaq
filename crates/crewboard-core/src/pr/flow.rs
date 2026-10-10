//! The Open PR / Push to PR flow of a delivery unit (REV-10, REV-14; d0050, d0041, d0059), from the user's click
//! until the skill reports the PR or the user cancels: `pr_flows` (migration 0018).

use serde::{Deserialize, Serialize};

use super::proposal::Proposal;
use crate::board::{Board, BoardError};
use crate::domain::stored_enum;

stored_enum! {
    pub enum FlowKind ("PR flow kind") {
        /// Open PR: the first push, the branch gets its real name.
        Open = "open",
        /// Push to PR #N: new commits to the PR's locked branch.
        Push = "push",
    }
}

stored_enum! {
    pub enum FlowState ("PR flow state") {
        /// The skill runs; no proposal yet ("Open PR skill is preparing…").
        Preparing = "preparing",
        /// The dialog shows the proposal.
        Proposed = "proposed",
        /// The user confirmed: the edits are applied and the skill was told to push.
        Pushing = "pushing",
    }
}

/// A ticket's PR flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrFlow {
    pub kind: FlowKind,
    pub state: FlowState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal: Option<Proposal>,
    /// What the user confirmed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed: Option<Proposal>,
    /// What the skill reported instead of a PR (a failed push, a missing login).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported: Option<String>,
    pub started_at: i64,
}

type FlowRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    i64,
);

impl Board {
    /// The ticket's PR flow, if one is under way.
    pub async fn pr_flow(&self, ticket_id: &str) -> Result<Option<PrFlow>, BoardError> {
        let row: Option<FlowRow> = sqlx::query_as(
            "SELECT kind, state, proposal_json, confirmed_json, reported, started_at FROM pr_flows WHERE ticket_id = ?",
        )
        .bind(ticket_id)
        .fetch_optional(self.store().pool())
        .await?;
        let Some((kind, state, proposal, confirmed, reported, started_at)) = row else {
            return Ok(None);
        };
        let parse = |json: Option<String>| json.and_then(|j| serde_json::from_str(&j).ok());
        Ok(Some(PrFlow {
            kind: kind.parse().map_err(unknown)?,
            state: state.parse().map_err(unknown)?,
            proposal: parse(proposal),
            confirmed: parse(confirmed),
            reported,
            started_at,
        }))
    }
}

fn unknown(e: crate::domain::UnknownValue) -> BoardError {
    BoardError::NotFound(e.to_string())
}
