//! The amber mark on a named agent (AGT-05; d0088, d0077): "Claude Code couldn't run `claude-fable-5-1` on SHOP-48".
//!
//! - **Set** on any model refused ([`set`]): the session's named agent, with the model (the session's exact model,
//!   else the model passed, else the one the tool named) and where it happened (the ticket, the epic, or the project
//!   for a Project agent), and the tool's own message for the edit dialog. A later refusal replaces it.
//! - **Cleared**, never dismissed: when the entry's model is changed and saved ([`super::NamedAgents::edit`]), or
//!   when a session of the entry that started after the mark ends its first turn normally on the model the entry
//!   has now ([`clear_after_first_turn`]). A rename, a note, or another entry's run leaves it. M23 adds "when the
//!   tool's list has it again".

use super::Mark;
use crate::store::{Store, StoreError, now_ms};

/// The model named when nothing says which one ran: a tool default the tool didn't name.
pub const DEFAULT_MODEL: &str = "its default model";

/// The mark's line: "Claude Code couldn't run claude-fable-5-1 on SHOP-48" (views draw the model in mono).
pub fn text(tool: &str, model: &str, on: &str) -> String {
    format!("{tool} couldn't run {model} on {on}")
}

/// The model a mark names: the session's exact model, else the model passed, else the one in the tool's message.
pub fn model_named<'a>(
    model_ran: Option<&'a str>,
    passed: Option<&'a str>,
    in_message: Option<&'a str>,
) -> &'a str {
    [model_ran, passed, in_message]
        .into_iter()
        .flatten()
        .find(|m| !m.trim().is_empty())
        .unwrap_or(DEFAULT_MODEL)
}

/// The mark for a refusal of `model` by `tool` (its name as shown) on `on` (a ticket or epic key, or a project), with
/// the tool's own `message`.
pub fn mark(tool: &str, model: &str, on: &str, message: &str) -> Mark {
    Mark {
        text: text(tool, model, on),
        model: Some(model.to_owned()).filter(|m| m != DEFAULT_MODEL),
        ticket: Some(on.to_owned()),
        message: Some(message.trim().to_owned()).filter(|m| !m.is_empty()),
        at: now_ms(),
    }
}

/// Sets (or replaces) the entry's mark. Whether the entry exists.
pub async fn set(store: &Store, agent_id: &str, mark: &Mark) -> Result<bool, StoreError> {
    let done = sqlx::query(
        "UPDATE named_agents SET mark_text = ?, mark_model = ?, mark_ticket = ?, mark_message = ?, marked_at = ?
         WHERE id = ?",
    )
    .bind(&mark.text)
    .bind(&mark.model)
    .bind(&mark.ticket)
    .bind(&mark.message)
    .bind(mark.at)
    .bind(agent_id)
    .execute(store.pool())
    .await?;
    Ok(done.rows_affected() > 0)
}

/// A session of the entry got through its first turn: the mark clears when the session started after it and ran on
/// the model the entry has now (`passed`: the model the session was given, `None` for the tool default). Whether a
/// mark was cleared.
pub async fn clear_after_first_turn(
    store: &Store,
    agent_id: &str,
    passed: Option<&str>,
    session_started_at: i64,
) -> Result<bool, StoreError> {
    let done = sqlx::query(
        "UPDATE named_agents SET mark_text = NULL, mark_model = NULL, mark_ticket = NULL, mark_message = NULL,
                                 marked_at = NULL
         WHERE id = ? AND marked_at IS NOT NULL AND marked_at <= ? AND model IS ?",
    )
    .bind(agent_id)
    .bind(session_started_at)
    .bind(passed)
    .execute(store.pool())
    .await?;
    Ok(done.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_names_the_tool_the_model_and_where() {
        assert_eq!(
            text("Claude Code", "claude-fable-5-1", "SHOP-48"),
            "Claude Code couldn't run claude-fable-5-1 on SHOP-48"
        );
    }

    #[test]
    fn the_model_is_the_one_that_ran_else_the_one_passed_else_the_tools() {
        let ran = Some("claude-fable-5-1");
        assert_eq!(model_named(ran, Some("fable"), None), "claude-fable-5-1");
        assert_eq!(model_named(None, Some("fable"), ran), "fable");
        assert_eq!(model_named(None, None, ran), "claude-fable-5-1");
        assert_eq!(model_named(None, Some(" "), None), DEFAULT_MODEL);
        let m = mark("Claude Code", DEFAULT_MODEL, "shop-web", " ");
        assert_eq!((m.model, m.message), (None, None));
        assert_eq!(m.ticket.as_deref(), Some("shop-web"));
    }
}
