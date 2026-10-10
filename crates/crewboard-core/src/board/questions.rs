//! Questions to the user (`crewboard ask human`, ATT-01) with optional answer options (ATT-12, d0061).
//!
//! A question may carry **2–4 options** with short labels and, optionally, the recommended one (1-based); without
//! options it is free text. The UI always adds "Other…" (M7). Options apply only to questions to the human; a
//! question to another ticket (`ask <KEY>`, M10) stays free text. The answer is typed into the session as
//! [`format_answer`] words it.

use serde::{Deserialize, Serialize};

use super::{Board, BoardError, Refusal};
use crate::store::{new_id, now_ms};

/// The most options a question may carry.
pub const MAX_OPTIONS: usize = 4;
/// The longest option label, in characters: labels are short so they fit the inbox, card and panel.
pub const MAX_OPTION_LABEL: usize = 40;

/// A question an agent asked the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanQuestion {
    pub id: String,
    pub text: String,
    /// The answer options, in order; empty for a free-text question.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    /// The recommended option, 1-based.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended: Option<u32>,
    pub asked_at: i64,
}

/// The user's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    /// One of the options, 1-based.
    Option(u32),
    /// "Other…": free text.
    Other(String),
}

/// Checks the options of a question: none, or 2–4 short labels, and a recommendation among them.
pub fn validate_options(options: &[String], recommended: Option<u32>) -> Result<(), BoardError> {
    let refuse = |message: String| Err(BoardError::refused(Refusal::BadQuestion, message));
    if options.len() == 1 || options.len() > MAX_OPTIONS {
        return refuse("Give 2–4 options, or none for a free-text question".to_owned());
    }
    if options.iter().any(|o| o.trim().is_empty()) {
        return refuse("An option can't be empty".to_owned());
    }
    if let Some(long) = options
        .iter()
        .find(|o| o.trim().chars().count() > MAX_OPTION_LABEL)
    {
        return refuse(format!(
            "Option labels can be at most {MAX_OPTION_LABEL} characters: \"{}\" is too long",
            long.trim()
        ));
    }
    match recommended {
        Some(_) if options.is_empty() => {
            refuse("--recommended needs options to choose from".to_owned())
        }
        Some(n) if n == 0 || n as usize > options.len() => refuse(format!(
            "--recommended must be between 1 and {}",
            options.len()
        )),
        _ => Ok(()),
    }
}

/// The text typed into the agent's session: "Answer to your question: Postmark (option 1)." for an option,
/// "Answer to your question: <text>" for "Other…".
pub fn format_answer(question: &HumanQuestion, choice: &Choice) -> String {
    match choice {
        Choice::Option(n) => {
            let label = question
                .options
                .get((*n as usize).wrapping_sub(1))
                .map_or("?", String::as_str);
            format!("Answer to your question: {label} (option {n}).")
        }
        Choice::Other(text) => format!("Answer to your question: {}", text.trim()),
    }
}

#[derive(sqlx::FromRow)]
struct QuestionRow {
    id: String,
    question: String,
    options_json: String,
    recommended: Option<i64>,
    asked_at: i64,
}

impl From<QuestionRow> for HumanQuestion {
    fn from(row: QuestionRow) -> Self {
        Self {
            id: row.id,
            text: row.question,
            options: serde_json::from_str(&row.options_json).unwrap_or_default(),
            recommended: row.recommended.and_then(|n| u32::try_from(n).ok()),
            asked_at: row.asked_at,
        }
    }
}

