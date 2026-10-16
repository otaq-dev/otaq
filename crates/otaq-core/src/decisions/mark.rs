//! The lead's mark (DEC-04; d0083, d0084): in a merge turn an epic lead may mark a child's recorded decision that a
//! written source contradicts, with `otaq decision mark <ID> --source "<source>" --stdin`. The mark is one line and
//! names its source ("PAY-123"); the review shows it on the decision. The lead never overrules a decision itself:
//! keeping or changing it stays the user's.
//!
//! Only an epic's lead marks, and only its own epic's children's decisions: the service checks the caller first
//! ([`crate::policy::ONLY_A_LEAD_MARKS`], CLI-07). A second mark replaces the first.

use serde_json::json;

use super::{Decision, get};
use crate::board::tickets::{EventKind, record_event};
use crate::board::{Board, BoardError, Refusal};
use crate::domain::Actor;
use crate::store::now_ms;

/// The refusal of a mark without a source.
pub const MISSING_SOURCE: &str =
    "Name the written source the decision contradicts: --source \"PAY-123\"";

/// The refusal of a mark without text.
pub const MISSING_TEXT: &str = "Give the mark on stdin: one line saying what the source says";

/// Marks the decision `id` (`SHOP-40.1#1`) with `text` and its `source`, replacing an earlier mark, and writes
/// "Marked by Wishlist lead · contradicts PAY-123: …" in the child's History. `lead` is the lead's name.
pub async fn mark(
    board: &Board,
    id: &str,
    source: &str,
    text: &str,
    lead: &str,
    actor: &Actor,
) -> Result<Decision, BoardError> {
    let source = source.trim();
    if source.is_empty() {
        return Err(BoardError::refused(Refusal::EmptyField, MISSING_SOURCE));
    }
    // One line: wrapped stdin joins with spaces.
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return Err(BoardError::refused(Refusal::EmptyField, MISSING_TEXT));
    }
    let decision = get(board, id).await?;
    let mut tx = board.store().begin_write().await?;
    sqlx::query(
        "UPDATE decisions SET mark_text = ?, mark_source = ?, marked_by = ?, marked_at = ? WHERE id = ?",
    )
    .bind(&text)
    .bind(source)
    .bind(actor.to_string())
    .bind(now_ms())
    .bind(&decision.row_id)
    .execute(&mut *tx)
    .await?;
    record_event(
        &mut tx,
        &decision.ticket_id,
        EventKind::DecisionMarked,
        actor,
        json!({
            "ordinal": decision.id.ordinal,
            "lead": lead,
            "source": source,
            "text": text,
        }),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(id = %decision.id, actor = %actor, "decision marked");
    get(board, &decision.id.to_string()).await
}

/// The reply to `decision mark`: "Marked SHOP-40.1#1. The user sees it in the review; keeping or changing it is
/// theirs."
pub fn marked_message(decision: &Decision) -> String {
    format!(
        "Marked {}. The user sees it in the review; keeping or changing it is theirs.",
        decision.id
    )
}
