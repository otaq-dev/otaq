//! The Needs-you inbox as the page draws it (T-0230; UI-06, ATT-06…ATT-10, ATT-12, USG-07, MSG-05, CLS-04; *10 ·
//! Needs-you inbox*): the head with the total and Next ⌘J, the usage-limit row once at the top, the groups with their
//! counts and Resume all, each row with its project, reason, age, the answer box's texts and options and its
//! buttons, the empty state and the foot's keys; the ⌘J run bar on top of the panel and "All clear"; the Resume all
//! confirmation with its ticket list; the Project agents waiting for the user in group 1, labelled with the project
//! and the agent's name and answered inline (T-0241; PAG-07). Every text is worded here, from `inbox.rs`, `inbox_run.rs`, `answer.rs` and
//! `answer_options.rs`; the page keeps only the selection and the draft.

use otaq_core::board::needs_you::NeedsYouGroup;
use otaq_core::board::questions::Choice;
use otaq_core::domain::{TicketKey, ValidationAction};
use otaq_core::project_agents::AgentNeedsYou;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;
use crate::view_models::answer::{self as answers, AnswerKind};
use crate::view_models::answer_options::{self, AnswerOptions};
use crate::view_models::inbox::{
    InboxGroupView, InboxRow, ResumeAllConfirm, RowAction, age, group_title,
};
use crate::view_models::inbox_run::{ALL_CLEAR_FOR, RunBar, all_clear_text, bar_title};
use crate::view_models::limit_banner::LimitBanner;
use crate::view_models::panel::Tab;
use crate::view_models::project_terminal::agent_tab_id;
use crate::view_models::status::{PillView, pill};
use crate::view_models::terminal_strip::UnsentPrompt;

/// A key hint: its keys on this platform and what they do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxKeyHint")]
pub struct KeyHint {
    pub keys: String,
    pub label: &'static str,
}

/// What a row's button does; the page sends it back with `inbox.act`. `InboxRowAction` on the page. Later item
/// kinds (Project agents) add their own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "InboxRowAction")]
pub enum Action {
    RetryStart,
    ReopenParent {
        parent: String,
    },
    RemoveDependency {
        parent: String,
    },
    /// "This is the usage limit" (USG-07).
    UsageLimit,
    /// "Resume SHOP-17": the finished ticket answers the question sent to the user (MSG-05).
    ResumeTarget {
        message: String,
    },
    /// Validation failed N times: Ready for Review without a pass (VAL-09).
    MoveToReviewAnyway,
    /// Validation failed N times: one more round, the findings go to the agent.
    AnotherRound,
    /// Validation failed N times: the user's note goes with the findings. The button opens the row's note box;
    /// its Send comes back with the note.
    AnswerAgent {
        #[serde(default)]
        note: String,
    },
    /// Ended without a result: a fresh validator (VAL-07).
    RetryValidation,
    /// Auto-merge stopped: run the merge again (AMG-03, AMG-04).
    RetryMerge,
    /// Auto-merge stopped: Ready for Review, still validated, not merged again.
    MoveToReview,
}

impl Action {
    /// The service's validation action and its note, for the M12 items' buttons.
    pub fn validation(&self) -> Option<(ValidationAction, String)> {
        use ValidationAction as A;
        Some(match self {
            Self::MoveToReviewAnyway => (A::MoveToReviewAnyway, String::new()),
            Self::AnotherRound => (A::AnotherRound, String::new()),
            Self::AnswerAgent { note } => (A::AnswerAgent, note.clone()),
            Self::RetryValidation => (A::Retry, String::new()),
            Self::RetryMerge => (A::RetryMerge, String::new()),
            Self::MoveToReview => (A::MoveToReview, String::new()),
            _ => return None,
        })
    }

    pub fn of(action: &RowAction) -> Self {
        match action {
            RowAction::RetryStart => Self::RetryStart,
            RowAction::ReopenParent(p) => Self::ReopenParent {
                parent: p.to_string(),
            },
            RowAction::RemoveDependency(p) => Self::RemoveDependency {
                parent: p.to_string(),
            },
            RowAction::UsageLimit => Self::UsageLimit,
            RowAction::ResumeTarget { message, .. } => Self::ResumeTarget {
                message: message.clone(),
            },
            RowAction::Validation(action) => match action {
                ValidationAction::MoveToReviewAnyway => Self::MoveToReviewAnyway,
                ValidationAction::AnotherRound => Self::AnotherRound,
                ValidationAction::AnswerAgent => Self::AnswerAgent {
                    note: String::new(),
                },
                ValidationAction::Retry => Self::RetryValidation,
                ValidationAction::RetryMerge => Self::RetryMerge,
                // Only the actions an inbox item offers (validation_actions) reach here.
                _ => Self::MoveToReview,
            },
        }
    }
}

