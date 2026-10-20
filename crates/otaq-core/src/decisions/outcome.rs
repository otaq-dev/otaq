//! What the review does with recorded decisions (DEC-05, DEC-06; d0084).
//!
//! - **Approve keeps them:** a Local merge or an Open PR (the PR reported) on a ticket marks every `open` decision
//!   `kept`, on an epic every child's too, with one History line per ticket: "Kept by you · 1 decision". Auto-merge
//!   (d0064) isn't an approval and keeps nothing: those decisions stay open and show in the epic's review.
//! - **Change… on a ticket in review** runs Send back (REV-04, d0034) with a message that names the decision, pending
//!   line comments included; the decision becomes `changed_sent_back` and the Sent back marker counts it.
//! - **Change… in an epic's review**, for a merged child, adds a fix child to the epic as any child added by the user
//!   (EPC-13, d0080): the epic becomes a track again. The decision becomes `changed_fix_child` with the new key.

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{Decision, Outcome, get, list_by_ticket_id};
use crate::board::tickets::{EventKind, NewTicketInput, Ticket, record_event};
use crate::board::{Board, BoardError, Refusal};
use crate::domain::{Actor, Status, TicketKey};
use crate::review::{LocalMergeOutcome, ReturnOutcome, Review};
use crate::store::now_ms;

/// The answer to `decision.change`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ChangeOutcome {
    /// The ticket went back to its agent with the message: "Changed by you → sent back".
    SentBack {
        decision: Box<Decision>,
        ticket: Box<Ticket>,
    },
    /// The session's input line has unsent text: ask "Keep it and cancel / Clear it and send" (d0048), then
    /// repeat with `clear_first`. Nothing was changed. `unclearable_by` as in [`ReturnOutcome::NeedsConfirmation`].
    NeedsConfirmation {
        ticket: TicketKey,
        #[serde(default)]
        unclearable_by: Option<String>,
    },
    /// A fix child joined the epic: "Changed by you → SHOP-40.7".
    FixChild {
        decision: Box<Decision>,
        child: Box<Ticket>,
    },
}

/// The ticket's `open` decisions, and for an epic its children's too: the count on its card and in "Approve ·
/// keeps N decisions" (DEC-03, DEC-05).
pub async fn open_count(board: &Board, key: &str) -> Result<u32, BoardError> {
    let ticket = board.ticket(key).await?;
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM decisions d JOIN tickets t ON t.id = d.ticket_id
         WHERE d.outcome = 'open' AND (t.id = ? OR t.epic_id = ?)",
    )
    .bind(&ticket.id)
    .bind(&ticket.id)
    .fetch_one(board.store().pool())
    .await?;
    Ok(u32::try_from(n).unwrap_or(0))
}

/// Approve (DEC-05): every `open` decision of the ticket `key`, and for an epic of each child, becomes `kept`,
/// with one History line "Kept by you · N decisions" on each ticket that had any. Returns how many were kept.
/// Called by the user's Local merge and when an Open PR is reported, never by Auto-merge.
pub async fn keep_all(board: &Board, key: &str, actor: &Actor) -> Result<u32, BoardError> {
    let ticket = board.ticket(key).await?;
    let mut tickets = vec![ticket.id.clone()];
    if ticket.is_epic() {
        tickets.extend(board.epic_children(key).await?.into_iter().map(|c| c.id));
    }
    let mut open_by_ticket = Vec::new();
    for ticket_id in tickets {
        let open: Vec<Decision> = list_by_ticket_id(board, &ticket_id)
            .await?
            .into_iter()
            .filter(|d| d.outcome == Outcome::Open)
            .collect();
        if !open.is_empty() {
            open_by_ticket.push((ticket_id, open));
        }
    }
    if open_by_ticket.is_empty() {
        return Ok(0);
    }
    let now = now_ms();
    let mut kept = 0u32;
    let mut tx = board.store().begin_write().await?;
    for (ticket_id, open) in &open_by_ticket {
        sqlx::query(
            "UPDATE decisions SET outcome = 'kept', outcome_at = ? WHERE ticket_id = ? AND outcome = 'open'",
        )
        .bind(now)
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
        let decisions: Vec<_> = open
            .iter()
            .map(|d| json!({ "ordinal": d.id.ordinal, "question": d.question, "built": d.built }))
            .collect();
        record_event(
            &mut tx,
            ticket_id,
            EventKind::DecisionsKept,
            actor,
            json!({ "count": open.len(), "decisions": decisions }),
        )
        .await?;
        kept += u32::try_from(open.len()).unwrap_or(u32::MAX);
    }
    tx.commit().await?;
    tracing::info!(key, kept, "decisions kept");
    Ok(kept)
}

