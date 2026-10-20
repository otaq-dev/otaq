//! The lead's notes to running children (LEAD-10, CLI-07, CLI-08; d0083, d0048). A note waits for a checkpoint the
//! child's agent passes anyway, so it works for any agent provider:
//! - `otaq note <KEY> --stdin` (the lead, for a **running** child of its epic) stores the note as **unread**, with
//!   "Note from Wishlist lead" in the child's and the epic's History. A never-started child is refused (the lead
//!   edits its description instead), and so is a child that isn't running any more ([`note_target`]). A note to a
//!   child whose first stop waits for the lead answers that stop instead ([`super::stuck`], LEAD-07).
//! - `otaq notes` (the child's agent; the Commit skill runs it before each commit) prints the unread notes and marks
//!   them read; `otaq prime` lists them at the top and marks them read too.
//! - `otaq review submit` is refused while one is unread ([`submit_refusal`]); the user's Submit for review isn't.
//! - When the child goes idle with an unread note, the service types it in as an automatic message (§9.3,
//!   [`type_in`]) and marks it read; that is input for the agent, so the child doesn't become an Idle stop.
//!
//! The card shows "1 unread note from Wishlist lead" ([`crate::domain::UnreadNotes`]) until the notes are read.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::board::epic_flight::is_never_started;
use crate::board::tickets::{EventKind, Ticket, key_from_parts, record_event};
use crate::board::{Board, BoardError, Refusal};
use crate::domain::{Actor, Status, TicketKey, UnreadNotes, stored_enum};
use crate::naming;
use crate::project_agents::lead::lead_name;
use crate::session::deliveries::{Deliveries, DeliveryKind};
use crate::session::{SessionKind, Sessions};
use crate::store::{new_id, now_ms};

stored_enum! {
    /// How a note was read (`lead_notes.read_how`).
    pub enum ReadHow ("note read") {
        /// The agent ran `otaq notes`.
        Notes = "notes",
        /// The agent ran `otaq prime`.
        Prime = "prime",
        /// The service typed it in when the child went idle.
        Typed = "typed",
        /// It went with a stuck child's own action (T-0285).
        WithStop = "with_stop",
    }
}

/// One note of a lead to a child.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeadNote {
    pub id: String,
    pub epic: TicketKey,
    pub epic_id: String,
    pub child: TicketKey,
    pub child_id: String,
    /// The lead's name: "Wishlist lead".
    pub lead: String,
    /// The lead's Project agent id.
    pub lead_id: String,
    /// The lead's actor (`project-agent:<id>`).
    pub written_by: Actor,
    pub text: String,
    /// The lead's view of a stuck child's cause (T-0285).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_how: Option<ReadHow>,
}

/// The refusal of `note` from anyone but an epic's lead (CLI-07).
pub const ONLY_A_LEAD_NOTES: &str = "Only an epic's lead can leave a note for a child";

/// The refusal of a note without text.
pub const MISSING_TEXT: &str = "Give the note on stdin";

/// What a note to a child of the epic does, from where the child stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteTarget {
    /// In Dev with an agent at work: the note waits as unread.
    Running,
    /// Its first stop waits for the lead (LEAD-07): the note answers it as Answer the agent
    /// ([`super::stuck::Stuck::answer`]).
    Stuck,
    /// It has never been to Dev: refused, the lead edits its description instead.
    NeverStarted,
    /// It has finished or isn't running (Interrupted, Parked, in review, Done): refused.
    NotRunning,
}

/// Where a child stands for a note; `held` says its first stop waits for the lead ([`super::stuck::is_held`]).
pub fn note_target(child: &Ticket, held: bool) -> NoteTarget {
    if held {
        NoteTarget::Stuck
    } else if is_never_started(child) {
        NoteTarget::NeverStarted
    } else if super::overlap::is_running(child.status) {
        NoteTarget::Running
    } else {
        NoteTarget::NotRunning
    }
}