/// A row's button.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxRowButton")]
pub struct Button {
    pub action: Action,
    pub label: String,
    pub icon: IconName,
    pub primary: bool,
}

fn button(action: &RowAction) -> Button {
    let (label, icon, primary) = match action {
        RowAction::RetryStart => ("Retry start".to_owned(), IconName::RotateCcw, false),
        RowAction::ReopenParent(_) => ("Reopen parent".to_owned(), IconName::RotateCcw, true),
        RowAction::RemoveDependency(_) => ("Remove dependency".to_owned(), IconName::Unlink, false),
        RowAction::UsageLimit => ("This is the usage limit".to_owned(), IconName::Gauge, false),
        RowAction::ResumeTarget { target, .. } => {
            (format!("Resume {target}"), IconName::Play, false)
        }
        RowAction::Validation(action) => {
            // The first of an item's actions is the primary one (the frame's filled button).
            let (label, icon) = validation_button(*action);
            (label.to_owned(), icon, false)
        }
    };
    Button {
        action: Action::of(action),
        label,
        icon,
        primary,
    }
}

/// An M12 item's button: its label and icon (*10 → New item types · group 1 (M12)*).
fn validation_button(action: ValidationAction) -> (&'static str, IconName) {
    use ValidationAction as A;
    match action {
        A::MoveToReviewAnyway => ("Move to Review anyway", IconName::Eye),
        A::AnotherRound => ("Give it another round", IconName::RotateCcw),
        A::AnswerAgent => ("Answer the agent\u{2026}", IconName::MessageSquare),
        A::Retry => ("Retry validation", IconName::RotateCcw),
        A::RetryMerge => ("Retry merge", IconName::RotateCcw),
        _ => ("Move to Review", IconName::Eye),
    }
}

/// The box "Answer the agent…" opens in a row: the note goes to the agent with the findings (VAL-09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxNoteBox")]
pub struct NoteBox {
    pub placeholder: &'static str,
    pub send: &'static str,
    pub send_icon: IconName,
    pub cancel: &'static str,
    /// "The findings go with it; the round counter restarts".
    pub text: &'static str,
}

/// One answer option (ATT-12). `InboxOption` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxOption")]
pub struct OptionRow {
    pub number: u32,
    pub label: String,
    pub recommended: bool,
    pub picked: bool,
    pub other: bool,
}

/// How a row takes an answer (ATT-08): its buttons, and its options when the question has them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxAnswer")]
pub struct Answer {
    /// "Answer", or "Answer & resume" for an interrupted question (d0045).
    pub open: &'static str,
    pub open_icon: IconName,
    pub send: &'static str,
    pub send_icon: IconName,
    /// The open question's ID, sent back so a changed question isn't answered by mistake.
    pub question: Option<String>,
    /// An agent's question sent to the user (MSG-05): answered as free text.
    pub message: Option<String>,
    /// "OPTIONS FROM SHOP-21’S AGENT", with the options; none for a free-text question.
    pub heading: Option<String>,
    pub options: Vec<OptionRow>,
    pub recommended: &'static str,
    pub placeholder: &'static str,
    /// "Logged in SHOP-21 Messages".
    pub logged: String,
    pub cancel: &'static str,
}

