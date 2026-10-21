//! The read gate (CMT-08, d0109): an agent can't end its stage until it has read the comments. The service records
//! when the calling session ran `prime` (or `prime --validate`) for its open stage run (`stage_runs.primed_at`);
//! `stage next`, `stage done` and `check pass|fail` from that session are refused until it has, since the run started
//! or since Otaq last handed it a new comment (`stage_runs.handed_over_at`: a Send back, a drag back, a fail that
//! resumes it). Comments posted while it runs after its prime are for the next agent and don't re-arm the gate. The
//! user's own moves never pass through here.

use sqlx::FromRow;

use super::{Board, BoardError, Refusal};
use crate::store::now_ms;

/// The run the gate judges: the ticket's open run, its start, the last hand-over and the last prime.
#[derive(FromRow)]
struct OpenRun {
    id: String,
    started_at: i64,
    handed_over_at: Option<i64>,
    primed_at: Option<i64>,
}

impl OpenRun {
    fn primed(&self) -> bool {
        self.primed_at.is_some_and(|at| {
            at >= self.started_at && self.handed_over_at.is_none_or(|handed| at >= handed)
        })
    }
}

/// "Run `otaq prime` first: SHOP-18 has 2 comments you haven't read."; "Run `otaq prime` first." with none.
pub fn unprimed_refusal(key: &str, unread: usize) -> String {
    let cmd = crate::naming::COMMAND_NAME;
    match unread {
        0 => format!("Run `{cmd} prime` first."),
        1 => format!("Run `{cmd} prime` first: {key} has 1 comment you haven't read."),
        n => format!("Run `{cmd} prime` first: {key} has {n} comments you haven't read."),
    }
}

impl Board {
    /// The ticket's open run that `session` works in; else the ticket's open run (a session the run hasn't taken).
    async fn open_run(
        &self,
        ticket_id: &str,
        session: &str,
    ) -> Result<Option<OpenRun>, BoardError> {
        Ok(sqlx::query_as(
            "SELECT id, started_at, handed_over_at, primed_at FROM stage_runs
             WHERE ticket_id = ? AND ended_at IS NULL
             ORDER BY session_id IS NOT ?, started_at DESC LIMIT 1",
        )
        .bind(ticket_id)
        .bind(session)
        .fetch_optional(self.store.pool())
        .await?)
    }

    /// `session` ran `prime` for the ticket `ticket_id`: its open stage run is primed now. Nothing when the ticket
    /// runs no stage.
    pub async fn record_prime(&self, ticket_id: &str, session: &str) -> Result<(), BoardError> {
        if let Some(run) = self.open_run(ticket_id, session).await? {
            sqlx::query("UPDATE stage_runs SET primed_at = ? WHERE id = ?")
                .bind(now_ms())
                .bind(&run.id)
                .execute(self.store.pool())
                .await?;
        }
        Ok(())
    }

    /// Refuses `session`'s stage end (`stage next`, `stage done`, `check pass|fail`) until it has run `prime` since its
    /// run started or was last handed a new comment, counting the comments posted since its last prime (all of them
    /// when it never ran). Called after [`Board::refuse_unread_notes`]; never for the user's moves.
    pub async fn refuse_unprimed(&self, key: &str, session: &str) -> Result<(), BoardError> {
        let ticket = self.ticket(key).await?;
        let Some(run) = self.open_run(&ticket.id, session).await? else {
            return Ok(());
        };
        if run.primed() {
            return Ok(());
        }
        let unread: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ticket_comments
             WHERE ticket_id = ? AND deleted_at IS NULL AND (? IS NULL OR posted_at > ?)",
        )
        .bind(&ticket.id)
        .bind(run.primed_at)
        .bind(run.primed_at)
        .fetch_one(self.store.pool())
        .await?;
        Err(BoardError::refused(
            Refusal::MoveRefused,
            unprimed_refusal(
                &ticket.key.to_string(),
                usize::try_from(unread).unwrap_or(0),
            ),
        ))
    }
}
