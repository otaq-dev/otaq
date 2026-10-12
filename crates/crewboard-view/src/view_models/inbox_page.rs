//! The Needs-you inbox as the page draws it (T-0230; UI-06, ATT-06…ATT-10, ATT-12, USG-07, MSG-05, CLS-04; *10 ·
//! Needs-you inbox*): the head with the total and Next ⌘J, the usage-limit row once at the top, the groups with their
//! counts and Resume all, each row with its project, reason, age, the answer box's texts and options and its
//! buttons, the empty state and the foot's keys; the ⌘J run bar on top of the panel and "All clear"; the Resume all
//! confirmation with its ticket list. Every text is worded here, from `inbox.rs`, `inbox_run.rs`, `answer.rs` and
//! `answer_options.rs`; the page keeps only the selection and the draft.

use crewboard_core::board::needs_you::NeedsYouGroup;
use crewboard_core::domain::TicketKey;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;
use crate::view_models::answer::AnswerKind;
use crate::view_models::answer_options::{self, AnswerOptions};
use crate::view_models::inbox::{InboxGroupView, InboxRow, ResumeAllConfirm, RowAction};
use crate::view_models::inbox_run::{ALL_CLEAR_FOR, RunBar, all_clear_text, bar_title};
use crate::view_models::limit_banner::LimitBanner;
use crate::view_models::panel::Tab;
use crate::view_models::status::PillView;

/// A key hint: its keys on this platform and what they do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxKeyHint")]
pub struct KeyHint {
    pub keys: String,
    pub label: &'static str,
}

/// What a row's button does; the page sends it back with `inbox.act`. `InboxRowAction` on the page. Later item
/// kinds (validation, Auto-merge, Project agents) add their own.
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
}

impl Action {
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
    };
    Button {
        action: Action::of(action),
        label,
        icon,
        primary,
    }
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
}

fn row(r: &InboxRow) -> Row {
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
        buttons: r.actions.iter().map(button).collect(),
    }
}

/// Which group. `InboxGroupId` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "InboxGroupId")]
pub enum GroupId {
    Orange,
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

fn group(g: &InboxGroupView) -> Group {
    Group {
        id: match g.group {
            NeedsYouGroup::Orange => GroupId::Orange,
            NeedsYouGroup::Stopped => GroupId::Stopped,
            NeedsYouGroup::Review => GroupId::Review,
        },
        title: g.title,
        count: g.count as u32,
        resume_all: g.resume_all.map(|n| format!("Resume all ({n})")),
        rows: g.rows.iter().map(row).collect(),
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

/// The inbox from the model's groups, its total, the usage-limit banner, the agents running and ⌘J's keys.
pub fn view(
    groups: &[InboxGroupView],
    total: u32,
    limit: Option<&LimitBanner>,
    running: u32,
    next_keys: &str,
) -> View {
    let empty = groups.is_empty().then(|| {
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
        foot: if groups.is_empty() {
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
        groups: groups.iter().map(group).collect(),
        empty,
    }
}

/// How the run bar looks: a step (orange), the open item answered (green). `InboxRunTone` on the page.
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

    use crewboard_core::attention::inbox::{InboxCounts, InboxList};
    use crewboard_core::board::needs_you::NeedsYouItem;
    use crewboard_core::board::questions::HumanQuestion;
    use crewboard_core::domain::Status;

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

    #[test]
    fn the_inbox_has_its_head_groups_rows_and_foot() {
        let mut asks = item("SHOP-21", Status::AsksYou, NeedsYouGroup::Orange);
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
        let v = view(&m.groups(&|_| None), 3, None, 3, "⌘J");
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

    #[test]
    fn an_empty_inbox_says_what_happens_next() {
        let v = view(&[], 0, None, 1, "⌘J");
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