fn answer(row: &InboxRow) -> Option<Answer> {
    let kind = row.answer?;
    let options = row
        .question
        .as_ref()
        .filter(|_| row.message.is_none())
        .and_then(AnswerOptions::of);
    Some(Answer {
        open: kind.open_label(),
        open_icon: match kind {
            AnswerKind::Send => IconName::CornerDownLeft,
            AnswerKind::Resume => IconName::Play,
        },
        send: kind.send_label(),
        send_icon: kind.send_icon(),
        question: row.question.as_ref().map(|q| q.id.clone()),
        message: row.message.clone(),
        heading: options.as_ref().map(|_| answer_options::heading(&row.key)),
        options: options
            .map(|o| {
                o.rows()
                    .into_iter()
                    .map(|r| OptionRow {
                        number: r.number,
                        label: r.label,
                        recommended: r.recommended,
                        picked: r.picked,
                        other: r.other,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        recommended: "Recommended",
        placeholder: "Type your answer",
        logged: format!("Logged in {} Messages", row.key),
        cancel: "Cancel",
    })
}

/// One row. `InboxRowView` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxRowView")]
pub struct Row {
    pub key: String,
    pub project: String,
    pub project_name: String,
    pub title: String,
    pub reason: String,
    pub age: String,
    /// "blocks 2".
    pub blocks: Option<String>,
    /// The status's icon and colours.
    pub pill: PillView,
    /// "session ended by restart · answering resumes it".
    pub restart_note: Option<&'static str>,
    /// "Sent to you: finished tickets don’t answer in this project".
    pub sent_note: Option<&'static str>,
    /// Resolved elsewhere, fading out: struck through, no buttons.
    pub resolved: bool,
    /// The panel tab the row opens.
    pub tab: Tab,
    pub answer: Option<Answer>,
    pub buttons: Vec<Button>,
    /// "↵ / ⌘J open Terminal · Validator side" (Needs you · validator).
    pub hint: Option<KeyHint>,
    /// The note box of "Answer the agent…".
    pub note: Option<NoteBox>,
    /// A Project agent's item (PAG-07): no ticket, `key` is the agent's id; opening it shows the agent's tab in
    /// the Terminals & agents drawer, and its answer goes to `project_agent.answer`.
    pub agent: Option<AgentLink>,
}

/// The Project agent a row belongs to. `InboxAgentLink` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxAgentLink")]
pub struct AgentLink {
    pub id: String,
    /// The agent's tab in the Terminals & agents drawer.
    pub tab: String,
}

/// A Project agent waiting for the user (*10 → New item types · group 1 (M12)*, the "Import GitHub issues" item):
/// the project and `Project agent “name”` instead of a ticket, the reason, the age and "opens that Project agent".
/// A question is answered inline like a ticket's (its options when it has them); a permission prompt is answered in
/// the agent's terminal.
pub fn agent_row(a: &AgentNeedsYou, now_ms: i64) -> Row {
    let (legend, _) = pill(a.status);
    let mut icon = PillView::legend(legend, None);
    icon.icon = IconName::Bot;
    let options = a.question.as_ref().and_then(AnswerOptions::of);
    let answer = a.question.as_ref().map(|q| Answer {
        open: AnswerKind::Send.open_label(),
        open_icon: IconName::CornerDownLeft,
        send: AnswerKind::Send.send_label(),
        send_icon: AnswerKind::Send.send_icon(),
        question: Some(q.id.clone()),
        message: None,
        heading: options
            .as_ref()
            .map(|_| "OPTIONS FROM THE PROJECT AGENT".to_owned()),
        options: options
            .map(|o| {
                o.rows()
                    .into_iter()
                    .map(|r| OptionRow {
                        number: r.number,
                        label: r.label,
                        recommended: r.recommended,
                        picked: r.picked,
                        other: r.other,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        recommended: "Recommended",
        placeholder: "Type your answer",
        logged: "Typed into the agent\u{2019}s terminal".to_owned(),
        cancel: "Cancel",
    });
    Row {
        key: a.agent.clone(),
        project: a.project.clone(),
        project_name: a.project_name.clone(),
        title: agent_title(&a.name),
        reason: a.reason.clone(),
        age: age(now_ms.saturating_sub(a.since)),
        blocks: None,
        pill: icon,
        restart_note: None,
        sent_note: None,
        resolved: false,
        tab: Tab::Terminal,
        answer,
        buttons: Vec::new(),
        hint: Some(KeyHint {
            keys: String::new(),
            label: "opens that Project agent",
        }),
        note: None,
        agent: Some(AgentLink {
            id: a.agent.clone(),
            tab: agent_tab_id(&a.agent),
        }),
    }
}

/// "Project agent “Import GitHub issues”".
pub fn agent_title(name: &str) -> String {
    format!("Project agent \u{201c}{name}\u{201d}")
}

/// The unsent-text question before a Project agent's answer (d0048): the agent has no ticket, so it is named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentUnsent {
    pub title: String,
    pub body: String,
}

pub fn agent_unsent(name: &str, choice: &Choice) -> AgentUnsent {
    AgentUnsent {
        title: format!("{} has unsent text in its terminal", agent_title(name)),
        body: format!(
            "{} {}.",
            UnsentPrompt::EXPLANATION,
            answers::sending(choice)
        ),
    }
}

fn row(r: &InboxRow, next_keys: &str) -> Row {
    let mut buttons: Vec<Button> = r.actions.iter().map(button).collect();
    let validation = |b: &Button| b.action.validation().is_some();
    if let Some(first) = buttons.iter_mut().find(|b| validation(b)) {
        first.primary = true;
    }
    let note = buttons
        .iter()
        .any(|b| matches!(b.action, Action::AnswerAgent { .. }))
        .then_some(NoteBox {
            placeholder: "Tell the agent what to do",
            send: "Send to the agent",
            send_icon: IconName::CornerDownLeft,
            cancel: "Cancel",
            text: "Goes with the findings · the round count starts over",
        });
    Row {
        key: r.key.to_string(),
        project: r.project.clone(),
        project_name: r.project_name.clone(),
        title: r.title.clone(),
        reason: r.reason.clone(),
        age: r.age.clone(),
        blocks: r.blocks.clone(),
        pill: PillView::legend(r.legend, None),
        restart_note: r.restart_note,
        sent_note: r.sent_note,
        resolved: r.resolved,
        tab: r.tab,
        answer: answer(r),
        buttons,
        hint: r.opens_validator.then(|| KeyHint {
            keys: format!("↵ / {next_keys}"),
            label: "open Terminal · Validator side",
        }),
        note,
        agent: None,
    }
}

/// Which group. `InboxGroupId` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "InboxGroupId")]
pub enum GroupId {
    Family,
    /// An epic lead's suggestions, right after group 1 (LEAD-08).
    Suggestions,
    Stopped,
    Review,
}

/// One group with its rows. `InboxGroup` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxGroup")]
pub struct Group {
    pub id: GroupId,
    pub title: &'static str,
    pub count: u32,
    /// "Resume all (1)" on the interrupted group.
    pub resume_all: Option<String>,
    pub rows: Vec<Row>,
}

fn group(g: &InboxGroupView, next_keys: &str) -> Group {
    Group {
        id: match g.group {
            NeedsYouGroup::Family => GroupId::Family,
            NeedsYouGroup::Suggestions => GroupId::Suggestions,
            NeedsYouGroup::Stopped => GroupId::Stopped,
            NeedsYouGroup::Review => GroupId::Review,
        },
        title: g.title,
        count: g.count as u32,
        resume_all: g.resume_all.map(|n| format!("Resume all ({n})")),
        rows: g.rows.iter().map(|r| row(r, next_keys)).collect(),
    }
}

/// The usage limit once at the top (*10 → Usage limit active · no duplicates*), with Resume all after the reset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxLimitRow")]
pub struct LimitRow {
    pub text: String,
    pub resume: Option<String>,
}

/// "Nothing needs you" (*10 → Empty · nothing needs you*).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxEmpty")]
pub struct Empty {
    pub title: &'static str,
    pub text: String,
}

/// The inbox. `InboxView` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxView")]
pub struct View {
    pub title: &'static str,
    pub total: u32,
    pub scope: &'static str,
    /// "Next" with ⌘J, while something waits.
    pub next: Option<KeyHint>,
    pub limit: Option<LimitRow>,
    pub groups: Vec<Group>,
    pub empty: Option<Empty>,
    /// ↑↓ move · ↵ open · ⌘J next item, under a non-empty list.
    pub foot: Vec<KeyHint>,
}

/// The inbox from the model's groups, the waiting Project agents (after the tickets in group 1), its total, the
/// usage-limit banner, the agents running and ⌘J's keys.
#[allow(clippy::too_many_arguments)]
pub fn view(
    groups: &[InboxGroupView],
    agents: &[AgentNeedsYou],
    now_ms: i64,
    total: u32,
    limit: Option<&LimitBanner>,
    running: u32,
    next_keys: &str,
) -> View {
    let mut drawn: Vec<Group> = groups.iter().map(|g| group(g, next_keys)).collect();
    if !agents.is_empty() {
        let rows = agents.iter().map(|a| agent_row(a, now_ms));
        match drawn.iter_mut().find(|g| g.id == GroupId::Family) {
            Some(g) => {
                g.count += agents.len() as u32;
                g.rows.extend(rows);
            }
            None => drawn.insert(
                0,
                Group {
                    id: GroupId::Family,
                    title: group_title(NeedsYouGroup::Family),
                    count: agents.len() as u32,
                    resume_all: None,
                    rows: rows.collect(),
                },
            ),
        }
    }
    let empty = drawn.is_empty().then(|| {
        let plural = if running == 1 { "agent" } else { "agents" };
        Empty {
            title: "Nothing needs you",
            text: format!(
                "{running} {plural} running. You\u{2019}ll get a notification when one is blocked on you."
            ),
        }
    });
    View {
        title: "Needs you",
        total,
        scope: "· all projects",
        next: (total > 0).then(|| KeyHint {
            keys: next_keys.to_owned(),
            label: "Next",
        }),
        limit: limit.map(|b| LimitRow {
            text: b.inbox_row.clone(),
            resume: b.after_reset.then(|| b.resume.label.clone()),
        }),
        foot: if drawn.is_empty() {
            Vec::new()
        } else {
            vec![
                KeyHint {
                    keys: "↑↓".to_owned(),
                    label: "move",
                },
                KeyHint {
                    keys: "↵".to_owned(),
                    label: "open",
                },
                KeyHint {
                    keys: next_keys.to_owned(),
                    label: "next item",
                },
            ]
        },
        groups: drawn,
        empty,
    }
}

/// How the run bar looks: a step (turquoise), the open item answered (green). `InboxRunTone` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "InboxRunTone")]
pub enum RunTone {
    Step,
    Done,
}

/// One segment of the run bar. `InboxRunSegment` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "InboxRunSegment")]
pub enum Segment {
    Done,
    Current,
    Waiting,
}

/// The bar on top of the panel during a ⌘J run (*10 → 1 · ⌘J from anywhere, 2 · Answered*). `InboxRunBar` on the
/// page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxRunBar")]
pub struct RunBarView {
    pub tone: RunTone,
    /// "Inbox · 1 of 7", "Done · 6 left".
    pub title: String,
    pub segments: Vec<Segment>,
    /// "Next" (a step) or "next:" (answered), with ⌘J.
    pub next: KeyHint,
    pub exit: Option<&'static str>,
}

/// The bar while the run is on a step or answered; `None` for "All clear" (drawn over the board instead).
pub fn run_bar(bar: &RunBar, next_keys: &str) -> Option<RunBarView> {
    let segments = |of: usize, done: usize, current: Option<usize>| {
        (0..of.min(24))
            .map(|i| {
                if i < done {
                    Segment::Done
                } else if Some(i) == current {
                    Segment::Current
                } else {
                    Segment::Waiting
                }
            })
            .collect()
    };
    match bar {
        RunBar::Step { n, of, done } => Some(RunBarView {
            tone: RunTone::Step,
            title: bar_title(bar),
            segments: segments(*of, *done, Some(n - 1)),
            next: KeyHint {
                keys: next_keys.to_owned(),
                label: "Next",
            },
            exit: Some("Exit"),
        }),
        RunBar::Done { left, of } => Some(RunBarView {
            tone: RunTone::Done,
            title: bar_title(bar),
            segments: segments(*of, of - left, None),
            next: KeyHint {
                keys: next_keys.to_owned(),
                label: "next:",
            },
            exit: None,
        }),
        RunBar::AllClear { .. } => None,
    }
}

/// "All clear" over the board after the last item (*10 → 4 · All clear*). `InboxAllClear` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxAllClear")]
pub struct AllClear {
    pub title: &'static str,
    pub text: String,
    /// One segment per item handled (at least one, at most 24).
    pub segments: u32,
    /// How long it stays.
    pub shown_ms: u64,
}

pub fn all_clear(handled: usize, minutes: u64, running: u32) -> AllClear {
    AllClear {
        title: "All clear",
        text: all_clear_text(handled, minutes, running),
        segments: handled.clamp(1, 24) as u32,
        shown_ms: ALL_CLEAR_FOR.as_millis() as u64,
    }
}

/// One ticket of the Resume all confirmation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxResumeTicket")]
pub struct ResumeTicket {
    pub key: String,
    pub title: String,
}

/// The Resume all confirmation (*10 → Resume all · confirm*). `InboxResumeAllDialog` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxResumeAllDialog")]
pub struct ResumeAllDialog {
    pub title: String,
    pub text: &'static str,
    pub included: Vec<ResumeTicket>,
    pub not_included: Option<String>,
    pub cancel: &'static str,
    pub cancel_hint: &'static str,
    pub confirm: String,
}

pub fn resume_all_dialog(confirm: &ResumeAllConfirm) -> ResumeAllDialog {
    ResumeAllDialog {
        title: confirm.title.clone(),
        text: confirm.text,
        included: confirm
            .included
            .iter()
            .map(|(key, title): &(TicketKey, String)| ResumeTicket {
                key: key.to_string(),
                title: title.clone(),
            })
            .collect(),
        not_included: confirm.not_included.clone(),
        cancel: "Cancel",
        cancel_hint: "cancel",
        confirm: confirm.confirm.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use otaq_core::attention::inbox::{InboxCounts, InboxList};
    use otaq_core::board::needs_you::NeedsYouItem;
    use otaq_core::board::questions::HumanQuestion;
    use otaq_core::domain::Status;

    use super::*;
    use crate::view_models::inbox::InboxModel;

    fn item(key: &str, status: Status, group: NeedsYouGroup) -> NeedsYouItem {
        NeedsYouItem {
            group,
            project: "SHOP".into(),
            project_name: "shop-web".into(),
            id: key.into(),
            key: key.parse().unwrap(),
            title: format!("{key} title"),
            status,
            interrupted_from: None,
            question: None,
            reason: "asks: which?".into(),
            since: 0,
            age_ms: 6 * 60_000,
            blocked: 0,
            message: None,
            suggestion: None,
        }
    }

    fn model(items: Vec<NeedsYouItem>) -> InboxModel {
        let mut m = InboxModel::default();
        let total = items.len() as u32;
        m.update(
            InboxList {
                items,
                counts: InboxCounts {
                    total,
                    ..InboxCounts::default()
                },
                ..InboxList::default()
            },
            Instant::now(),
        );
        m
    }

    fn agent(id: &str, status: Status, question: Option<HumanQuestion>) -> AgentNeedsYou {
        AgentNeedsYou {
            project: "SHOP".into(),
            project_name: "shop-web".into(),
            agent: id.into(),
            name: "Import GitHub issues".into(),
            epic: None,
            status,
            reason: if question.is_some() {
                "asks: Import closed issues too?".into()
            } else {
                "permission: Allow gh issue list?".into()
            },
            question,
            since: 1_000,
        }
    }

    /// *10 → New item types · group 1 (M12)*, the Project agent item (PAG-07): in group 1 after the tickets, labelled
    /// with the project and the agent's name (no ticket key), "opens that Project agent", counted in the group; a
    /// question is answered inline with its options, a permission prompt only opens the agent.
    #[test]
    fn project_agents_wait_in_group_1() {
        let asks = agent(
            "a1",
            Status::AsksYou,
            Some(HumanQuestion {
                id: "q7".into(),
                text: "Import closed issues too?".into(),
                options: vec!["Yes".into(), "No".into()],
                recommended: Some(2),
                asked_at: 1_000,
            }),
        );
        let permission = agent("a2", Status::NeedsYou, None);
        let m = model(vec![item(
            "SHOP-32",
            Status::ValidationFailed,
            NeedsYouGroup::Family,
        )]);
        let agents = [asks, permission];
        let v = view(
            &m.groups(&|_| None),
            &agents,
            1_000 + 3 * 60_000,
            3,
            None,
            3,
            "⌘J",
        );
        let g = &v.groups[0];
        assert_eq!(g.count, 3);
        assert_eq!(g.rows[0].key, "SHOP-32", "tickets first");
        let row = &g.rows[1];
        assert_eq!(row.title, "Project agent “Import GitHub issues”");
        assert_eq!(row.project_name, "shop-web");
        assert_eq!(row.reason, "asks: Import closed issues too?");
        assert_eq!(row.age, "3 min");
        assert_eq!(row.pill.icon, IconName::Bot);
        assert_eq!(row.hint.as_ref().unwrap().label, "opens that Project agent");
        assert_eq!(
            row.agent,
            Some(AgentLink {
                id: "a1".into(),
                tab: "agent:a1".into()
            })
        );
        let answer = row.answer.as_ref().unwrap();
        assert_eq!(answer.question.as_deref(), Some("q7"));
        assert_eq!(
            answer.heading.as_deref(),
            Some("OPTIONS FROM THE PROJECT AGENT")
        );
        assert_eq!(answer.options.len(), 3, "two options and Other…");
        assert!(answer.options[1].picked, "the recommended one is picked");
        assert!(row.buttons.is_empty());
        let permission = &g.rows[2];
        assert_eq!(permission.reason, "permission: Allow gh issue list?");
        assert_eq!(permission.answer, None, "answered in the agent's terminal");
        assert_eq!(v.foot.len(), 3);

        // Only Project agents waiting: group 1 is there with them, the inbox isn't empty.
        let v = view(&[], &agents[1..], 1_000, 1, None, 0, "⌘J");
        assert_eq!(v.groups.len(), 1);
        assert_eq!(v.groups[0].id, GroupId::Family);
        assert_eq!(v.groups[0].count, 1);
        assert_eq!(v.empty, None);
    }

    /// Unsent text in the agent's terminal: the question names the agent and what is sent (d0048).
    #[test]
    fn a_project_agents_unsent_text_question_names_the_agent() {
        let ask = agent_unsent("Import GitHub issues", &Choice::Option(2));
        assert_eq!(
            ask.title,
            "Project agent “Import GitHub issues” has unsent text in its terminal"
        );
        assert!(
            ask.body
                .ends_with("nothing is sent. Your answer: option 2.")
        );
    }

    #[test]
    fn the_inbox_has_its_head_groups_rows_and_foot() {
        let mut asks = item("SHOP-21", Status::AsksYou, NeedsYouGroup::Family);
        asks.question = Some(HumanQuestion {
            id: "q1".into(),
            text: "Which provider?".into(),
            options: vec!["Postmark".into(), "SES".into()],
            recommended: Some(1),
            asked_at: 0,
        });
        let m = model(vec![
            asks,
            item("SHOP-25", Status::Interrupted, NeedsYouGroup::Stopped),
            item("SHOP-28", Status::StartInterrupted, NeedsYouGroup::Stopped),
        ]);
        let v = view(&m.groups(&|_| None), &[], 0, 3, None, 3, "⌘J");
        assert_eq!(v.title, "Needs you");
        assert_eq!(v.next.as_ref().unwrap().label, "Next");
        assert_eq!(v.groups.len(), 2);
        assert_eq!(v.groups[1].resume_all.as_deref(), Some("Resume all (1)"));
        let row = &v.groups[0].rows[0];
        assert_eq!(row.age, "6 min");
        let answer = row.answer.as_ref().unwrap();
        assert_eq!(answer.open, "Answer");
        assert_eq!(
            answer.heading.as_deref(),
            Some("OPTIONS FROM SHOP-21’S AGENT")
        );
        assert_eq!(answer.options.len(), 3, "two options and Other…");
        assert!(answer.options[0].picked && answer.options[0].recommended);
        assert_eq!(answer.logged, "Logged in SHOP-21 Messages");
        let retry = &v.groups[1].rows[1].buttons[0];
        assert_eq!(retry.label, "Retry start");
        assert_eq!(retry.action, Action::RetryStart);
        assert_eq!(v.foot.len(), 3);
        assert_eq!(v.empty, None);
    }

    /// *10 → New item types · group 1 (M12)*: the group heading, each item's buttons (the first one filled), the
    /// validator's "↵ / ⌘J open Terminal · Validator side", Answer the agent…'s note box, and "validated · round 2"
    /// on a review item (the service's reason).
    #[test]
    fn the_m12_items_in_group_1() {
        let mut review = item("SHOP-18", Status::ReadyForReview, NeedsYouGroup::Review);
        review.reason = "ready for review · validated · round 2".into();
        let m = model(vec![
            item("SHOP-33", Status::ValidatorNeedsYou, NeedsYouGroup::Family),
            item("SHOP-32", Status::ValidationFailed, NeedsYouGroup::Family),
            item("SHOP-34", Status::ValidationNoResult, NeedsYouGroup::Family),
            item("SHOP-40.4", Status::AutoMergeStopped, NeedsYouGroup::Family),
            review,
        ]);
        let v = view(&m.groups(&|_| None), &[], 0, 5, None, 3, "⌘J");
        assert_eq!(
            v.groups[0].title,
            "ASKS YOU · NEEDS YOU · IDLE · VALIDATION · PROJECT AGENTS"
        );
        let rows = &v.groups[0].rows;
        let buttons = |i: usize| {
            rows[i]
                .buttons
                .iter()
                .map(|b| (b.label.as_str(), b.primary))
                .collect::<Vec<_>>()
        };
        let validator = &rows[0];
        assert!(validator.buttons.is_empty());
        let hint = validator.hint.as_ref().unwrap();
        assert_eq!(
            (hint.keys.as_str(), hint.label),
            ("↵ / ⌘J", "open Terminal · Validator side")
        );
        assert_eq!(
            buttons(1),
            [
                ("Move to Review anyway", true),
                ("Give it another round", false),
                ("Answer the agent\u{2026}", false)
            ]
        );
        assert!(
            rows[1].note.is_some(),
            "Answer the agent… opens its note box"
        );
        assert_eq!(rows[1].hint, None);
        assert_eq!(
            buttons(2),
            [("Retry validation", true), ("Move to Review anyway", false)]
        );
        assert_eq!(rows[2].note, None);
        assert_eq!(
            buttons(3),
            [("Retry merge", true), ("Move to Review", false)]
        );
        assert_eq!(
            v.groups[1].rows[0].reason,
            "ready for review · validated · round 2"
        );
        // What each button asks the service for.
        let sent: Vec<_> = rows[1..]
            .iter()
            .flat_map(|r| r.buttons.iter().map(|b| b.action.validation().unwrap().0))
            .collect();
        use ValidationAction as A;
        assert_eq!(
            sent,
            [
                A::MoveToReviewAnyway,
                A::AnotherRound,
                A::AnswerAgent,
                A::Retry,
                A::MoveToReviewAnyway,
                A::RetryMerge,
                A::MoveToReview
            ]
        );
        let note: Action = serde_json::from_value(
            serde_json::json!({ "kind": "answer_agent", "note": "Use the arrow keys" }),
        )
        .unwrap();
        assert_eq!(
            note.validation(),
            Some((A::AnswerAgent, "Use the arrow keys".to_owned()))
        );
    }

    #[test]
    fn an_empty_inbox_says_what_happens_next() {
        let v = view(&[], &[], 0, 0, None, 1, "⌘J");
        assert_eq!(v.next, None);
        assert!(v.foot.is_empty());
        let empty = v.empty.unwrap();
        assert_eq!(empty.title, "Nothing needs you");
        assert_eq!(
            empty.text,
            "1 agent running. You’ll get a notification when one is blocked on you."
        );
    }

    #[test]
    fn row_actions_go_back_as_tagged_json() {
        let parent: TicketKey = "SHOP-26".parse().unwrap();
        let b = button(&RowAction::ReopenParent(parent.clone()));
        assert!(b.primary);
        assert_eq!(
            serde_json::to_value(&b.action).unwrap(),
            serde_json::json!({ "kind": "reopen_parent", "parent": "SHOP-26" })
        );
        let back: Action =
            serde_json::from_value(serde_json::json!({ "kind": "usage_limit" })).unwrap();
        assert_eq!(back, Action::UsageLimit);
        let target = button(&RowAction::ResumeTarget {
            message: "m1".into(),
            target: "SHOP-17".parse().unwrap(),
        });
        assert_eq!(target.label, "Resume SHOP-17");
        assert_eq!(
            button(&RowAction::RemoveDependency(parent)).label,
            "Remove dependency"
        );
    }

    #[test]
    fn the_run_bar_and_all_clear() {
        let step = run_bar(
            &RunBar::Step {
                n: 2,
                of: 7,
                done: 1,
            },
            "⌘J",
        )
        .unwrap();
        assert_eq!(step.title, "Inbox · 2 of 7");
        assert_eq!(step.segments[0], Segment::Done);
        assert_eq!(step.segments[1], Segment::Current);
        assert_eq!(step.segments[2], Segment::Waiting);
        assert_eq!(step.exit, Some("Exit"));
        let done = run_bar(&RunBar::Done { left: 6, of: 7 }, "⌘J").unwrap();
        assert_eq!(done.title, "Done · 6 left");
        assert_eq!(done.tone, RunTone::Done);
        assert_eq!(
            done.segments
                .iter()
                .filter(|s| **s == Segment::Done)
                .count(),
            1
        );
        assert_eq!(
            run_bar(
                &RunBar::AllClear {
                    handled: 7,
                    minutes: 14
                },
                "⌘J"
            ),
            None
        );
        let clear = all_clear(7, 14, 3);
        assert_eq!(
            clear.text,
            "7 items handled in 14 min. 3 agents running; nothing needs you right now."
        );
        assert_eq!(clear.segments, 7);
    }

    #[test]
    fn the_resume_all_dialog_lists_its_tickets() {
        let m = model(vec![
            item("SHOP-25", Status::Interrupted, NeedsYouGroup::Stopped),
            item("SHOP-28", Status::StartInterrupted, NeedsYouGroup::Stopped),
        ]);
        let d = resume_all_dialog(&m.resume_all_confirm().unwrap());
        assert_eq!(d.title, "Resume 1 interrupted session?");
        assert_eq!(d.included[0].key, "SHOP-25");
        assert_eq!(d.confirm, "Resume 1");
        assert!(d.not_included.unwrap().starts_with("Not included: SHOP-28"));
    }
}