/// The user's **Local merge** (GIT-03): the merge, then, once merged, Approve keeps the decisions (DEC-05).
/// Auto-merge calls [`Review::local_merge`] itself and keeps nothing.
pub async fn local_merge_keeping(
    review: &Review,
    key: &str,
    message: &str,
    approve_anyway: bool,
    actor: &Actor,
) -> Result<LocalMergeOutcome, BoardError> {
    let outcome = review
        .local_merge(key, message, approve_anyway, actor)
        .await?;
    if matches!(outcome, LocalMergeOutcome::Merged { .. }) {
        keep_all(review.board(), key, actor).await?;
    }
    Ok(outcome)
}

/// The Send back message of a Change… (07, the "back in Dev" frame): "Review feedback on a recorded decision:
/// “Split fullName everywhere, or only in the profile form?” You built: only in the profile form. Change it to:
/// Split fullName everywhere."
pub fn change_message(decision: &Decision, text: &str) -> String {
    format!(
        "Review feedback on a recorded decision: \u{201c}{}\u{201d} You built: {}. Change it to: {}",
        decision.question.trim(),
        decision.built.trim().trim_end_matches('.'),
        with_period(text.trim()),
    )
}

/// The fix child's title the "Add a fix child" dialog starts from: "Change: in the order they were added" (the
/// first other option; the user rewords it as an instruction).
pub fn fix_child_title(decision: &Decision) -> String {
    let other = decision
        .other_options
        .first()
        .map_or("", |o| o.trim())
        .trim_end_matches('.');
    format!("Change: {other}")
}

/// The fix child's description the dialog starts from (07, "Add a fix child"): the decision, the lead's mark if
/// any, then the change. "SHOP-40.1 built newest first for “Order of saved items?” (recorded decision). Lead:
/// contradicts PAY-123: “items are listed in the order they were added”. Change it to: in the order they were
/// added."
pub fn fix_child_description(decision: &Decision) -> String {
    let mut out = format!(
        "{} built {} for \u{201c}{}\u{201d} (recorded decision).",
        decision.id.ticket,
        decision.built.trim().trim_end_matches('.'),
        decision.question.trim(),
    );
    if let Some(mark) = &decision.mark {
        let text = mark.text.trim().trim_end_matches('.');
        match mark.source.as_deref().map(str::trim) {
            Some(source) if !source.is_empty() && !text.contains(source) => {
                out.push_str(&format!(" Lead: {text} ({source})."))
            }
            _ => out.push_str(&format!(" Lead: {text}.")),
        }
    }
    if let Some(other) = decision.other_options.first() {
        out.push_str(&format!(" Change it to: {}", with_period(other.trim())));
    }
    out
}

fn with_period(text: &str) -> String {
    if text.ends_with(['.', '!', '?']) {
        text.to_owned()
    } else {
        format!("{text}.")
    }
}

/// Only an `open` decision can be changed.
fn still_open(decision: &Decision) -> Result<(), BoardError> {
    match decision.outcome {
        Outcome::Open => Ok(()),
        _ => Err(BoardError::refused(
            Refusal::MoveRefused,
            format!(
                "{} was already reviewed: only an open decision can be changed",
                decision.id
            ),
        )),
    }
}