/// "SHOP-40.5 hasn't started; edit its description instead (`otaq update`)".
pub fn never_started_message(child: &TicketKey) -> String {
    format!(
        "{child} hasn't started; edit its description instead (`{} update`)",
        naming::COMMAND_NAME
    )
}

/// "SHOP-40.1 isn't running (Merged); a note reaches only a running child".
pub fn not_running_message(child: &Ticket) -> String {
    format!(
        "{} isn't running ({}); a note reaches only a running child",
        child.key,
        child.status.label()
    )
}

/// The reply to `otaq note`.
pub fn left_message(child: &TicketKey) -> String {
    format!("Note left for {child}. Its agent reads it before its next commit or submit.")
}

/// The reply to `otaq note`: [`left_message`], or for a note that answered the child's stop (LEAD-07) "Note sent to
/// SHOP-40.3 with its stop, as Answer the agent. You won't be woken for SHOP-40.3 again."
pub fn reply_message(note: &LeadNote) -> String {
    if note.read_how == Some(ReadHow::WithStop) {
        format!(
            "Note sent to {child} with its stop, as Answer the agent. You won't be woken for {child} again.",
            child = note.child
        )
    } else {
        left_message(&note.child)
    }
}

/// "1 unread note from Wishlist lead", "2 unread notes from Wishlist lead".
pub fn count_text(count: usize, lead: &str) -> String {
    match count {
        1 => format!("1 unread note from {lead}"),
        n => format!("{n} unread notes from {lead}"),
    }
}

/// What `otaq notes` prints: "1 unread note from Wishlist lead (14:20): …", one line per note after a count line
/// when there are several, or "No unread notes.".
pub fn unread_message(notes: &[LeadNote]) -> String {
    match notes {
        [] => "No unread notes.".to_owned(),
        [note] => format!(
            "{} ({}): {}",
            count_text(1, &note.lead),
            clock(note.created_at),
            note.text
        ),
        [first, ..] => {
            let mut out = format!("{}:", count_text(notes.len(), &first.lead));
            for note in notes {
                out.push_str(&format!("\n- ({}): {}", clock(note.created_at), note.text));
            }
            out
        }
    }
}

/// "Submit refused: 1 unread note from Wishlist lead. Run `otaq notes`, then submit again." (CLI-08).
pub fn submit_refusal(count: usize, lead: &str) -> String {
    format!(
        "Submit refused: {}. Run `{} notes`, then submit again.",
        count_text(count, lead),
        naming::COMMAND_NAME
    )
}

