//! The History tab (UI-04; d0045): every session the ticket had, with its time, duration, end reason and full
//! Claude session ID (copyable, so `claude --resume <id>` finds the conversation), and the ticket's events
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
    /// The full Claude session ID, for the copy button.
    pub claude_session_id: Option<String>,
    pub is_session: bool,
    /// A validation round's outcome icon and colours; sessions and events have their own icons.
    pub outcome: Option<(IconName, Palette)>,
    /// A counted validation round opens its report.
    pub report: Option<Report>,
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
            claude_session_id: None,
            is_session: false,
            outcome: Some((round.icon, round.palette)),
            report: round.report,
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
    let detail = match (&s.claude_session_id, s.end_reason.as_deref()) {
        (Some(id), Some(reason)) => Some(format!("claude {id} · {reason}")),
        (Some(id), None) => Some(format!("claude {id}")),
        (None, Some(reason)) => Some(reason.to_owned()),
        (None, None) => None,
    };
    HistoryRow {
        title: title.to_owned(),
        detail,
        at: s.started_at,
        duration,
        tokens: crate::view_models::usage::session_label(usage, &s.id),
        claude_session_id: s.claude_session_id.clone(),
        is_session: true,
        outcome: None,
        report: None,
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
        EventKind::Created => ("Created".to_owned(), Some(by(e))),
        EventKind::Moved => {
            let to = text(p, "to").and_then(|s| s.parse::<Status>().ok());
            // A child queued by its epic's drag (d0080): "Queued with the epic · by you".
            let title = match to {
                Some(Status::Queued) if p.get("with_epic").is_some() => {
                    "Queued with the epic".to_owned()
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
                "Start interrupted".to_owned(),
                Some(match text(p, "message") {
                    Some(m) => format!("{step}: {}", first_line(m)),
                    None => step.to_owned(),
                }),
            )
        }
        EventKind::StartRetried => ("Retry start".to_owned(), Some(who(&e.actor))),
        EventKind::Held => ("Held".to_owned(), text(p, "message").map(first_line)),
        EventKind::StartNote => ("Note".to_owned(), text(p, "message").map(first_line)),
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
        EventKind::Closed => ("Closed".to_owned(), Some(who(&e.actor))),
        EventKind::ReviewSubmitted => (
            "Submitted for review".to_owned(),
            text(p, "summary").map(first_line),
        ),
        EventKind::Merged => (
            "Merged".to_owned(),
            text(p, "target").map(|t| format!("into {t}")),
        ),
        EventKind::Reopened => ("Reopened".to_owned(), Some(who(&e.actor))),
        EventKind::Parked => ("Parked".to_owned(), Some(who(&e.actor))),
        EventKind::Rebased => (
            "Rebased".to_owned(),
            text(p, "onto").map(|b| match p.get("conflict") {
                Some(_) => format!("onto {b}: conflicts, the agent resolves them first"),
                None => format!("onto {b}"),
            }),
        ),
        EventKind::SentToFix => (
            "Sent to agent to fix".to_owned(),
            text(p, "target").map(|t| format!("conflicts with {t}")),
        ),
        // *16 → History · Claude session IDs*: "Interrupted by restart · conversation saved".
        EventKind::Interrupted => match text(p, "reason").as_deref() {
            Some("restart") | None => (
                "Interrupted by restart".to_owned(),
                Some("conversation saved".to_owned()),
            ),
            Some(reason) => (
                "Interrupted".to_owned(),
                Some(format!("claude exited ({reason}) · conversation saved")),
            ),
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
            Some(match text(p, "claude_session_id") {
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
            Some(match text(p, "old_claude_session_id") {
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
            text(p, "target").map(|t| format!("into {t}")),
        ),
        EventKind::AutoMergeStopped => ("Auto-merge stopped".to_owned(), text(p, "message")),
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
        // "Marked by Wishlist lead · contradicts PAY-123: “…”" (DEC-04).
        EventKind::DecisionMarked => (
            format!(
                "Marked by {}",
                text(p, "lead").unwrap_or_else(|| "the lead".to_owned())
            ),
            Some(format!(
                "contradicts {}: \u{201c}{}\u{201d}",
                text(p, "source").unwrap_or_default(),
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
        claude_session_id: None,
        is_session: false,
        outcome: None,
        report: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
            claude_session_id: Some(format!("3be94c10-{id}")),
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
        ];
        for (e, (title, detail)) in cases {
            assert_eq!(row(e), (title.to_owned(), detail.map(str::to_owned)));
        }
    }

    /// A new child of a whole-queued epic (EPC-13).
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
        assert_eq!(rows[0].title, "Queued with the epic");
        assert_eq!(rows[0].detail.as_deref(), Some("by you"));
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
                    json!({"old_claude_session_id": "3be94c10-x"}),
                ),
                event(
                    EventKind::ResumeFailed,
                    6 * MIN,
                    json!({"claude_session_id": "3be94c10-x"}),
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
                "Start interrupted",
                "Queued",
                "Created"
            ]
        );
        assert_eq!(rows[0].duration, "running");
        assert_eq!(rows[1].duration, "72 min");
        assert_eq!(
            rows[1].detail.as_deref(),
            Some("claude 3be94c10-a · service stopped")
        );
        assert_eq!(rows[1].claude_session_id.as_deref(), Some("3be94c10-a"));
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
}