/// **Change…** on a ticket in Ready for Review (DEC-06): Send back (REV-04) with [`change_message`] as the note,
/// pending line comments included. `clear_first` is the user's "Clear it and send". Refused when the decision
/// isn't open, the text is empty, the ticket isn't in Ready for Review, or it is an epic child already merged
/// (its epic's review adds a fix child instead).
pub async fn change_and_send_back(
    review: &Review,
    id: &str,
    text: &str,
    clear_first: bool,
    actor: &Actor,
) -> Result<ChangeOutcome, BoardError> {
    let board = review.board();
    let decision = get(board, id).await?;
    still_open(&decision)?;
    let text = text.trim();
    if text.is_empty() {
        return Err(BoardError::refused(
            Refusal::EmptyField,
            "Write what the agent should change it to",
        ));
    }
    let ticket = board.ticket_by_id(&decision.ticket_id).await?;
    if let (Some(epic), Status::Merged) = (&ticket.epic, ticket.status) {
        return Err(BoardError::refused(
            Refusal::MoveRefused,
            format!(
                "{} is already merged into {epic}'s branch: Change\u{2026} in {epic}'s review adds a fix child",
                ticket.key
            ),
        ));
    }
    let note = change_message(&decision, text);
    let key = ticket.key.to_string();
    let ticket = match review.send_back(&key, &note, clear_first, actor).await? {
        ReturnOutcome::NeedsConfirmation {
            ticket,
            unclearable_by,
        } => {
            return Ok(ChangeOutcome::NeedsConfirmation {
                ticket,
                unclearable_by,
            });
        }
        ReturnOutcome::Done { ticket } => ticket,
    };
    let round = ticket.flags.sent_back.map_or(0, |s| s.round);
    let mut tx = board.store().begin_write().await?;
    // `outcome_at` is the Send back's time: the Sent back marker counts the decisions changed in its round.
    sqlx::query(
        "UPDATE decisions SET outcome = 'changed_sent_back', outcome_text = ?,
                              outcome_at = COALESCE((SELECT sent_back_at FROM tickets WHERE id = ?), ?)
         WHERE id = ?",
    )
    .bind(text)
    .bind(&decision.ticket_id)
    .bind(now_ms())
    .bind(&decision.row_id)
    .execute(&mut *tx)
    .await?;
    record_event(
        &mut tx,
        &decision.ticket_id,
        EventKind::DecisionChanged,
        actor,
        json!({
            "ordinal": decision.id.ordinal,
            "question": decision.question,
            "to": "sent_back",
            "text": text,
            "round": round,
        }),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(id = %decision.id, round, "decision changed: sent back");
    Ok(ChangeOutcome::SentBack {
        decision: Box::new(get(board, id).await?),
        ticket: Box::new(board.ticket(&key).await?),
    })
}

/// **Change…** in an epic's review (DEC-06): for a decision of a merged child, a new child of the epic with the
/// dialog's `title` and `description`, created by the user as any added child (EPC-13, d0080): it joins the queue
/// when the epic is queued as a whole, and the epic leaves Ready for Review for its track (EPC-11). Refused when
/// the decision isn't open, the ticket isn't a merged child, or its epic isn't in Ready for Review.
pub async fn change_with_fix_child(
    board: &Board,
    id: &str,
    title: &str,
    description: &str,
    actor: &Actor,
) -> Result<ChangeOutcome, BoardError> {
    let decision = get(board, id).await?;
    still_open(&decision)?;
    let ticket = board.ticket_by_id(&decision.ticket_id).await?;
    let (Some(epic), Status::Merged) = (&ticket.epic, ticket.status) else {
        return Err(BoardError::refused(
            Refusal::MoveRefused,
            format!(
                "{} isn't an epic child merged into its epic: Add a fix child works in an epic's review",
                ticket.key
            ),
        ));
    };
    let epic = board.ticket(&epic.to_string()).await?;
    if epic.status != Status::WaitingForYou {
        return Err(BoardError::refused(
            Refusal::MoveRefused,
            format!(
                "{} is {}; Add a fix child works in its review",
                epic.key,
                epic.status.label()
            ),
        ));
    }
    let child = board
        .create_ticket(
            &NewTicketInput {
                project: epic.key.project.to_string(),
                kind: Default::default(),
                title: title.to_owned(),
                body: description.trim().to_owned(),
                priority: None,
                epic: Some(epic.key.to_string()),
                base_branch: None,
                external_id: None,
            },
            actor,
        )
        .await?;
    let mut tx = board.store().begin_write().await?;
    sqlx::query(
        "UPDATE decisions SET outcome = 'changed_fix_child', outcome_text = ?, fix_child_id = ?, outcome_at = ?
         WHERE id = ?",
    )
    .bind(&child.title)
    .bind(&child.id)
    .bind(now_ms())
    .bind(&decision.row_id)
    .execute(&mut *tx)
    .await?;
    record_event(
        &mut tx,
        &decision.ticket_id,
        EventKind::DecisionChanged,
        actor,
        json!({
            "ordinal": decision.id.ordinal,
            "question": decision.question,
            "to": "fix_child",
            "child": child.key,
            "title": child.title,
            "epic": epic.key,
        }),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(id = %decision.id, child = %child.key, "decision changed: fix child");
    Ok(ChangeOutcome::FixChild {
        decision: Box::new(get(board, id).await?),
        child: Box::new(child),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decisions::{DecisionId, Mark};

    fn saved_order(mark: Option<Mark>) -> Decision {
        Decision {
            id: DecisionId {
                ticket: "SHOP-40.1".parse().unwrap(),
                ordinal: 1,
            },
            row_id: "r".into(),
            ticket_id: "t".into(),
            question: "Order of saved items?".into(),
            built: "newest first".into(),
            other_options: vec!["in the order they were added".into()],
            why: "matches the order history page".into(),
            recorded_by: Actor::Agent("SHOP-40.1".parse().unwrap()),
            recorded_at: 0,
            mark,
            outcome: Outcome::Open,
            outcome_text: None,
            fix_child: None,
            outcome_at: None,
        }
    }

    fn lead_mark(text: &str, source: Option<&str>) -> Option<Mark> {
        Some(Mark {
            text: text.into(),
            source: source.map(Into::into),
            by: Actor::ProjectAgent("lead".into()),
            at: 0,
        })
    }

    /// The "Add a fix child" prefill (07): the decision, the lead's mark with its source, then the change.
    #[test]
    fn the_fix_child_prefill_names_the_decision_and_the_mark() {
        let plain = saved_order(None);
        assert_eq!(
            fix_child_title(&plain),
            "Change: in the order they were added"
        );
        assert_eq!(
            fix_child_description(&plain),
            "SHOP-40.1 built newest first for \u{201c}Order of saved items?\u{201d} (recorded decision). Change it \
             to: in the order they were added."
        );
        let marked = saved_order(lead_mark(
            "Contradicts PAY-123: \u{201c}items are listed in the order they were added\u{201d}.",
            Some("PAY-123"),
        ));
        assert_eq!(
            fix_child_description(&marked),
            "SHOP-40.1 built newest first for \u{201c}Order of saved items?\u{201d} (recorded decision). Lead: \
             Contradicts PAY-123: \u{201c}items are listed in the order they were added\u{201d}. Change it to: in \
             the order they were added."
        );
        let sourced = saved_order(lead_mark("the ticket says otherwise", Some("PAY-123")));
        assert!(
            fix_child_description(&sourced).contains("Lead: the ticket says otherwise (PAY-123)."),
            "{}",
            fix_child_description(&sourced)
        );
    }

    #[test]
    fn the_send_back_message_ends_with_one_period() {
        let d = saved_order(None);
        assert_eq!(
            change_message(&d, "oldest first."),
            "Review feedback on a recorded decision: \u{201c}Order of saved items?\u{201d} You built: newest first. \
             Change it to: oldest first."
        );
    }
}