impl Board {
    /// Stores a question of `ticket_id` (options already validated).
    pub async fn store_question(
        &self,
        ticket_id: &str,
        text: &str,
        options: &[String],
        recommended: Option<u32>,
    ) -> Result<HumanQuestion, BoardError> {
        validate_options(options, recommended)?;
        let question = HumanQuestion {
            id: new_id(),
            text: text.trim().to_owned(),
            options: options.iter().map(|o| o.trim().to_owned()).collect(),
            recommended,
            asked_at: now_ms(),
        };
        sqlx::query(
            "INSERT INTO human_questions (id, ticket_id, question, options_json, recommended, asked_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&question.id)
        .bind(ticket_id)
        .bind(&question.text)
        .bind(serde_json::to_string(&question.options).expect("strings serialize"))
        .bind(question.recommended.map(i64::from))
        .bind(question.asked_at)
        .execute(self.store().pool())
        .await?;
        Ok(question)
    }

    /// Every unanswered question of a project's tickets, newest first: (ticket id, question).
    pub(crate) async fn open_questions(
        &self,
        project_id: &str,
    ) -> Result<Vec<(String, HumanQuestion)>, BoardError> {
        let rows: Vec<(String, String, String, String, Option<i64>, i64)> = sqlx::query_as(
            "SELECT q.ticket_id, q.id, q.question, q.options_json, q.recommended, q.asked_at
             FROM human_questions q JOIN tickets t ON t.id = q.ticket_id
             WHERE t.project_id = ? AND q.answered_at IS NULL ORDER BY q.asked_at DESC, q.id DESC",
        )
        .bind(project_id)
        .fetch_all(self.store().pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(ticket_id, id, question, options_json, recommended, asked_at)| {
                    let row = QuestionRow {
                        id,
                        question,
                        options_json,
                        recommended,
                        asked_at,
                    };
                    (ticket_id, row.into())
                },
            )
            .collect())
    }

    /// Records the answer to a question (`crewboard questions`, M10, reads it back).
    pub async fn record_answer(&self, question_id: &str, answer: &str) -> Result<(), BoardError> {
        sqlx::query(
            "UPDATE human_questions SET answer = ?, answered_at = ? WHERE id = ? AND answered_at IS NULL",
        )
        .bind(answer)
        .bind(now_ms())
        .bind(question_id)
        .execute(self.store().pool())
        .await?;
        Ok(())
    }

    /// The ticket's latest unanswered question.
    pub async fn open_question(
        &self,
        ticket_id: &str,
    ) -> Result<Option<HumanQuestion>, BoardError> {
        let row: Option<QuestionRow> = sqlx::query_as(
            "SELECT id, question, options_json, recommended, asked_at FROM human_questions
             WHERE ticket_id = ? AND answered_at IS NULL ORDER BY asked_at DESC, id DESC LIMIT 1",
        )
        .bind(ticket_id)
        .fetch_optional(self.store().pool())
        .await?;
        Ok(row.map(Into::into))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(labels: &[&str]) -> Vec<String> {
        labels.iter().map(|l| (*l).to_owned()).collect()
    }

    fn refusal(result: Result<(), BoardError>) -> String {
        let e = result.unwrap_err();
        assert_eq!(e.refusal(), Some(Refusal::BadQuestion));
        e.to_string()
    }

    #[test]
    fn two_to_four_short_options() {
        assert!(validate_options(&[], None).is_ok());
        assert!(validate_options(&opts(&["Postmark", "SES", "Resend"]), Some(1)).is_ok());
        assert!(validate_options(&opts(&["a", "b", "c", "d"]), None).is_ok());
        let wrong = "Give 2–4 options, or none for a free-text question";
        assert_eq!(refusal(validate_options(&opts(&["only"]), None)), wrong);
        assert_eq!(
            refusal(validate_options(&opts(&["a", "b", "c", "d", "e"]), None)),
            wrong
        );
        let long = "x".repeat(41);
        assert!(
            refusal(validate_options(&opts(&["a", &long]), None)).contains("at most 40 characters")
        );
        assert_eq!(
            refusal(validate_options(&opts(&["a", "b"]), Some(3))),
            "--recommended must be between 1 and 2"
        );
        assert_eq!(
            refusal(validate_options(&opts(&["a", "b"]), Some(0))),
            "--recommended must be between 1 and 2"
        );
        assert_eq!(
            refusal(validate_options(&[], Some(1))),
            "--recommended needs options to choose from"
        );
    }

    #[test]
    fn answers_name_the_choice() {
        let q = HumanQuestion {
            id: "q".into(),
            text: "Which email provider?".into(),
            options: opts(&["Postmark", "SES", "Resend"]),
            recommended: Some(1),
            asked_at: 0,
        };
        assert_eq!(
            format_answer(&q, &Choice::Option(1)),
            "Answer to your question: Postmark (option 1)."
        );
        assert_eq!(
            format_answer(&q, &Choice::Other(" Mailgun, we have a contract ".into())),
            "Answer to your question: Mailgun, we have a contract"
        );
    }
}
