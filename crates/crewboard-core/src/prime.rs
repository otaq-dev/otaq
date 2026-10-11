//! `crewboard prime`: the context a ticket's agent reads first (PLG-02; d0032, d0040).
//!
//! The ticket (title, What and why, base branch), its parents with the summaries their agents gave at
//! `review submit`, review feedback and pending questions (once M6 and M10 store them), Crewboard's own rules,
//! and the user's **Agent instructions**: the project's override if set, else the global text (d0040). The
//! output is concise Markdown for an agent ([`render`]); `--json` prints the [`PrimeContext`] itself.

use serde::{Deserialize, Serialize};

use crate::board::conflicts::{Conflict, ConflictReason};
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
    /// The branch conflicts with the one it merges into: the agent resolves that first, with the restack skill
    /// (Send to agent to fix, or a reopened ticket's rebase; GIT-03, GIT-06).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<Conflict>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<Instructions>,
    /// The pull request being prepared or already open (REV-10, REV-14): for the Open PR and Push skills.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<PrimePr>,
}

/// The "Pull request" section: what the skill proposes from, and what the user confirmed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimePr {
    /// "GitHub", "Azure DevOps"; `None` when origin's host matches neither.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// origin, as Settings shows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    pub view: crate::pr::flow::PrFlowView,
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
        let conflict = self.conflict(&ticket.id).await?;
        let pull_request = self.prime_pr(&ticket).await?;
        // Questions from linked tickets' agents waiting for this one's answer (MSG-02).
        let cmd = naming::COMMAND_NAME;
        let pending_questions = self
            .pending_questions(&ticket.id)
            .await?
            .into_iter()
            .map(|m| {
                format!(
                    "{} from {}: {} \u{2014} answer with `{cmd} answer {} --stdin`",
                    m.id, m.from, m.question, m.id
                )
            })
            .collect();
        Ok(PrimeContext {
            conflict,
            ticket,
            epic,
            parents,
            review_feedback: Vec::new(),
            pending_questions,
            commit_skill: settings.workflow_skills.commit,
            instructions,
            pull_request,
        })
    }

    /// The "Pull request" section, while a PR flow runs or once the ticket has a PR.
    async fn prime_pr(&self, ticket: &Ticket) -> Result<Option<PrimePr>, BoardError> {
        let flowing = self.pr_flow(&ticket.id).await?.is_some();
        if !flowing && ticket.flags.pr_number.is_none() {
            return Ok(None);
        }
        let repo = self.project(ticket.key.project.as_str()).await?.repo_path;
        let url = crate::pr::provider::origin_url(self.git()?, std::path::Path::new(&repo)).await;
        Ok(Some(PrimePr {
            provider: url
                .as_deref()
                .and_then(crate::pr::provider::Provider::detect)
                .map(|p| p.name().to_owned()),
            remote: url.as_deref().map(crate::pr::provider::display_remote),
            view: self.pr_flow_view(&ticket.key.to_string()).await?,
        }))
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

/// What `crewboard prime --validate` prints for a validator (VAL-10, d0063): the ticket, the agent's submit
/// summary, the diff against the ticket's base, the user's review comments after a Send back, earlier rounds'
/// findings, the Agent instructions and the linked tickets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatePrime {
    pub ticket: Ticket,
    /// The round this validator judges, and the project's rounds.
    pub round: u32,
    pub rounds: u32,
    /// What the agent said at its submit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// `git diff <base_sha> HEAD` in the worktree: the committed change.
    pub diff: String,
    /// After a Send back: the user's note and line comments, each to check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<crate::board::comments::ReviewFeedback>,
    /// Earlier rounds' findings since the counter last restarted: (round, report).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub earlier_findings: Vec<(u32, String)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<Instructions>,
    /// Tickets this one depends on, with their submit summaries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parents: Vec<Parent>,
    /// Tickets that depend on this one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Linked>,
    /// The epic a child belongs to, with its description and the other children.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic: Option<EpicContext>,
}

/// A linked ticket: key, title, status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Linked {
    pub key: TicketKey,
    pub title: String,
    pub status: Status,
}

/// The epic around an epic child.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpicContext {
    pub key: TicketKey,
    pub title: String,
    pub description: String,
    pub siblings: Vec<Linked>,
}

