//! `crewboard prime`: the context a ticket's agent reads first (PLG-02; d0032, d0040).
//!
//! The ticket (title, What and why, Done when, base branch), its parents with the summaries their agents gave at
//! `review submit`, review feedback and pending questions (once M6 and M10 store them), Crewboard's own rules,
//! and the user's **Agent instructions**: the project's override if set, else the global text (d0040). The
//! output is concise Markdown for an agent ([`render`]); `--json` prints the [`PrimeContext`] itself.

use serde::{Deserialize, Serialize};

use crate::board::tickets::{Base, EventKind, Ticket};
use crate::board::{Board, BoardError};
use crate::domain::{Status, TicketKey};
use crate::naming;

/// Everything `prime` prints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimeContext {
    pub ticket: Ticket,
    /// The epic a child belongs to: its key and title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic: Option<(TicketKey, String)>,
    pub parents: Vec<Parent>,
    /// Sent-back review comments (M6); empty until then.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_feedback: Vec<String>,
    /// Questions from other agents waiting for this one (M10); empty until then.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_questions: Vec<String>,
    /// The Commit workflow skill to use (d0050): `/crewboard:commit` unless the user set their own.
    pub commit_skill: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<Instructions>,
}

/// A ticket this one depends on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parent {
    pub key: TicketKey,
    pub title: String,
    pub status: Status,
    /// What its agent said at `review submit` (the latest submission).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// The user's Agent instructions and where they come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instructions {
    /// The project's name (`shop-web`) when they are the project's override; `None` for the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub text: String,
}

impl Board {
    /// The prime context of a ticket.
    pub async fn prime_context(&self, key: &str) -> Result<PrimeContext, BoardError> {
        let ticket = self.ticket(key).await?;
        let project = ticket.key.project.to_string();
        let tickets = self.tickets(&project).await?;
        let mut parents = Vec::new();
        for key in &ticket.depends_on {
            let Some(parent) = tickets.iter().find(|t| &t.key == key) else {
                continue;
            };
            parents.push(Parent {
                key: parent.key.clone(),
                title: parent.title.clone(),
                status: parent.status,
                summary: self.review_summary(&parent.id).await?,
            });
        }
        let epic = ticket.epic.as_ref().and_then(|e| {
            tickets
                .iter()
                .find(|t| &t.key == e)
                .map(|t| (t.key.clone(), t.title.clone()))
        });
        let settings = self.effective_settings(&project).await?;
        let own = self.project_settings(&project).await?;
        let overridden = !own.agent_instructions.trim().is_empty();
        let instructions = if settings.agent_instructions.trim().is_empty() {
            None
        } else {
            Some(Instructions {
                project: match overridden {
                    true => Some(self.project(&project).await?.name),
                    false => None,
                },
                text: settings.agent_instructions.trim().to_owned(),
            })
        };
        Ok(PrimeContext {
            ticket,
            epic,
            parents,
            review_feedback: Vec::new(),
            pending_questions: Vec::new(),
            commit_skill: settings.workflow_skills.commit,
            instructions,
        })
    }

    /// The summary of a ticket's latest `review submit`.
    pub async fn review_summary(&self, ticket_id: &str) -> Result<Option<String>, BoardError> {
        let payload: Option<String> = sqlx::query_scalar(
            "SELECT payload_json FROM events WHERE ticket_id = ? AND kind = ? ORDER BY at DESC, id DESC LIMIT 1",
        )
        .bind(ticket_id)
        .bind(EventKind::ReviewSubmitted.as_str())
        .fetch_optional(self.store().pool())
        .await?;
        Ok(payload
            .and_then(|p| serde_json::from_str::<serde_json::Value>(&p).ok())
            .and_then(|p| p.get("summary")?.as_str().map(str::to_owned))
            .filter(|s| !s.trim().is_empty()))
    }
}

/// The Markdown an agent reads.
pub fn render(ctx: &PrimeContext) -> String {
    let t = &ctx.ticket;
    let cmd = naming::COMMAND_NAME;
    let mut out = format!("# {} · {}\n\n", t.key, t.title);
    let base = match &t.base {
        Base::Branch(b) => format!("`{b}`"),
        Base::Epic(e) => format!("the epic's branch ({e})"),
    };
    let branch = t
        .work
        .as_ref()
        .and_then(|w| w.branch.clone())
        .unwrap_or_else(|| naming::branch(&t.key.to_string()));
    out.push_str(&format!(
        "You are the agent for {} ticket {}, working in its own worktree on branch `{branch}`. It merges into \
         {base} once the user approves it.\n",
        naming::PRODUCT_NAME,
        t.key
    ));
    if let Some((key, title)) = &ctx.epic {
        out.push_str(&format!("It is part of the epic {key} · {title}.\n"));
    }

    out.push_str("\n## What and why\n\n");
    if t.body.trim().is_empty() {
        out.push_str("(no description)\n");
    } else {
        out.push_str(t.body.trim());
        out.push('\n');
    }

    out.push_str("\n## Done when\n\n");
    if t.done_when.is_empty() {
        out.push_str("(no criteria: agree on them with the user if the goal is unclear)\n");
    }
    for item in &t.done_when {
        out.push_str(&format!("- [ ] {}\n", item.text));
    }

    if !ctx.parents.is_empty() {
        out.push_str("\n## Depends on\n\n");
        for p in &ctx.parents {
            out.push_str(&format!(
                "- {} · {} ({})\n",
                p.key,
                p.title,
                p.status.label()
            ));
            match &p.summary {
                Some(summary) => {
                    for line in summary.trim().lines() {
                        out.push_str(&format!("  > {line}\n").replace("  > \n", "  >\n"));
                    }
                }
                None => out.push_str("  (no summary yet)\n"),
            }
        }
    }

    if !ctx.review_feedback.is_empty() {
        out.push_str("\n## Review feedback\n\n");
        for comment in &ctx.review_feedback {
            out.push_str(&format!("- {comment}\n"));
        }
    }
    if !ctx.pending_questions.is_empty() {
        out.push_str("\n## Pending questions\n\n");
        for question in &ctx.pending_questions {
            out.push_str(&format!("- {question}\n"));
        }
    }

    out.push_str(&format!("\n## {}'s rules\n\n", naming::PRODUCT_NAME));
    let rules = [
        "Follow the Agent instructions below. They add to these rules; they can't skip getting context or \
         submitting for review."
            .to_owned(),
        format!(
            "Commit everything with `{}` before you submit. Never push and never open a PR: the user does that.",
            ctx.commit_skill
        ),
        format!(
            "When the Done when list is met, submit: `{cmd} review submit --stdin` with a short summary of what you \
             changed on stdin. It is refused while files are uncommitted."
        ),
        format!(
            "When you are stuck or a decision is the user's, ask: `{cmd} ask human --stdin` with the question on \
             stdin, then end your turn."
        ),
        "Long text (summaries, questions, ticket bodies) always goes through stdin (`--stdin`), never quoted \
         arguments."
            .to_owned(),
        format!(
            "`{cmd} show <KEY>` reads any ticket; `{cmd} create \"Title\" --stdin` files a follow-up in Backlog."
        ),
    ];
    for (n, rule) in rules.iter().enumerate() {
        out.push_str(&format!("{}. {rule}\n", n + 1));
    }

    if let Some(instructions) = &ctx.instructions {
        let source = match &instructions.project {
            Some(project) => format!("project {project}"),
            None => "global".to_owned(),
        };
        out.push_str(&format!("\n## Agent instructions ({source})\n\n"));
        out.push_str(instructions.text.trim());
        out.push('\n');
    }
    out
}
