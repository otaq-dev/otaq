//! The History tab (UI-04; d0045): every session the ticket had, with its time, duration, end reason and full
//! tool session id (copyable, so the tool's resume finds the conversation), and the ticket's events
//! (created, moved, start interrupted, held, retried, edited…), newest first; one entry per validation round, which
//! opens its report (VAL-12, `validation_report.rs`).

use otaq_core::board::epic_flight::EpicQueueMode;
use otaq_core::board::tickets::{EventKind, HistoryEvent};
use otaq_core::board::validation::ValidationRecord;
use otaq_core::domain::{Actor, StartStep, Status};
use otaq_core::session::{SessionInfo, SessionKind};
use otaq_core::usage::SessionUsage;

use crate::icons::IconName;
use crate::theme::Palette;
use crate::view_models::clock;
use crate::view_models::stage_choices::{self, HistoryLine};
use crate::view_models::validation_report::{self, Report};

/// One line of the History table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRow {
    pub title: String,
    pub detail: Option<String>,
    /// Unix milliseconds; shown as HH:MM.
    pub at: i64,
    /// "72 min", "running", or "—".
    pub duration: String,
    /// The session's tokens ("226k"); "—" for events, other sessions, and unknown counts (USG-01).
    pub tokens: String,
    /// The full tool session id, for the copy button.
    pub tool_session_id: Option<String>,
    pub is_session: bool,
    /// A validation round's outcome icon and colours; sessions and events have their own icons.
    pub outcome: Option<(IconName, Palette)>,
    /// A counted validation round opens its report.
    pub report: Option<Report>,
    /// A session row's session id (its title names the agent, [`with_agents`]).
    pub session: Option<String>,
}