impl Board {
    /// The context of a ticket's validator (`crewboard prime --validate`).
    pub async fn validate_prime(&self, key: &str) -> Result<ValidatePrime, BoardError> {
        let base = self.prime_context(key).await?;
        let ticket = base.ticket;
        let project = ticket.key.project.to_string();
        let tickets = self.tickets(&project).await?;
        let linked = |t: &Ticket| Linked {
            key: t.key.clone(),
            title: t.title.clone(),
            status: t.status,
        };
        let children = tickets
            .iter()
            .filter(|t| t.depends_on.contains(&ticket.key))
            .map(linked)
            .collect();
        let epic = ticket
            .epic
            .as_ref()
            .and_then(|e| tickets.iter().find(|t| &t.key == e))
            .map(|e| EpicContext {
                key: e.key.clone(),
                title: e.title.clone(),
                description: e.body.clone(),
                siblings: tickets
                    .iter()
                    .filter(|t| t.epic.as_ref() == Some(&e.key) && t.key != ticket.key)
                    .map(linked)
                    .collect(),
            });
        let rounds = self.effective_settings(&project).await?.validation_rounds;
        let earlier_findings: Vec<(u32, String)> = sqlx::query_as::<_, (i64, String)>(
            "SELECT round, report FROM validations WHERE ticket_id = ? AND outcome = 'fail' AND report IS NOT NULL
             AND round <= ? ORDER BY started_at",
        )
        .bind(&ticket.id)
        .bind(i64::from(ticket.validation_round))
        .fetch_all(self.store().pool())
        .await?
        .into_iter()
        .map(|(round, report)| (u32::try_from(round).unwrap_or(0), report))
        .collect();
        let review = if ticket.flags.sent_back.is_some() {
            Some(self.review_feedback(key).await?)
        } else {
            None
        };
        Ok(ValidatePrime {
            round: ticket.validation_round + 1,
            rounds,
            summary: self.review_summary(&ticket.id).await?,
            diff: self.committed_diff(&ticket).await?,
            review,
            earlier_findings,
            instructions: base.instructions,
            parents: base.parents,
            children,
            epic,
            ticket,
        })
    }

    /// `git diff <base_sha> HEAD` in the ticket's worktree; empty when it has none yet.
    async fn committed_diff(&self, ticket: &Ticket) -> Result<String, BoardError> {
        let Some(work) = &ticket.work else {
            return Ok(String::new());
        };
        let (Some(worktree), Some(base)) = (&work.worktree, &work.base_sha) else {
            return Ok(String::new());
        };
        Ok(self
            .git()?
            .run(std::path::Path::new(worktree), &["diff", base, "HEAD"])
            .await?)
    }
}

/// The Markdown a validator reads.
pub fn render_validate(ctx: &ValidatePrime) -> String {
    let t = &ctx.ticket;
    let cmd = naming::COMMAND_NAME;
    let mut out = format!("# Validate {} · {}\n\n", t.key, t.title);
    out.push_str(&format!(
        "You are the validator for {} ticket {}: a read-only judge of its agent's work (round {} of {}). Never \
         edit files, never commit and never fix anything.\n",
        naming::PRODUCT_NAME,
        t.key,
        ctx.round,
        ctx.rounds
    ));
    out.push_str("\n## What and why\n\n");
    out.push_str(if t.body.trim().is_empty() {
        "(no description)"
    } else {
        t.body.trim()
    });
    out.push('\n');
    out.push_str("\n## The agent's summary\n\n");
    out.push_str(ctx.summary.as_deref().unwrap_or("(none)").trim());
    out.push('\n');
    if let Some(review) = &ctx.review {
        out.push_str(&format!(
            "\n## The user's review (Send back, round {})\n\nCheck that each comment was addressed.\n\n",
            review.round
        ));
        if let Some(note) = &review.note {
            out.push_str(&format!("Note: {}\n", note.trim()));
        }
        for c in &review.comments {
            out.push_str(&format!("- {}:{}: {}\n", c.file, c.line, c.body.trim()));
        }
    }
    if !ctx.earlier_findings.is_empty() {
        out.push_str("\n## Earlier rounds' findings\n");
        for (round, report) in &ctx.earlier_findings {
            out.push_str(&format!("\nRound {round}:\n{}\n", report.trim()));
        }
    }
    if !ctx.parents.is_empty() || !ctx.children.is_empty() || ctx.epic.is_some() {
        out.push_str(
            "\n## Linked tickets\n\nA gap another of these covers is a note, not a failure.\n",
        );
        for p in &ctx.parents {
            out.push_str(&format!(
                "\n- depends on {} · {} ({})\n",
                p.key,
                p.title,
                p.status.label()
            ));
            if let Some(summary) = &p.summary {
                for line in summary.lines() {
                    out.push_str(&format!("  > {line}\n"));
                }
            }
        }
        for c in &ctx.children {
            out.push_str(&format!(
                "- needed by {} · {} ({})\n",
                c.key,
                c.title,
                c.status.label()
            ));
        }
        if let Some(epic) = &ctx.epic {
            out.push_str(&format!("\nEpic {} · {}:\n", epic.key, epic.title));
            if !epic.description.trim().is_empty() {
                out.push_str(epic.description.trim());
                out.push('\n');
            }
            for s in &epic.siblings {
                out.push_str(&format!(
                    "- sibling {} · {} ({})\n",
                    s.key,
                    s.title,
                    s.status.label()
                ));
            }
        }
    }
    if let Some(i) = &ctx.instructions {
        let from = match &i.project {
            Some(project) => format!("project {project}"),
            None => "global".to_owned(),
        };
        out.push_str(&format!("\n## Agent instructions ({from})\n\n{}\n", i.text));
    }
    out.push_str(&format!(
        "\n## The diff\n\n```diff\n{}\n```\n\n## Your verdict\n\nReport one line per finding with its file and \
         line; notes start with \"Note:\". Finish with `{cmd} validate pass --stdin` or `{cmd} validate fail --stdin` \
         and the report on stdin.\n",
        ctx.diff.trim_end()
    ));
    out
}