/// The automatic message typed in when the child goes idle: "Note from Wishlist lead: …", one paragraph per note.
pub fn typed_text(notes: &[LeadNote]) -> String {
    notes
        .iter()
        .map(|n| format!("Note from {}: {}", n.lead, n.text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// "14:20", local time.
fn clock(at: i64) -> String {
    crate::local_time::local(at)
        .map(|t| format!("{:02}:{:02}", t.hour, t.minute))
        .unwrap_or_default()
}

#[derive(sqlx::FromRow)]
struct Row {
    id: String,
    epic_id: String,
    child_id: String,
    lead_id: String,
    written_by: String,
    text: String,
    view: Option<String>,
    created_at: i64,
    read_at: Option<i64>,
    read_how: Option<String>,
    project_key: String,
    epic_number: i64,
    child_number: i64,
    epic_title: String,
}

const SELECT: &str = "SELECT n.*, p.key AS project_key, e.number AS epic_number, c.child_number,
                             e.title AS epic_title
                      FROM lead_notes n
                      JOIN tickets c ON c.id = n.child_id
                      JOIN tickets e ON e.id = n.epic_id
                      JOIN projects p ON p.id = c.project_id";

impl Row {
    fn into_note(self) -> Option<LeadNote> {
        Some(LeadNote {
            epic: key_from_parts(&self.project_key, self.epic_number, 0)?,
            child: key_from_parts(&self.project_key, self.epic_number, self.child_number)?,
            id: self.id,
            epic_id: self.epic_id,
            child_id: self.child_id,
            lead: lead_name(&self.epic_title),
            lead_id: self.lead_id,
            written_by: self.written_by.parse().expect("infallible"),
            text: self.text,
            view: self.view,
            created_at: self.created_at,
            read_at: self.read_at,
            read_how: self.read_how.and_then(|h| h.parse().ok()),
        })
    }
}

async fn select(board: &Board, filter: &str, bind: &str) -> Result<Vec<LeadNote>, BoardError> {
    let sql = format!("{SELECT} {filter} ORDER BY n.created_at, n.id");
    Ok(sqlx::query_as::<_, Row>(sqlx::AssertSqlSafe(sql))
        .bind(bind.to_owned())
        .fetch_all(board.store().pool())
        .await?
        .into_iter()
        .filter_map(Row::into_note)
        .collect())
}

/// Stores the lead's note to `child`, a child of the epic `epic_id` (the caller's epic; the policy checked it is
/// one of its children), with History on the child and the epic. `view` is the lead's view of a stuck child's cause
/// (`--view`), kept with the note. Refused without text, for a ticket outside the epic, and for a child that isn't
/// running or stuck ([`note_target`]).
pub async fn leave(
    board: &Board,
    epic_id: &str,
    lead_id: &str,
    child: &str,
    text: &str,
    view: Option<&str>,
    actor: &Actor,
) -> Result<LeadNote, BoardError> {
    let text = text.trim();
    let view = view.map(str::trim).filter(|v| !v.is_empty());
    if text.is_empty() {
        return Err(BoardError::refused(Refusal::EmptyField, MISSING_TEXT));
    }
    let ticket = board.ticket(child).await?;
    let epic = board.ticket_by_id(epic_id).await?;
    let lead = lead_name(&epic.title);
    if ticket.epic.as_ref() != Some(&epic.key) {
        return Err(BoardError::refused(
            Refusal::NotAllowed,
            crate::policy::lead_refusal(&lead, &epic.key),
        ));
    }
    let held = super::stuck::is_held(board, &ticket.id).await?;
    match note_target(&ticket, held) {
        NoteTarget::Running | NoteTarget::Stuck => {}
        NoteTarget::NeverStarted => {
            return Err(BoardError::refused(
                Refusal::Started,
                never_started_message(&ticket.key),
            ));
        }
        NoteTarget::NotRunning => {
            return Err(BoardError::refused(
                Refusal::NotAllowed,
                not_running_message(&ticket),
            ));
        }
    }
    let id = new_id();
    let mut tx = board.store().begin_write().await?;
    sqlx::query(
        "INSERT INTO lead_notes (id, epic_id, child_id, lead_id, written_by, text, view, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(epic_id)
    .bind(&ticket.id)
    .bind(lead_id)
    .bind(actor.to_string())
    .bind(text)
    .bind(view)
    .bind(now_ms())
    .execute(&mut *tx)
    .await?;
    let mut payload = json!({"lead": lead, "child": ticket.key, "text": text});
    if let (Some(view), Some(p)) = (view, payload.as_object_mut()) {
        p.insert("view".into(), json!(view));
    }
    for ticket_id in [&ticket.id, &epic.id] {
        record_event(
            &mut tx,
            ticket_id,
            EventKind::LeadNote,
            actor,
            payload.clone(),
        )
        .await?;
    }
    tx.commit().await?;
    tracing::info!(child = %ticket.key, actor = %actor, "lead note");
    get(board, &id).await
}

/// One note by id.
pub async fn get(board: &Board, id: &str) -> Result<LeadNote, BoardError> {
    select(board, "WHERE n.id = ?", id)
        .await?
        .pop()
        .ok_or_else(|| BoardError::NotFound(format!("unknown note {id}")))
}

/// The child's unread notes (by ticket id), oldest first.
pub async fn unread(board: &Board, ticket_id: &str) -> Result<Vec<LeadNote>, BoardError> {
    select(
        board,
        "WHERE n.child_id = ? AND n.read_at IS NULL",
        ticket_id,
    )
    .await
}

/// Every note the epic's lead left (by the epic's key), read or not, oldest first: `prime --epic` lists the unread
/// ones (T-0283).
pub async fn list_for_epic(board: &Board, epic: &str) -> Result<Vec<LeadNote>, BoardError> {
    let epic_id = board.ticket_id(epic).await?;
    select(board, "WHERE n.epic_id = ?", &epic_id).await
}

/// Marks these notes read, `how`; notes read already keep their first reading. Returns how many it marked.
pub async fn mark_read(board: &Board, ids: &[String], how: ReadHow) -> Result<u64, BoardError> {
    let mut marked = 0;
    let now = now_ms();
    for id in ids {
        marked += sqlx::query(
            "UPDATE lead_notes SET read_at = ?, read_how = ? WHERE id = ? AND read_at IS NULL",
        )
        .bind(now)
        .bind(how.as_str())
        .bind(id)
        .execute(board.store().pool())
        .await?
        .rows_affected();
    }
    Ok(marked)
}

/// `otaq notes`: the child's unread notes, now marked read.
pub async fn read_unread(board: &Board, key: &str) -> Result<Vec<LeadNote>, BoardError> {
    let ticket = board.ticket(key).await?;
    let notes = unread(board, &ticket.id).await?;
    let ids: Vec<String> = notes.iter().map(|n| n.id.clone()).collect();
    mark_read(board, &ids, ReadHow::Notes).await?;
    Ok(notes)
}

/// The "1 unread note from Wishlist lead" flag of every ticket of the project that has one, by ticket id.
pub(crate) async fn unread_flags(
    board: &Board,
    project_id: &str,
) -> Result<HashMap<String, UnreadNotes>, BoardError> {
    let rows: Vec<(String, String, i64, String)> = sqlx::query_as(
        "SELECT n.child_id, n.text, n.created_at, e.title FROM lead_notes n
         JOIN tickets c ON c.id = n.child_id JOIN tickets e ON e.id = n.epic_id
         WHERE c.project_id = ? AND n.read_at IS NULL ORDER BY n.created_at, n.id",
    )
    .bind(project_id)
    .fetch_all(board.store().pool())
    .await?;
    let mut flags: HashMap<String, UnreadNotes> = HashMap::new();
    for (child, text, at, epic_title) in rows {
        let flag = flags.entry(child).or_insert_with(|| UnreadNotes {
            count: 0,
            from: lead_name(&epic_title),
            latest_text: String::new(),
            latest_at: 0,
        });
        flag.count += 1;
        flag.latest_text = text;
        flag.latest_at = at;
    }
    Ok(flags)
}

/// Whether a Stop that ends the turn with nothing pending would make this child an Idle stop, so an unread note may
/// be typed in instead: its agent was working (not validating, not waiting for the user).
pub fn idle_at_stop(status: Status) -> bool {
    matches!(status, Status::Working | Status::WorkingInBackground)
}

/// Types the child's unread notes into its agent's session as one automatic message ("Note from Wishlist lead:
/// …", §9.3) and marks them read, if the agent is idle between turns with an empty input line. Whether it typed.
pub async fn type_in(
    board: &Board,
    sessions: &Sessions,
    deliveries: &Deliveries,
    ticket_id: &str,
) -> Result<bool, BoardError> {
    let Some(session) = sessions.running_of(ticket_id, SessionKind::Agent) else {
        return Ok(false);
    };
    let ready = deliveries.is_ready(&session.id).map_err(session_error)?;
    if !ready {
        return Ok(false);
    }
    let notes = unread(board, ticket_id).await?;
    if notes.is_empty() {
        return Ok(false);
    }
    deliveries
        .hold_delivery(&session.id, DeliveryKind::LeadNote, &typed_text(&notes))
        .await
        .map_err(session_error)?;
    let sent = deliveries
        .deliver_if_ready(&session.id)
        .await
        .map_err(session_error)?;
    if sent == 0 {
        return Ok(false);
    }
    let ids: Vec<String> = notes.iter().map(|n| n.id.clone()).collect();
    mark_read(board, &ids, ReadHow::Typed).await?;
    tracing::info!(ticket_id, notes = ids.len(), "lead notes typed in at idle");
    Ok(true)
}

/// The Stop of a working child's agent with unread notes: types them in ([`type_in`]) so the child stays at work
/// instead of becoming an Idle stop, and tells clients the card's marker cleared. Whether it typed.
pub async fn type_in_at_stop(
    board: &Board,
    sessions: &Sessions,
    deliveries: &Deliveries,
    events: &crate::service::EventBus,
    ticket_id: &str,
) -> bool {
    let typed = async {
        let ticket = board.ticket_by_id(ticket_id).await?;
        if !idle_at_stop(ticket.status) {
            return Ok(None);
        }
        Ok::<_, BoardError>(
            type_in(board, sessions, deliveries, ticket_id)
                .await?
                .then_some(ticket),
        )
    };
    match typed.await {
        Ok(Some(ticket)) => {
            events.publish(crate::protocol::Event::TicketChanged {
                project: ticket.key.project.to_string(),
                id: ticket.id,
                key: ticket.key,
            });
            true
        }
        Ok(None) => false,
        Err(e) => {
            tracing::warn!(ticket_id, error = %e, "can't type the lead's notes in");
            false
        }
    }
}

fn session_error(e: crate::session::SessionError) -> BoardError {
    BoardError::refused(Refusal::SessionEnded, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(text: &str, at: i64) -> LeadNote {
        LeadNote {
            id: "n".into(),
            epic: "SHOP-40".parse().unwrap(),
            epic_id: "e".into(),
            child: "SHOP-40.4".parse().unwrap(),
            child_id: "c".into(),
            lead: "Wishlist lead".into(),
            lead_id: "l".into(),
            written_by: Actor::ProjectAgent("l".into()),
            text: text.into(),
            view: None,
            created_at: at,
            read_at: None,
            read_how: None,
        }
    }

    #[test]
    fn the_wordings() {
        let cmd = naming::COMMAND_NAME;
        assert_eq!(
            left_message(&"SHOP-40.4".parse().unwrap()),
            "Note left for SHOP-40.4. Its agent reads it before its next commit or submit."
        );
        assert_eq!(
            never_started_message(&"SHOP-40.5".parse().unwrap()),
            format!("SHOP-40.5 hasn't started; edit its description instead (`{cmd} update`)")
        );
        assert_eq!(
            submit_refusal(1, "Wishlist lead"),
            format!(
                "Submit refused: 1 unread note from Wishlist lead. Run `{cmd} notes`, then submit again."
            )
        );
        assert_eq!(
            submit_refusal(2, "Wishlist lead"),
            format!(
                "Submit refused: 2 unread notes from Wishlist lead. Run `{cmd} notes`, then submit again."
            )
        );
        assert_eq!(unread_message(&[]), "No unread notes.");
        let at = crate::local_time::on_same_day_at(now_ms(), 14, 20).unwrap();
        assert_eq!(
            unread_message(&[note("Use the same order on the shared list.", at)]),
            "1 unread note from Wishlist lead (14:20): Use the same order on the shared list."
        );
        assert_eq!(
            unread_message(&[note("A", at), note("B", at)]),
            "2 unread notes from Wishlist lead:\n- (14:20): A\n- (14:20): B"
        );
        assert_eq!(
            typed_text(&[note("A", at), note("B", at)]),
            "Note from Wishlist lead: A\n\nNote from Wishlist lead: B"
        );
    }

    #[test]
    fn only_a_working_agents_stop_would_be_idle() {
        assert!(idle_at_stop(Status::Working));
        assert!(idle_at_stop(Status::WorkingInBackground));
        for status in [
            Status::Checking,
            Status::AsksYou,
            Status::Idle,
            Status::WaitingForYou,
        ] {
            assert!(!idle_at_stop(status), "{status:?}");
        }
    }
}