/// "Built: only in the profile form · Other option: everywhere"; several other options read "Other options: a · b".
fn decision_detail(p: &serde_json::Value) -> String {
    let others: Vec<&str> = p
        .get("other_options")
        .and_then(|o| o.as_array())
        .map(|o| o.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let built = format!("Built: {}", text(p, "built").unwrap_or_default());
    match others.as_slice() {
        [] => built,
        [one] => format!("{built} \u{b7} Other option: {one}"),
        many => format!("{built} \u{b7} Other options: {}", many.join(" \u{b7} ")),
    }
}

/// Sessions and events merged, newest first, with each session's tokens from `usage`.
pub fn rows(
    events: &[HistoryEvent],
    sessions: &[SessionInfo],
    usage: &[SessionUsage],
) -> Vec<HistoryRow> {
    let mut rows: Vec<HistoryRow> = sessions.iter().map(|s| session_row(s, usage)).collect();
    rows.extend(events.iter().filter_map(event_row));
    // Newest first; a session row comes before an event at the same moment.
    rows.sort_by(|a, b| b.at.cmp(&a.at).then(b.is_session.cmp(&a.is_session)));
    rows
}

/// [`rows`] with one entry per validation round from `records` (in place of the rounds' bare events), `rounds`
/// being the project's Validation rounds.
pub fn rows_with_rounds(
    events: &[HistoryEvent],
    sessions: &[SessionInfo],
    usage: &[SessionUsage],
    records: &[ValidationRecord],
    rounds: u32,
) -> Vec<HistoryRow> {
    let events: Vec<HistoryEvent> = events
        .iter()
        .filter(|e| e.kind != EventKind::ValidationEnded)
        .cloned()
        .collect();
    let mut rows = rows(&events, sessions, usage);
    rows.extend(records.iter().map(|r| {
        let round = validation_report::round_row(r, rounds);
        HistoryRow {
            title: round.title,
            detail: round.detail,
            at: round.at,
            duration: round.duration,
            tokens: round.tokens,
            tool_session_id: None,
            is_session: false,
            outcome: Some((round.icon, round.palette)),
            report: round.report,
            session: None,
        }
    }));
    rows.sort_by(|a, b| b.at.cmp(&a.at).then(b.is_session.cmp(&a.is_session)));
    rows
}

fn session_row(s: &SessionInfo, usage: &[SessionUsage]) -> HistoryRow {
    let title = match s.kind {
        SessionKind::Agent => "Agent session",
        SessionKind::Answer => "Answer session",
        SessionKind::Delivery => "PR session",
        SessionKind::Shell => "Shell",
        SessionKind::Project => "Project terminal",
        SessionKind::Run => "Run",
        SessionKind::Validator => "Validator session",
        SessionKind::ProjectAgent => "Project agent",
    };
    let duration = match s.ended_at {
        Some(end) => clock::duration(end - s.started_at),
        None => "running".to_owned(),
    };
    // "claude 3be94c10… · service stopped": the tool's command and its id.
    let conversation = s
        .tool
        .zip(s.tool_session_id.as_deref())
        .map(|(tool, id)| format!("{} {id}", tool.command_name()));
    let detail = match (conversation, s.end_reason.as_deref()) {
        (Some(conversation), Some(reason)) => Some(format!("{conversation} · {reason}")),
        (Some(conversation), None) => Some(conversation),
        (None, Some(reason)) => Some(reason.to_owned()),
        (None, None) => None,
    };
    HistoryRow {
        title: title.to_owned(),
        detail,
        at: s.started_at,
        duration,
        tokens: crate::view_models::usage::session_label(usage, &s.id),
        tool_session_id: s.tool_session_id.clone(),
        is_session: true,
        outcome: None,
        report: None,
        session: Some(s.id.clone()),
    }
}

fn who(actor: &Actor) -> String {
    match actor {
        Actor::User(_) => "by you".to_owned(),
        Actor::Agent(key) => format!("by {key}'s agent"),
        Actor::ProjectAgent(_) => "by a Project agent".to_owned(),
    }
}

/// A decision's question cut short for a "Kept by you" line: "Split fullName …"; a short one stays whole.
fn short_question(question: &str) -> String {
    let question = question.trim();
    if question.chars().count() <= 24 {
        return question.to_owned();
    }
    let mut out = String::new();
    for word in question.split_whitespace() {
        let word = word.trim_end_matches([',', ';', ':']);
        if !out.is_empty() && out.chars().count() + 1 + word.chars().count() > 20 {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    format!("{out} \u{2026}")
}

/// "by Wishlist lead" when an epic lead made the change (its name is in the payload, CLI-07), else [`who`].
fn by(e: &HistoryEvent) -> String {
    match text(&e.payload, "lead") {
        Some(lead) => format!("by {lead}"),
        None => who(&e.actor),
    }
}

/// "description", "title · priority": the fields a lead's update changed, as History names them.
fn fields(names: impl Iterator<Item = String>) -> String {
    names
        .map(|f| match f.as_str() {
            "body" => "description".to_owned(),
            "base_branch" => "base branch".to_owned(),
            "external_id" => "External ID".to_owned(),
            _ => f,
        })
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

/// An epic lead's change of a child, on the epic (CLI-07, d0083).
fn lead_changed(p: &serde_json::Value) -> (String, Option<String>) {
    let lead = text(p, "lead").unwrap_or_else(|| "the lead".to_owned());
    let child = text(p, "child").unwrap_or_default();
    let parent = text(p, "parent").unwrap_or_default();
    let by = format!("by {lead}");
    match text(p, "change").as_deref() {
        Some("updated") => {
            let names = p
                .get("fields")
                .and_then(|f| f.as_array())
                .into_iter()
                .flatten()
                .filter_map(|f| f.as_str().map(str::to_owned));
            let changed = fields(names);
            (
                format!("Updated {child}"),
                Some(if changed.is_empty() {
                    by
                } else {
                    format!("{by} \u{b7} {changed}")
                }),
            )
        }
        Some("dep_added") => (format!("{child} waits for {parent}"), Some(by)),
        Some("dep_removed") => (format!("{child} no longer waits for {parent}"), Some(by)),
        Some("created") => (
            format!("Added {child} {}", text(p, "title").unwrap_or_default())
                .trim_end()
                .to_owned(),
            Some(by),
        ),
        _ => (format!("Changed {child}"), Some(by)),
    }
}

/// "SHOP-40.2 asked Wishlist lead", "Wishlist lead answered SHOP-40.2", "Wishlist lead passed SHOP-40.2's question
/// on to you", "SHOP-40.2's question handed back to you" (LEAD-06).
fn lead_question(p: &serde_json::Value) -> (String, Option<String>) {
    let child = text(p, "child").unwrap_or_default();
    let lead = text(p, "lead").unwrap_or_else(|| "the lead".to_owned());
    let quoted = |field: &str| {
        text(p, field).map(|t| format!("\u{201c}{}\u{201d}", t.lines().next().unwrap_or_default()))
    };
    match text(p, "step").as_deref() {
        Some("answered") => (format!("{lead} answered {child}"), quoted("answer")),
        Some("passed_on") => (
            format!("{lead} passed {child}'s question on to you"),
            Some(match (text(p, "recommended"), text(p, "reason")) {
                (Some(option), Some(reason)) => {
                    format!("recommended \u{201c}{option}\u{201d}: {reason}")
                }
                (None, Some(reason)) => reason,
                (_, None) => String::new(),
            })
            .filter(|d| !d.is_empty()),
        ),
        Some("handed_back") => (
            format!("{child}'s question handed back to you"),
            Some(format!("no answer from {lead}")),
        ),
        _ => (format!("{child} asked {lead}"), quoted("question")),
    }
}

/// A stuck child and its epic's lead (LEAD-07): "SHOP-40.3 stopped · sent to Wishlist lead" · "Validation failed 3
/// times", "Wishlist lead sent its note to SHOP-40.3", "SHOP-40.3's stop handed back to you" · "no note from
/// Wishlist lead".
fn lead_stuck(p: &serde_json::Value) -> (String, Option<String>) {
    let child = text(p, "child").unwrap_or_default();
    let lead = text(p, "lead").unwrap_or_else(|| "the lead".to_owned());
    match text(p, "step").as_deref() {
        Some("noted") => (
            format!("{lead} sent its note to {child}"),
            text(p, "reason"),
        ),
        Some("handed_back") => (
            format!("{child}'s stop handed back to you"),
            Some(format!("no note from {lead}")),
        ),
        _ => (
            format!("{child} stopped \u{b7} sent to {lead}"),
            text(p, "reason"),
        ),
    }
}

/// A stage choice (AGM-08): "Dev: Quick developer (set by the Project agent)" · "typo fix"; on an epic for a child,
/// "Wishlist lead set SHOP-40.3's Dev: Quick developer".
fn choice_line(p: &serde_json::Value) -> (String, Option<String>) {
    let stage = text(p, "stage").unwrap_or_default();
    let choice = text(p, "choice").unwrap_or_default();
    let by = text(p, "by").unwrap_or_else(|| "you".to_owned());
    let reason = text(p, "reason");
    match text(p, "child") {
        Some(child) => (format!("{by} set {child}'s {stage}: {choice}"), reason),
        None => (format!("{stage}: {choice} (set by {by})"), reason),
    }
}

/// `detail` with the stage Apply now's move came from, when it did: "by you · Security was removed" (APN-04).
fn removed(payload: &serde_json::Value, detail: String) -> String {
    match text(payload, "removed") {
        Some(stage) => format!(
            "{detail} \u{b7} {}",
            otaq_core::flow::apply_now::removed_note(&stage)
        ),
        None => detail,
    }
}

fn text(payload: &serde_json::Value, field: &str) -> Option<String> {
    payload
        .get(field)
        .and_then(|v| v.as_str())
        .map(str::to_owned)
}

fn event_row(e: &HistoryEvent) -> Option<HistoryRow> {
    let p = &e.payload;
    let first_line = |s: String| s.lines().next().unwrap_or_default().to_owned();
    let (title, detail) = match e.kind {
        // The session row stands for a successful start.
        EventKind::Started | EventKind::AgentEditsSeen => return None,
        // Ticket comments' lines (CMT-14) are drawn by T-0447; until then History leaves them out.
        EventKind::CommentPosted | EventKind::CommentEdited | EventKind::CommentDeleted => {
            return None;
        }
        EventKind::Created => ("Created".to_owned(), Some(by(e))),
        EventKind::Moved => {
            let to = text(p, "to").and_then(|s| s.parse::<Status>().ok());
            // A child queued by its epic's drag (d0080): "Queued with the epic · by you".
            let title = match to {
                Some(Status::Queued) if p.get("with_epic").is_some() => {
                    "Ready for Dev with the epic".to_owned()
                }
                Some(s) => s.label().to_owned(),
                None => "moved".to_owned(),
            };
            (title, Some(who(&e.actor)))
        }
        EventKind::StartInterrupted => {
            let step = text(p, "step")
                .and_then(|s| s.parse::<StartStep>().ok())
                .map_or("start stopped", StartStep::failure_label);
            (
                "Start failed".to_owned(),
                Some(match text(p, "message") {
                    Some(m) => format!("{step}: {}", first_line(m)),
                    None => step.to_owned(),
                }),
            )
        }
        EventKind::StartRetried => ("Retry start".to_owned(), Some(who(&e.actor))),
        EventKind::Held => ("Held".to_owned(), text(p, "message").map(first_line)),
        EventKind::StartNote => ("Note".to_owned(), text(p, "message").map(first_line)),
        EventKind::TemplateRefreshFailed => (
            "Template refresh failed".to_owned(),
            text(p, "message").map(first_line),
        ),
        EventKind::AgentRowSet => stage_choices::row_change_line(p),
        EventKind::StageChoiceSet => choice_line(p),
        // FLW-05: "Quick developer is no longer an option for Dev · runs Rust developer".
        EventKind::ChoiceDropped => (
            otaq_core::flow::apply::dropped_choice_line(
                &text(p, "chose").unwrap_or_default(),
                &text(p, "stage").unwrap_or_default(),
                &text(p, "runs").unwrap_or_default(),
            ),
            None,
        ),
        EventKind::FallbackUsed => (stage_choices::fallback_line(p), None),
        EventKind::SchemaOverlap => (
            match text(p, "other") {
                Some(other) => format!("Schema path shared with {other}"),
                None => "Schema path shared".to_owned(),
            },
            text(p, "path"),
        ),
        // An epic lead's edit names it and the fields: "Updated by Wishlist lead · description" (CLI-07).
        EventKind::Updated => match text(p, "lead") {
            Some(lead) => {
                let names = p
                    .get("changes")
                    .and_then(|c| c.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|c| text(c, "field"));
                let changed = fields(names);
                (
                    format!("Updated by {lead}"),
                    (!changed.is_empty()).then_some(changed),
                )
            }
            None => ("Edited".to_owned(), Some(who(&e.actor))),
        },
        EventKind::Pinned => ("Pinned".to_owned(), None),
        EventKind::Unpinned => ("Unpinned".to_owned(), None),
        EventKind::DependencyAdded => (
            "Dependency added".to_owned(),
            text(p, "parent").map(|k| match text(p, "lead") {
                Some(lead) => format!("depends on {k} \u{b7} by {lead}"),
                None => format!("depends on {k}"),
            }),
        ),
        EventKind::DependencyRemoved => (
            "Dependency removed".to_owned(),
            text(p, "parent").map(|k| match text(p, "lead") {
                Some(lead) => format!("no longer depends on {k} \u{b7} by {lead}"),
                None => format!("no longer depends on {k}"),
            }),
        ),
        EventKind::MovedToEpic => ("Moved to epic".to_owned(), text(p, "epic")),
        // `otaq stage done` (AGM-10): the agent's reason.
        EventKind::Closed if p.get("nothing_to_merge").is_some() => (
            "Closed \u{b7} nothing to merge".to_owned(),
            text(p, "reason").map(first_line),
        ),
        EventKind::Closed => ("Closed".to_owned(), Some(who(&e.actor))),
        EventKind::ReviewSubmitted => (
            "Submitted for review".to_owned(),
            text(p, "summary").map(first_line),
        ),
        // An Agent stage's `stage next` into anything but a human check (Q263): "Finished Dev".
        EventKind::StageFinished => (
            format!("Finished {}", text(p, "stage").unwrap_or_default()),
            text(p, "summary").map(first_line),
        ),
        EventKind::Merged => (
            "Merged".to_owned(),
            text(p, "target").map(|t| format!("into {t}")),
        ),
        EventKind::Reopened => ("Reopened".to_owned(), Some(who(&e.actor))),
        // A human check's Pass (FLW-14): "Passed QA" · "by you".
        EventKind::Passed => (
            format!("Passed {}", text(p, "stage").unwrap_or_default()),
            Some(who(&e.actor)),
        ),
        EventKind::Parked => ("Parked".to_owned(), Some(removed(p, who(&e.actor)))),
        // A drag to another stage (FLW-18): "Moved back to Dev" · "Grill · run by you", the note, or "by you";
        // "Moved to Review" · "Skipped Validate · by you". Apply now's moves add the stage that went (APN-04):
        // "Moved to Review" · "by you · Security was removed".
        EventKind::Dragged => {
            let stage = text(p, "stage").unwrap_or_default();
            let skipped: Vec<&str> = p
                .get("skipped")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            let flag = |field: &str| p.get(field).and_then(|v| v.as_bool()) == Some(true);
            let title = match flag("back") {
                true => format!("Moved back to {stage}"),
                false => format!("Moved to {stage}"),
            };
            let detail = if flag("run_by_you") {
                format!("{stage} \u{b7} run by you")
            } else if !skipped.is_empty() {
                format!("Skipped {} \u{b7} by you", skipped.join(", "))
            } else {
                text(p, "note")
                    .map(first_line)
                    .unwrap_or_else(|| who(&e.actor))
            };
            (title, Some(removed(p, detail)))
        }
        EventKind::Rebased => (
            "Rebased".to_owned(),
            text(p, "onto").map(|b| match p.get("conflict") {
                Some(_) => format!("onto {b}: conflicts, the agent resolves them first"),
                None => format!("onto {b}"),
            }),
        ),
        // A Delivery's conflict went to the agent with restack, no click (FLW-17).
        EventKind::SentToFix => (
            "Merge conflict \u{b7} sent to the agent".to_owned(),
            text(p, "target").map(|t| format!("conflicts with {t}")),
        ),
        EventKind::SentBackWithPrComments => (
            "Sent back with PR comments".to_owned(),
            text(p, "note").filter(|n| !n.is_empty()).map(first_line),
        ),
        // *16 → History · Claude session IDs*: "Interrupted by restart · conversation saved".
        EventKind::Interrupted => match text(p, "reason").as_deref() {
            Some("restart") | None => (
                "Interrupted by restart".to_owned(),
                Some("conversation saved".to_owned()),
            ),
            // "codex exited (crash) · conversation saved": the session's tool, by its command (Q77).
            Some(reason) => {
                let program = text(p, "tool")
                    .and_then(|t| t.parse::<otaq_core::agent_tools::Tool>().ok())
                    .map_or("the agent", |t| t.command_name());
                (
                    "Interrupted".to_owned(),
                    Some(format!("{program} exited ({reason}) · conversation saved")),
                )
            }
        },
        EventKind::SessionEnded => ("Session ended (restart)".to_owned(), None),
        EventKind::Resumed => (
            if p.get("retry").is_some() {
                "Retry resume"
            } else {
                "Resumed"
            }
            .to_owned(),
            Some(format!("same conversation · {}", who(&e.actor))),
        ),
        EventKind::ResumeFailed => (
            "Resume failed".to_owned(),
            Some(match text(p, "tool_session_id") {
                Some(id) => format!("conversation {id} not found"),
                None => "conversation not found".to_owned(),
            }),
        ),
        EventKind::LimitMarked => (
            "Marked as usage limit".to_owned(),
            Some(format!("waiting for limit reset · {}", who(&e.actor))),
        ),
        EventKind::StartedFromScratch => (
            "Started from scratch (resume failed)".to_owned(),
            Some(match text(p, "old_tool_session_id") {
                Some(id) => format!("same worktree · old conversation {id}"),
                None => "same worktree".to_owned(),
            }),
        ),
        EventKind::BranchRenamed => (
            "Branch renamed".to_owned(),
            match (text(p, "from"), text(p, "to")) {
                (Some(from), Some(to)) => Some(format!("{from} \u{2192} {to}")),
                _ => None,
            },
        ),
        EventKind::PrProposed => ("PR proposed".to_owned(), text(p, "branch")),
        EventKind::PrConfirmed => (
            match text(p, "kind").as_deref() {
                Some("push") => "Push confirmed",
                _ => "PR confirmed",
            }
            .to_owned(),
            Some(format!(
                "{} \u{b7} {}",
                text(p, "branch").unwrap_or_default(),
                who(&e.actor)
            )),
        ),
        EventKind::PrCancelled => ("PR cancelled".to_owned(), Some(who(&e.actor))),
        EventKind::QuestionAsked => match (text(p, "to"), text(p, "from")) {
            (Some(to), _) => (format!("Asked {to}"), text(p, "question").map(first_line)),
            (_, Some(from)) => (format!("{from} asked"), text(p, "question").map(first_line)),
            _ => ("Question".to_owned(), text(p, "question").map(first_line)),
        },
        EventKind::QuestionAnswered => (
            match text(p, "with") {
                Some(with) => format!("Question answered \u{b7} {with}"),
                None => "Question answered".to_owned(),
            },
            text(p, "answer").map(first_line),
        ),
        EventKind::MergedAutomatically => (
            otaq_core::domain::validation::MERGED_AUTOMATICALLY.to_owned(),
            text(p, "target").map(
                |t| match p.get("without_checks").and_then(|w| w.as_bool()) {
                    Some(true) => format!(
                        "into {t} \u{b7} {}",
                        otaq_core::domain::validation::DELIVERED_WITHOUT_CHECKS
                    ),
                    _ => format!("into {t}"),
                },
            ),
        ),
        EventKind::AutoMergeStopped => ("Delivery stopped".to_owned(), text(p, "message")),
        // An epic's queue mode (d0080).
        EventKind::EpicQueueModeSet => (
            match text(p, "to")
                .and_then(|m| m.parse::<EpicQueueMode>().ok())
                .unwrap_or_default()
            {
                EpicQueueMode::Whole => "Queued the whole epic",
                EpicQueueMode::Single => "Queued one child",
                EpicQueueMode::None => "Taken back",
            }
            .to_owned(),
            Some(who(&e.actor)),
        ),
        // A new child of a whole-queued epic (EPC-13).
        EventKind::JoinedQueue => (
            "Joined the queue".to_owned(),
            text(p, "epic").map(|epic| format!("{epic} is queued as a whole")),
        ),
        // *05 · Ticket side panel → Row · Recorded decisions (0084)*.
        EventKind::DecisionRecorded => (
            format!(
                "Recorded a decision: {}",
                text(p, "question").unwrap_or_default()
            ),
            Some(decision_detail(p)),
        ),
        // Approve kept them (DEC-05; *05 → Row · Recorded decisions (0084)*).
        EventKind::DecisionsKept => {
            let count = p.get("count").and_then(|c| c.as_u64()).unwrap_or(0);
            let kept: Vec<String> = p
                .get("decisions")
                .and_then(|d| d.as_array())
                .map(|d| {
                    d.iter()
                        .map(|d| {
                            format!(
                                "{} \u{2192} {}",
                                short_question(&text(d, "question").unwrap_or_default()),
                                text(d, "built").unwrap_or_default()
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            (
                format!(
                    "Kept {} \u{b7} {count} decision{}",
                    who(&e.actor),
                    if count == 1 { "" } else { "s" }
                ),
                (!kept.is_empty()).then(|| kept.join(" \u{b7} ")),
            )
        }
        // Change… (DEC-06): sent back, or a fix child in the epic's review.
        EventKind::DecisionChanged => {
            let changed = format!("Changed {}", who(&e.actor));
            match text(p, "child") {
                Some(child) => (
                    format!("{changed} \u{2192} {child}"),
                    Some(format!(
                        "\u{201c}{}\u{201d} \u{b7} added in {}'s review",
                        text(p, "title").unwrap_or_default(),
                        text(p, "epic").unwrap_or_default()
                    )),
                ),
                None => (
                    format!("{changed} \u{2192} sent back"),
                    Some(format!(
                        "\u{201c}{}\u{201d} \u{b7} round {} starts",
                        text(p, "text").unwrap_or_default(),
                        p.get("round").and_then(|r| r.as_u64()).unwrap_or(0)
                    )),
                ),
            }
        }
        // The epic's lead ended at Ready for Review (d0083).
        EventKind::LeadEnded => (
            "Lead ended \u{b7} epic in review".to_owned(),
            text(p, "lead"),
        ),
        EventKind::LeadChanged => lead_changed(p),
        // "Suggested closing SHOP-40.6 · closed by you" (LEAD-08).
        EventKind::LeadSuggestion => {
            let verb = match text(p, "kind").as_deref() {
                Some("delete") => "deleting",
                _ => "closing",
            };
            (
                format!("Suggested {verb} {}", text(p, "child").unwrap_or_default()),
                Some(format!(
                    "{} {}",
                    text(p, "outcome").unwrap_or_default(),
                    who(&e.actor)
                )),
            )
        }
        // A child's question and its epic's lead (LEAD-06), on the child and the epic.
        EventKind::LeadQuestion => lead_question(p),
        // A stuck child and its epic's lead (LEAD-07), on the child and the epic.
        EventKind::LeadStuck => lead_stuck(p),
        // "Marked by Wishlist lead · contradicts PAY-123: “…”" (DEC-04).
        EventKind::DecisionMarked => (
            format!(
                "Marked by {}",
                text(p, "lead").unwrap_or_else(|| "the lead".to_owned())
            ),
            Some(match text(p, "source") {
                Some(source) => format!(
                    "contradicts {source}: \u{201c}{}\u{201d}",
                    text(p, "text").unwrap_or_default()
                ),
                None => format!("\u{201c}{}\u{201d}", text(p, "text").unwrap_or_default()),
            }),
        ),
        // "Note from Wishlist lead" · "to SHOP-40.4: “…”" (LEAD-10), on the child and the epic.
        EventKind::LeadNote => (
            format!(
                "Note from {}",
                text(p, "lead").unwrap_or_else(|| "the lead".to_owned())
            ),
            Some(format!(
                "to {}: \u{201c}{}\u{201d}",
                text(p, "child").unwrap_or_default(),
                text(p, "text").unwrap_or_default()
            )),
        ),
        // One row per validation round; T-0195 opens its report.
        EventKind::ValidationEnded => (
            format!(
                "Validation round {} \u{b7} {}",
                p.get("round").and_then(|r| r.as_u64()).unwrap_or(0),
                text(p, "outcome").unwrap_or_default()
            ),
            None,
        ),
        EventKind::PrOpened => (
            match p.get("number").and_then(|n| n.as_u64()) {
                Some(n) => format!("PR #{n} opened"),
                None => "PR opened".to_owned(),
            },
            text(p, "url"),
        ),
        EventKind::SentBack => {
            let n = p.get("comments").and_then(|v| v.as_u64()).unwrap_or(0);
            let round = p.get("round").and_then(|v| v.as_u64()).unwrap_or(0);
            let plural = if n == 1 { "" } else { "s" };
            (
                "Sent back".to_owned(),
                Some(format!("{n} comment{plural} · round {round}")),
            )
        }
    };
    Some(HistoryRow {
        title,
        detail,
        at: e.at,
        duration: "—".to_owned(),
        tokens: "—".to_owned(),
        tool_session_id: None,
        is_session: false,
        outcome: None,
        report: None,
        session: None,
    })
}

/// The named agents in History (AGT-05, d0086): each session's row titled "Session started on Sonnet ·
/// claude-sonnet-5-6" (with "(Sonnet → claude-sonnet-5-6)" when it moved on; with several tools "Session started on
/// Verifier · Copilot · auto", the swap note before its session id), and its end and a Done ticket's agents as lines
/// of their own, newest first like the rest.
pub fn with_agents(mut rows: Vec<HistoryRow>, lines: &[HistoryLine]) -> Vec<HistoryRow> {
    for line in lines {
        match line {
            HistoryLine::Start {
                session,
                title,
                note,
            } => {
                if let Some(row) = rows
                    .iter_mut()
                    .find(|r| r.session.as_ref() == Some(session))
                {
                    row.title = title.clone();
                    // "asked for gpt-5.5, ran on auto · copilot c7f1d0…" (AGT-05).
                    if let Some(note) = note {
                        row.detail = Some(match row.detail.take() {
                            Some(detail) => format!("{note} \u{b7} {detail}"),
                            None => note.clone(),
                        });
                    }
                }
            }
            HistoryLine::Line { at, title, tokens } => rows.push(HistoryRow {
                title: title.clone(),
                detail: None,
                at: *at,
                duration: "\u{2014}".to_owned(),
                tokens: tokens.clone(),
                tool_session_id: None,
                is_session: false,
                outcome: None,
                report: None,
                session: None,
            }),
        }
    }
    rows.sort_by(|a, b| b.at.cmp(&a.at).then(b.is_session.cmp(&a.is_session)));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use otaq_core::agent_tools::Tool;
    use otaq_core::session::SessionState;
    use serde_json::json;

    fn session(id: &str, start: i64, end: Option<i64>, reason: Option<&str>) -> SessionInfo {
        SessionInfo {
            id: id.into(),
            kind: SessionKind::Agent,
            project: "SHOP".into(),
            ticket: Some("SHOP-28".parse().unwrap()),
            state: if end.is_some() {
                SessionState::Ended
            } else {
                SessionState::Running
            },
            started_at: start,
            ended_at: end,
            end_reason: reason.map(str::to_owned),
            tool: Some(Tool::Claude),
            tool_session_id: Some(format!("3be94c10-{id}")),
            model: None,
            agent: None,
            effort: None,
            fallback: None,
            model_ran: None,
            size: None,
        }
    }

    fn event(kind: EventKind, at: i64, payload: serde_json::Value) -> HistoryEvent {
        HistoryEvent {
            kind,
            actor: Actor::User("me".into()),
            at,
            payload,
        }
    }

    const MIN: i64 = 60_000;

    /// One entry per validation round in place of its bare event, sorted with the rest (VAL-12).
    #[test]
    fn validation_rounds_replace_their_events() {
        use otaq_core::domain::ValidationOutcome;
        use otaq_core::domain::validation::ValidationTrigger;
        let record = ValidationRecord {
            round: 1,
            trigger: ValidationTrigger::Agent,
            outcome: Some(ValidationOutcome::Fail),
            started_at: 20 * MIN,
            ended_at: Some(24 * MIN),
            duration_ms: Some(4 * MIN),
            session: None,
            report: Some("a.ts:1: wrong".into()),
            notes: Vec::new(),
            tokens: None,
        };
        let rows = rows_with_rounds(
            &[
                event(EventKind::Created, 0, json!({})),
                event(
                    EventKind::ValidationEnded,
                    24 * MIN,
                    json!({ "round": 1, "outcome": "fail" }),
                ),
            ],
            &[],
            &[],
            &[record],
            3,
        );
        let titles: Vec<_> = rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, ["Round 1 · failed", "Created"]);
        assert_eq!(rows[0].outcome, Some((IconName::ShieldX, Palette::Attn)));
        assert!(rows[0].report.is_some());
        assert_eq!(rows[1].report, None);
    }

    /// An epic's queue mode reads as what the user did (d0080).
    #[test]
    fn epic_queue_modes_read_as_queued_or_taken_back() {
        let rows = rows(
            &[
                event(
                    EventKind::EpicQueueModeSet,
                    MIN,
                    json!({"from": "none", "to": "whole"}),
                ),
                event(
                    EventKind::EpicQueueModeSet,
                    2 * MIN,
                    json!({"from": "whole", "to": "none"}),
                ),
                event(
                    EventKind::EpicQueueModeSet,
                    3 * MIN,
                    json!({"from": "none", "to": "single"}),
                ),
            ],
            &[],
            &[],
        );
        let titles: Vec<_> = rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Queued one child", "Taken back", "Queued the whole epic"]
        );
    }

    /// A recorded decision (DEC-02; *05 · Ticket side panel → Row · Recorded decisions (0084)*).
    #[test]
    fn a_recorded_decision_reads_as_the_frame() {
        let rows = rows(
            &[
                event(
                    EventKind::DecisionRecorded,
                    MIN,
                    json!({
                        "ordinal": 1,
                        "question": "Split fullName everywhere, or only in the profile form?",
                        "built": "only in the profile form",
                        "other_options": ["everywhere"],
                        "why": "the description only mentions the profile form",
                    }),
                ),
                event(
                    EventKind::DecisionRecorded,
                    2 * MIN,
                    json!({
                        "ordinal": 2,
                        "question": "Order of saved items?",
                        "built": "newest first",
                        "other_options": ["oldest first", "by price"],
                        "why": "w",
                    }),
                ),
            ],
            &[],
            &[],
        );
        assert_eq!(
            rows[1].title,
            "Recorded a decision: Split fullName everywhere, or only in the profile form?"
        );
        assert_eq!(
            rows[1].detail.as_deref(),
            Some("Built: only in the profile form \u{b7} Other option: everywhere")
        );
        assert_eq!(
            rows[0].detail.as_deref(),
            Some("Built: newest first \u{b7} Other options: oldest first \u{b7} by price")
        );
    }

    /// Approve kept them, and Change… sent back or added a fix child (DEC-05, DEC-06; *05 → Row · Recorded
    /// decisions (0084)*).
    #[test]
    fn kept_and_changed_decisions_read_as_the_frame() {
        let rows = rows(
            &[
                event(
                    EventKind::DecisionsKept,
                    MIN,
                    json!({
                        "count": 1,
                        "decisions": [{
                            "ordinal": 1,
                            "question": "Split fullName everywhere, or only in the profile form?",
                            "built": "only in the profile form",
                        }],
                    }),
                ),
                event(
                    EventKind::DecisionChanged,
                    2 * MIN,
                    json!({
                        "ordinal": 1,
                        "question": "Split fullName everywhere, or only in the profile form?",
                        "to": "sent_back",
                        "text": "Split fullName everywhere",
                        "round": 2,
                    }),
                ),
                event(
                    EventKind::DecisionChanged,
                    3 * MIN,
                    json!({
                        "ordinal": 1,
                        "question": "Order of saved items?",
                        "to": "fix_child",
                        "child": "SHOP-40.7",
                        "title": "Change: list wishlist items in the order they were added",
                        "epic": "SHOP-40",
                    }),
                ),
            ],
            &[],
            &[],
        );
        assert_eq!(rows[2].title, "Kept by you \u{b7} 1 decision");
        assert_eq!(
            rows[2].detail.as_deref(),
            Some("Split fullName \u{2026} \u{2192} only in the profile form")
        );
        assert_eq!(rows[1].title, "Changed by you \u{2192} sent back");
        assert_eq!(
            rows[1].detail.as_deref(),
            Some("\u{201c}Split fullName everywhere\u{201d} \u{b7} round 2 starts")
        );
        assert_eq!(rows[0].title, "Changed by you \u{2192} SHOP-40.7");
        assert_eq!(
            rows[0].detail.as_deref(),
            Some(
                "\u{201c}Change: list wishlist items in the order they were added\u{201d} \u{b7} added in SHOP-40's review"
            )
        );
        assert_eq!(
            short_question("Order of saved items?"),
            "Order of saved items?"
        );
    }

    /// T-0280 (CLI-07, LEAD-08, DEC-04; d0083): the lead's changes on the child and the epic, a suggestion's outcome
    /// and the lead's mark.
    #[test]
    fn the_leads_rows_name_it() {
        let lead_event = |kind: EventKind, payload: serde_json::Value| HistoryEvent {
            actor: "project-agent:l1".parse().unwrap(),
            ..event(kind, MIN, payload)
        };
        let row = |e: HistoryEvent| {
            let rows = rows(&[e], &[], &[]);
            (rows[0].title.clone(), rows[0].detail.clone())
        };
        let cases = [
            (
                lead_event(
                    EventKind::Updated,
                    json!({"lead": "Wishlist lead", "changes": [{"field": "body", "before": "a", "after": "b"}]}),
                ),
                ("Updated by Wishlist lead", Some("description")),
            ),
            (
                lead_event(EventKind::Updated, json!({"changes": []})),
                ("Edited", Some("by a Project agent")),
            ),
            (
                lead_event(
                    EventKind::DependencyAdded,
                    json!({"lead": "Wishlist lead", "parent": "SHOP-40.4"}),
                ),
                (
                    "Dependency added",
                    Some("depends on SHOP-40.4 \u{b7} by Wishlist lead"),
                ),
            ),
            (
                lead_event(EventKind::Created, json!({"lead": "Wishlist lead"})),
                ("Created", Some("by Wishlist lead")),
            ),
            (
                lead_event(
                    EventKind::LeadChanged,
                    json!({"lead": "Wishlist lead", "child": "SHOP-40.3", "change": "updated",
                           "fields": ["description", "priority"]}),
                ),
                (
                    "Updated SHOP-40.3",
                    Some("by Wishlist lead \u{b7} description \u{b7} priority"),
                ),
            ),
            (
                lead_event(
                    EventKind::LeadChanged,
                    json!({"lead": "Wishlist lead", "child": "SHOP-40.3", "change": "dep_added",
                           "parent": "SHOP-40.4"}),
                ),
                ("SHOP-40.3 waits for SHOP-40.4", Some("by Wishlist lead")),
            ),
            (
                lead_event(
                    EventKind::LeadChanged,
                    json!({"lead": "Wishlist lead", "child": "SHOP-40.5", "change": "created",
                           "title": "Wishlist empty state"}),
                ),
                (
                    "Added SHOP-40.5 Wishlist empty state",
                    Some("by Wishlist lead"),
                ),
            ),
            (
                event(
                    EventKind::LeadSuggestion,
                    MIN,
                    json!({"lead": "Wishlist lead", "child": "SHOP-40.6", "kind": "close",
                           "outcome": "closed", "reason": "r"}),
                ),
                ("Suggested closing SHOP-40.6", Some("closed by you")),
            ),
            (
                event(
                    EventKind::LeadSuggestion,
                    MIN,
                    json!({"child": "SHOP-40.6", "kind": "delete", "outcome": "dismissed"}),
                ),
                ("Suggested deleting SHOP-40.6", Some("dismissed by you")),
            ),
            (
                lead_event(
                    EventKind::DecisionMarked,
                    json!({"ordinal": 1, "lead": "Wishlist lead", "source": "PAY-123",
                           "text": "items are listed in the order they were added"}),
                ),
                (
                    "Marked by Wishlist lead",
                    Some(
                        "contradicts PAY-123: \u{201c}items are listed in the order they were added\u{201d}",
                    ),
                ),
            ),
            (
                lead_event(
                    EventKind::LeadNote,
                    json!({"lead": "Wishlist lead", "child": "SHOP-40.4",
                           "text": "Use the same order on the shared list."}),
                ),
                (
                    "Note from Wishlist lead",
                    Some("to SHOP-40.4: \u{201c}Use the same order on the shared list.\u{201d}"),
                ),
            ),
            (
                lead_event(
                    EventKind::DecisionMarked,
                    json!({"ordinal": 1, "lead": "Wishlist lead", "text": "the page sorts the other way"}),
                ),
                (
                    "Marked by Wishlist lead",
                    Some("\u{201c}the page sorts the other way\u{201d}"),
                ),
            ),
        ];
        for (e, (title, detail)) in cases {
            assert_eq!(row(e), (title.to_owned(), detail.map(str::to_owned)));
        }
    }

    /// A new child of a whole-queued epic (EPC-13).
    /// T-0406 (Q263, AGM-10): "Finished Grill" and "Finished Dev" for stages that end on their way to anything but
    /// a human check, "Submitted for review" before one, and `stage done`'s "Closed · nothing to merge" with the
    /// reason.
    #[test]
    fn how_stages_ended() {
        let rows = rows(
            &[
                event(
                    EventKind::StageFinished,
                    MIN,
                    json!({"stage": "Grill", "stage_id": "grill", "summary": "Agreed: CSV.\nWith a header."}),
                ),
                event(
                    EventKind::StageFinished,
                    2 * MIN,
                    json!({"stage": "Dev", "stage_id": "dev", "summary": "CSV export."}),
                ),
                event(
                    EventKind::ReviewSubmitted,
                    3 * MIN,
                    json!({"summary": "CSV export, sorted."}),
                ),
                event(
                    EventKind::Closed,
                    4 * MIN,
                    json!({"from": "working", "nothing_to_merge": true, "reason": "Already fixed in SHOP-31."}),
                ),
            ],
            &[],
            &[],
        );
        let lines: Vec<_> = rows
            .iter()
            .map(|r| (r.title.as_str(), r.detail.as_deref()))
            .collect();
        assert_eq!(
            lines,
            [
                (
                    "Closed \u{b7} nothing to merge",
                    Some("Already fixed in SHOP-31.")
                ),
                ("Submitted for review", Some("CSV export, sorted.")),
                ("Finished Dev", Some("CSV export.")),
                ("Finished Grill", Some("Agreed: CSV.")),
            ]
        );
    }

    #[test]
    fn a_joined_child_says_why() {
        let rows = rows(
            &[event(
                EventKind::JoinedQueue,
                MIN,
                json!({"epic": "SHOP-40", "from": "backlog", "to": "queued"}),
            )],
            &[],
            &[],
        );
        assert_eq!(rows[0].title, "Joined the queue");
        assert_eq!(
            rows[0].detail.as_deref(),
            Some("SHOP-40 is queued as a whole")
        );
    }

    /// T-0285 (LEAD-07): a stuck child and its lead, the same rows on the child and the epic.
    #[test]
    fn a_stuck_child_and_its_lead() {
        let row = |step: &str| {
            let payload = json!({"child": "SHOP-40.3", "lead": "Wishlist lead", "stop": "validation_failed",
                                 "reason": "Validation failed 3 times", "step": step});
            let rows = rows(&[event(EventKind::LeadStuck, MIN, payload)], &[], &[]);
            (rows[0].title.clone(), rows[0].detail.clone())
        };
        assert_eq!(
            row("held"),
            (
                "SHOP-40.3 stopped \u{b7} sent to Wishlist lead".to_owned(),
                Some("Validation failed 3 times".to_owned())
            )
        );
        assert_eq!(
            row("noted"),
            (
                "Wishlist lead sent its note to SHOP-40.3".to_owned(),
                Some("Validation failed 3 times".to_owned())
            )
        );
        assert_eq!(
            row("handed_back"),
            (
                "SHOP-40.3's stop handed back to you".to_owned(),
                Some("no note from Wishlist lead".to_owned())
            )
        );
    }

    /// T-0284 (LEAD-06): a child's question and its lead, the same rows on the child and the epic.
    #[test]
    fn a_question_to_the_lead_names_both() {
        let row = |payload: serde_json::Value| {
            let rows = rows(&[event(EventKind::LeadQuestion, MIN, payload)], &[], &[]);
            (rows[0].title.clone(), rows[0].detail.clone())
        };
        let base = json!({"id": "7", "child": "SHOP-40.2", "lead": "Wishlist lead",
                          "question": "Can guests save items without logging in?"});
        let with = |extra: serde_json::Value| {
            let mut p = base.clone();
            p.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            p
        };
        let cases = [
            (
                with(json!({"step": "asked"})),
                (
                    "SHOP-40.2 asked Wishlist lead",
                    Some("\u{201c}Can guests save items without logging in?\u{201d}"),
                ),
            ),
            (
                with(
                    json!({"step": "answered", "answer": "Yes: filled heart means saved.\nMore."}),
                ),
                (
                    "Wishlist lead answered SHOP-40.2",
                    Some("\u{201c}Yes: filled heart means saved.\u{201d}"),
                ),
            ),
            (
                with(
                    json!({"step": "passed_on", "recommended": "Logged-in users only",
                            "reason": "PAY-123 only mentions signed-in users."}),
                ),
                (
                    "Wishlist lead passed SHOP-40.2's question on to you",
                    Some(
                        "recommended \u{201c}Logged-in users only\u{201d}: PAY-123 only mentions signed-in users.",
                    ),
                ),
            ),
            (
                with(
                    json!({"step": "passed_on", "recommended": null, "reason": "Not in the plan."}),
                ),
                (
                    "Wishlist lead passed SHOP-40.2's question on to you",
                    Some("Not in the plan."),
                ),
            ),
            (
                with(json!({"step": "handed_back"})),
                (
                    "SHOP-40.2's question handed back to you",
                    Some("no answer from Wishlist lead"),
                ),
            ),
        ];
        for (payload, (title, detail)) in cases {
            assert_eq!(row(payload), (title.to_owned(), detail.map(str::to_owned)));
        }
    }

    /// The epic's lead ended at Ready for Review (d0083).
    #[test]
    fn the_lead_ending_names_the_review() {
        let rows = rows(
            &[event(
                EventKind::LeadEnded,
                MIN,
                json!({"lead": "Wishlist lead"}),
            )],
            &[],
            &[],
        );
        assert_eq!(rows[0].title, "Lead ended \u{b7} epic in review");
        assert_eq!(rows[0].detail.as_deref(), Some("Wishlist lead"));
    }

    /// A child the epic's drag queued (d0080).
    #[test]
    fn a_child_queued_with_its_epic_says_so() {
        let rows = rows(
            &[event(
                EventKind::Moved,
                MIN,
                json!({"from": "backlog", "to": "queued", "with_epic": "SHOP-45"}),
            )],
            &[],
            &[],
        );
        assert_eq!(rows[0].title, "Ready for Dev with the epic");
        assert_eq!(rows[0].detail.as_deref(), Some("by you"));
    }

    /// Q77: the exit row names the session's tool by its command.
    #[test]
    fn an_exit_names_the_tool() {
        let exit = |payload| {
            rows(&[event(EventKind::Interrupted, MIN, payload)], &[], &[])[0]
                .detail
                .clone()
        };
        assert_eq!(
            exit(json!({"reason": "crash", "session": "s1", "tool": "codex", "from": "working"})),
            Some("codex exited (crash) · conversation saved".to_owned())
        );
        assert_eq!(
            exit(json!({"reason": "exit 1", "session": "s1", "tool": "claude"})),
            Some("claude exited (exit 1) · conversation saved".to_owned())
        );
        assert_eq!(
            exit(json!({"reason": "crash", "session": "s1"})),
            Some("the agent exited (crash) · conversation saved".to_owned())
        );
    }

    #[test]
    fn recovery_events_read_like_the_frame() {
        let rows = rows(
            &[
                event(
                    EventKind::Interrupted,
                    3 * MIN,
                    json!({"reason": "restart", "from": "working"}),
                ),
                event(EventKind::Resumed, 4 * MIN, json!({"from": "interrupted"})),
                event(
                    EventKind::StartedFromScratch,
                    5 * MIN,
                    json!({"tool": "claude", "old_tool_session_id": "3be94c10-x"}),
                ),
                event(
                    EventKind::ResumeFailed,
                    6 * MIN,
                    json!({"tool": "claude", "tool_session_id": "3be94c10-x"}),
                ),
            ],
            &[],
            &[],
        );
        let shown: Vec<(&str, &str)> = rows
            .iter()
            .map(|r| (r.title.as_str(), r.detail.as_deref().unwrap_or_default()))
            .collect();
        assert_eq!(
            shown,
            [
                ("Resume failed", "conversation 3be94c10-x not found"),
                (
                    "Started from scratch (resume failed)",
                    "same worktree · old conversation 3be94c10-x"
                ),
                ("Resumed", "same conversation · by you"),
                ("Interrupted by restart", "conversation saved"),
            ]
        );
    }

    #[test]
    fn sessions_and_events_newest_first() {
        let sessions = vec![
            session("b", 100 * MIN, None, None),
            session("a", 10 * MIN, Some(82 * MIN), Some("service stopped")),
        ];
        let events = vec![
            event(EventKind::Started, 10 * MIN, json!({})),
            event(
                EventKind::StartInterrupted,
                5 * MIN,
                json!({"step": "setup", "message": "setup command failed (exit 1):\nERR"}),
            ),
            event(
                EventKind::Moved,
                2 * MIN,
                json!({"from": "backlog", "to": "queued"}),
            ),
            event(EventKind::Created, MIN, json!({})),
        ];
        let rows = rows(&events, &sessions, &[]);
        let titles: Vec<_> = rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Agent session",
                "Agent session",
                "Start failed",
                "Ready for Dev",
                "Created"
            ]
        );
        assert_eq!(rows[0].duration, "running");
        assert_eq!(rows[1].duration, "72 min");
        assert_eq!(
            rows[1].detail.as_deref(),
            Some("claude 3be94c10-a · service stopped")
        );
        assert_eq!(rows[1].tool_session_id.as_deref(), Some("3be94c10-a"));
        assert_eq!(
            rows[2].detail.as_deref(),
            Some("setup command failed: setup command failed (exit 1):")
        );
        assert_eq!(rows[4].detail.as_deref(), Some("by you"));
        assert!(rows.iter().all(|r| r.tokens == "—"));
    }

    #[test]
    fn durations_round_to_minutes() {
        let r = session_row(
            &session("x", 0, Some(125 * MIN + 30_000), Some("exit 0")),
            &[],
        );
        assert_eq!(r.duration, "2 h 5 min");
        let r = session_row(&session("y", 0, Some(30_000), None), &[]);
        assert_eq!(r.duration, "0 min");
    }

    /// AGM-08: a stage choice's line, on the ticket and (for an epic lead's choice) on the epic.
    #[test]
    fn a_stage_choice_line() {
        let p = serde_json::json!({
            "stage": "Dev", "choice": "Quick developer", "set_by": "project_agent", "by": "the Project agent",
            "reason": "typo fix"
        });
        assert_eq!(
            choice_line(&p),
            (
                "Dev: Quick developer (set by the Project agent)".to_owned(),
                Some("typo fix".to_owned())
            )
        );
        let p = serde_json::json!({
            "stage": "Grill", "choice": "skip", "by": "Wishlist lead", "reason": "docs only", "child": "SHOP-40.3"
        });
        assert_eq!(
            choice_line(&p).0,
            "Wishlist lead set SHOP-40.3's Grill: skip"
        );
    }

    /// FLW-05: a choice the flow no longer offers, dropped when its stage started.
    #[test]
    fn a_dropped_choice_line() {
        let e = event(
            EventKind::ChoiceDropped,
            1,
            serde_json::json!({
                "stage": "Dev", "stage_id": "dev", "chose": "Quick developer", "runs": "Rust developer"
            }),
        );
        let row = event_row(&e).unwrap();
        assert_eq!(
            row.title,
            "Quick developer is no longer an option for Dev \u{b7} runs Rust developer"
        );
        assert_eq!(row.detail, None);
    }

    /// APN-04: Apply now's moves name the stage that went: "Moved to Review" · "by you · Security was removed".
    #[test]
    fn apply_now_moves_name_the_removed_stage() {
        let row = |kind, payload| event_row(&event(kind, 1, payload)).unwrap();
        let moved = row(
            EventKind::Dragged,
            serde_json::json!({ "stage": "Review", "back": false, "skipped": [], "removed": "Security" }),
        );
        assert_eq!(moved.title, "Moved to Review");
        assert_eq!(
            moved.detail.as_deref(),
            Some("by you \u{b7} Security was removed")
        );
        let skipped = row(
            EventKind::Dragged,
            serde_json::json!({ "stage": "QA", "back": false, "skipped": ["Dev"], "removed": "Grill" }),
        );
        assert_eq!(
            skipped.detail.as_deref(),
            Some("Skipped Dev \u{b7} by you \u{b7} Grill was removed")
        );
        let parked = row(EventKind::Parked, serde_json::json!({ "removed": "Grill" }));
        assert_eq!(
            parked.detail.as_deref(),
            Some("by you \u{b7} Grill was removed")
        );
        let dragged = row(
            EventKind::Dragged,
            serde_json::json!({ "stage": "Dev", "back": true }),
        );
        assert_eq!(
            dragged.detail.as_deref(),
            Some("by you"),
            "an ordinary drag says nothing more"
        );
    }
}