/// The "resolve this first" section: what conflicts, and the restack skill.
fn render_conflict(c: &Conflict) -> String {
    let restack = naming::skill("restack");
    let lead = match c.reason {
        ConflictReason::Reopen => format!(
            "Reopened: the rebase onto `{}` conflicted. Resolve it first with the restack skill (`{restack}`).",
            c.target
        ),
        ConflictReason::Merge => format!(
            "This branch conflicts with the latest `{}`. Rebase it onto `{}` and resolve the conflicts first, \
             with the restack skill (`{restack}`).",
            c.target, c.target
        ),
    };
    let mut out = format!("\n## Resolve first\n\n{lead}\n");
    if !c.files.is_empty() {
        out.push_str("\nConflicting files:\n");
        for file in &c.files {
            out.push_str(&format!("- {file}\n"));
        }
    }
    out
}

/// The "Pull request" section (REV-10): provider, target, branch, the commits to propose, and what the user
/// confirmed.
fn render_pr(pr: &PrimePr) -> String {
    use crate::pr::flow::{FlowKind, FlowState};
    let v = &pr.view;
    let mut out = String::from("\n## Pull request\n\n");
    let remote = pr.remote.as_deref().unwrap_or("none");
    match &pr.provider {
        Some(provider) => out.push_str(&format!("- Provider: {provider} (origin {remote})\n")),
        None => out.push_str(&format!(
            "- Provider: not detected (origin {remote}): the built-in skills only handle GitHub and Azure DevOps\n"
        )),
    }
    out.push_str(&format!("- Target branch: `{}`\n", v.target));
    if v.branch_locked {
        out.push_str(&format!(
            "- Branch: `{}` (pushed; it can't be renamed)\n",
            v.branch
        ));
    } else {
        out.push_str(&format!(
            "- Branch: `{}` (temporary; propose its real name)\n",
            v.branch
        ));
    }
    if let Some(id) = &v.external_id {
        out.push_str(&format!("- External ID: {id}\n"));
    }
    if let (Some(n), Some(url)) = (v.pr_number, &v.pr_url) {
        out.push_str(&format!("- PR: #{n} {url}\n"));
    }
    let kind = v.flow.as_ref().map(|f| f.kind);
    let pushed = v.commits.iter().filter(|c| c.pushed).count();
    if pushed > 0 {
        out.push_str("\nAlready on the PR (leave them as they are):\n");
        for c in v.commits.iter().filter(|c| c.pushed) {
            out.push_str(&format!("- {} {}\n", short(&c.sha), subject(&c.message)));
        }
    }
    let lead = match kind {
        Some(FlowKind::Push) => "New commits to propose messages for, oldest first:",
        _ => "Commits that go into the PR, oldest first (propose one message for each):",
    };
    out.push_str(&format!("\n{lead}\n"));
    for c in v.commits.iter().filter(|c| !c.pushed) {
        out.push_str(&format!("- {} {}\n", short(&c.sha), subject(&c.message)));
    }
    if let Some(flow) = &v.flow {
        match (flow.state, &flow.confirmed) {
            (FlowState::Pushing, Some(confirmed)) => {
                out.push_str(
                    &format!("\n**The user confirmed.** {} renamed the branch and reworded the commits. Push exactly this:\n\n", naming::PRODUCT_NAME),
                );
                out.push_str(&format!("- Branch: `{}`\n", confirmed.branch));
                if !confirmed.title.is_empty() {
                    out.push_str(&format!("- Title: {}\n", confirmed.title));
                }
                if !confirmed.description.is_empty() {
                    out.push_str("- Description:\n\n");
                    for line in confirmed.description.lines() {
                        out.push_str(&format!("  > {line}\n").replace("  > \n", "  >\n"));
                    }
                }
            }
            (FlowState::Proposed, _) => out.push_str(
                &format!("\nYour proposal is with the user. Wait: nothing is pushed until {} says they confirmed.\n", naming::PRODUCT_NAME),
            ),
            _ => out.push_str(&format!(
                "\nPropose with `{} pr propose --stdin`; don't push yet.\n",
                naming::COMMAND_NAME
            )),
        }
    }
    out
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

fn subject(message: &str) -> &str {
    message.lines().next().unwrap_or_default()
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
    out.push_str(&format!("Priority: {}.\n", t.priority));
    if let Some(conflict) = &ctx.conflict {
        out.push_str(&render_conflict(conflict));
    }

    out.push_str("\n## What and why\n\n");
    if t.body.trim().is_empty() {
        out.push_str("(no description)\n");
    } else {
        out.push_str(t.body.trim());
        out.push('\n');
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

    if let Some(pr) = &ctx.pull_request {
        out.push_str(&render_pr(pr));
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
            "When the ticket is done, as its description describes, submit: `{cmd} review submit --stdin` with a \
             short summary of what you changed on stdin. It is refused while files are uncommitted."
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
