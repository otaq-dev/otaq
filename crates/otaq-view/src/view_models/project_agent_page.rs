//! A Project agent's tab as the page draws it (T-0243; UI-09, PAG-04, PAG-05, PAG-06, TKT-08; d0068, d0060, d0019;
//! *18 · Project agents & terminals → Row · states, Row · lifecycle*): the Agent header above its terminal (name with
//! Rename, the state pill, tokens, what it may do, "named by you", Resume once it ended), the **Created and changed**
//! list beside it (links, Delete on the tickets it created), Close mid-turn's confirmation and Recently ended.

use otaq_core::domain::{Column, Status};
use otaq_core::project_agents::{AgentState, CreatedAndChanged, NameSource, ProjectAgent};
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::theme::Status as Legend;
use crate::view_models::card::tokens_label;
use crate::view_models::clock::local_hhmm;
use crate::view_models::status::PillView;

/// What every Project agent may do, under its name (d0068).
pub const SCOPE: &str = "reads & runs code \u{b7} never edits \u{b7} Backlog only";

/// The header's state pill: a legend entry's colours with the frame's icon and label.
fn pill(legend: Legend, icon: IconName, label: &str) -> PillView {
    PillView {
        icon,
        label: label.to_owned(),
        ..PillView::legend(legend, None)
    }
}

/// A quiet pill: the queued colours (grey) with the frame's icon and label.
fn quiet(icon: IconName, label: &str) -> PillView {
    pill(Legend::Queued, icon, label)
}

/// The state pill (*18 → Row · More pane states*, *Row · Switching agents*). Only waiting is turquoise (d0077): a
/// permission prompt or a question; idle and ended are quiet.
pub fn state_pill(agent: &ProjectAgent) -> PillView {
    match agent.state {
        AgentState::Ended => quiet(IconName::CircleStop, "Ended"),
        AgentState::EndedByRestart => quiet(IconName::CircleStop, "Ended by restart"),
        AgentState::Running if agent.needs_you.is_some() => {
            pill(Legend::NeedsYou, IconName::Hand, "Waiting for you")
        }
        AgentState::Running if agent.question.is_some() => pill(
            Legend::AsksYou,
            IconName::MessageCircleQuestionMark,
            "Waiting for you",
        ),
        AgentState::Running if agent.mid_turn => {
            pill(Legend::AgentWorking, IconName::Activity, "Working")
        }
        AgentState::Running => quiet(IconName::CirclePause, "Idle"),
    }
}

/// Resume on an ended agent's header: a quiet button after a normal end, the main one after a restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentResumeButton")]
pub struct ResumeButton {
    pub label: &'static str,
    pub primary: bool,
}

/// The Agent header. `AgentHeader` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct AgentHeader {
    pub id: String,
    pub name: String,
    /// "New agent" reads muted.
    pub unnamed: bool,
    pub rename: &'static str,
    pub pill: PillView,
    /// "48k".
    pub tokens: String,
    pub scope: &'static str,
    /// The user named it: "named by you · the agent can’t rename it".
    pub named_by_you: Option<&'static str>,
    pub resume: Option<ResumeButton>,
}

pub fn header(agent: &ProjectAgent) -> AgentHeader {
    AgentHeader {
        id: agent.id.clone(),
        name: agent.name.clone(),
        unnamed: agent.name_source.is_none(),
        rename: "Rename",
        pill: state_pill(agent),
        tokens: tokens_label(agent.tokens.shown()),
        scope: SCOPE,
        named_by_you: (agent.name_source == Some(NameSource::User))
            .then_some("named by you \u{b7} the agent can\u{2019}t rename it"),
        resume: resume_button(agent),
    }
}

/// Resume once it ended: quiet after a normal end, the main button after a restart.
pub fn resume_button(agent: &ProjectAgent) -> Option<ResumeButton> {
    match agent.state {
        AgentState::Running => None,
        AgentState::Ended => Some(ResumeButton {
            label: "Resume",
            primary: false,
        }),
        AgentState::EndedByRestart => Some(ResumeButton {
            label: "Resume",
            primary: true,
        }),
    }
}

/// One row of Created and changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CreatedRow")]
pub struct CreatedRow {
    /// The ticket the link opens.
    pub key: String,
    /// Its title, or what the agent changed ("edited").
    pub text: String,
    pub edited: bool,
    /// "Delete": it never started (TKT-08, the 0060 confirmation).
    pub delete: Option<&'static str>,
}

/// Created and changed (*Row · states*). `CreatedView` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct CreatedView {
    pub title: &'static str,
    /// What it touched, as a number ("4"); `None` while nothing.
    pub count: Option<String>,
    /// "Created SHOP-35, SHOP-36 · edited SHOP-31 · 1 dependency".
    pub summary: Option<String>,
    pub rows: Vec<CreatedRow>,
    pub empty: &'static str,
}

pub fn created(c: &CreatedAndChanged) -> CreatedView {
    let n = c.created.len() + c.edited.len() + c.dependencies as usize;
    let mut rows: Vec<CreatedRow> = c
        .created
        .iter()
        .map(|t| CreatedRow {
            key: t.key.to_string(),
            text: t.title.clone(),
            edited: false,
            delete: t.deletable.then_some("Delete"),
        })
        .collect();
    rows.extend(c.edited.iter().map(|key| CreatedRow {
        key: key.to_string(),
        text: "edited".to_owned(),
        edited: true,
        delete: None,
    }));
    CreatedView {
        title: "CREATED AND CHANGED",
        count: (n > 0).then(|| n.to_string()),
        summary: (n > 0 && !c.summary.is_empty()).then(|| c.summary.clone()),
        rows,
        empty: "Nothing yet. Tickets this agent creates or edits show up here, each with a link, and Delete for the \
                ones it created.",
    }
}

/// What a column is called in the close confirmation and the pane's Created and changed.
pub fn column_name(status: Status) -> &'static str {
    match status.column() {
        Column::Backlog => "Backlog",
        Column::ReadyForDev => "Ready for Dev",
        Column::Dev => "Dev",
        Column::ReadyForReview => "Ready for Review",
        Column::Approved => "Approved",
        Column::Done => "Done",
    }
}

/// Close mid-turn's confirmation (*Close an agent mid-turn · confirm*). `AgentCloseDialog` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AgentCloseDialog")]
pub struct CloseDialog {
    pub title: String,
    /// In the attention box: what closing does to the turn.
    pub mid_turn: &'static str,
    pub text: String,
    pub confirm: &'static str,
}

pub fn close_dialog(agent: &ProjectAgent, created: &CreatedAndChanged) -> CloseDialog {
    let resume = "You can resume the conversation from Recently ended.";
    let text = if created.created.is_empty() {
        resume.to_owned()
    } else {
        let places: Vec<String> = created
            .created
            .iter()
            .map(|t| format!("{} in {}", t.key, column_name(t.status)))
            .collect();
        format!(
            "The tickets it created stay where they are ({}). {resume}",
            places.join(", ")
        )
    };
    CloseDialog {
        title: format!(
            "Close \u{201c}{}\u{201d} while it\u{2019}s working?",
            agent.name
        ),
        mid_turn: "It\u{2019}s mid-turn. Closing stops it now.",
        text,
        confirm: "Stop and close",
    }
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// When an agent ended: "11:42" today, "yesterday", else "9 Oct".
pub fn ended_when(at_ms: i64, now_ms: i64) -> String {
    use otaq_core::local_time::local;
    let (Some(at), Some(now)) = (local(at_ms), local(now_ms)) else {
        return String::new();
    };
    let same = |a: &otaq_core::local_time::LocalTime, b: &otaq_core::local_time::LocalTime| {
        (a.year, a.month, a.day) == (b.year, b.month, b.day)
    };
    if same(&at, &now) {
        return local_hhmm(at_ms);
    }
    if local(now_ms - 86_400_000).is_some_and(|y| same(&at, &y)) {
        return "yesterday".to_owned();
    }
    format!(
        "{} {}",
        at.day,
        MONTHS[(at.month as usize).saturating_sub(1).min(11)]
    )
}

/// One Recently ended row (*Recently ended · list*).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "RecentAgentRow")]
pub struct RecentRow {
    pub id: String,
    pub name: String,
    /// "yesterday · ended normally".
    pub when: String,
    /// "yesterday": the empty pane's short form.
    pub ended: String,
    pub resume: &'static str,
}

/// Recently ended (*Recently ended · list*, with entries and empty). `RecentAgents` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "RecentAgents")]
pub struct Recent {
    /// "RECENTLY ENDED · SHOP-WEB".
    pub title: String,
    pub rows: Vec<RecentRow>,
    pub empty: &'static str,
}

pub fn recent(agents: &[ProjectAgent], project_name: &str, now_ms: i64) -> Recent {
    Recent {
        title: format!("RECENTLY ENDED \u{b7} {}", project_name.to_uppercase()),
        rows: agents
            .iter()
            .map(|a| {
                let how = match a.state {
                    AgentState::EndedByRestart => "ended by restart",
                    _ => "ended normally",
                };
                RecentRow {
                    id: a.id.clone(),
                    name: a.name.clone(),
                    when: match a.ended_at {
                        Some(at) => format!("{} \u{b7} {how}", ended_when(at, now_ms)),
                        None => how.to_owned(),
                    },
                    ended: a
                        .ended_at
                        .map(|at| ended_when(at, now_ms))
                        .unwrap_or_default(),
                    resume: "Resume",
                }
            })
            .collect(),
        empty: "No ended Project agents yet. Agents you close or that end by a restart show up here, up to 10.",
    }
}

#[cfg(test)]
mod tests {
    use otaq_core::project_agents::CreatedTicket;

    use super::*;

    fn agent(name: &str) -> ProjectAgent {
        ProjectAgent {
            id: "a1".into(),
            project: "SHOP".into(),
            name: name.into(),
            name_source: Some(NameSource::Agent),
            state: AgentState::Running,
            tool: otaq_core::agent_tools::Tool::Claude,
            tool_session_id: Some("s".into()),
            model: None,
            got_request: true,
            mid_turn: true,
            limit_stopped: false,
            created_at: 0,
            ended_at: None,
            closed: false,
            question: None,
            needs_you: None,
            focused: false,
            tokens: otaq_core::usage::Tokens {
                input: 40_000,
                output: 8_000,
                ..Default::default()
            },
            session: None,
            epic: None,
        }
    }

    fn changed() -> CreatedAndChanged {
        CreatedAndChanged {
            created: vec![
                CreatedTicket {
                    key: "SHOP-35".parse().unwrap(),
                    title: "Slow checkout query".into(),
                    status: Status::Queued,
                    deletable: false,
                },
                CreatedTicket {
                    key: "SHOP-36".parse().unwrap(),
                    title: "Cache product prices".into(),
                    status: Status::Backlog,
                    deletable: true,
                },
            ],
            edited: vec!["SHOP-31".parse().unwrap()],
            dependencies: 1,
            summary: "Created SHOP-35, SHOP-36 \u{b7} edited SHOP-31 \u{b7} 1 dependency".into(),
        }
    }

    #[test]
    fn the_header_follows_the_agent_state() {
        let mut a = agent("Checkout slowness");
        let h = header(&a);
        assert_eq!(
            (h.pill.label.as_str(), h.tokens.as_str()),
            ("Working", "48k")
        );
        assert_eq!((h.scope, h.named_by_you, h.resume), (SCOPE, None, None));
        a.needs_you = Some("Bash(pnpm run profile)".into());
        assert_eq!(header(&a).pill.label, "Waiting for you");
        a.needs_you = None;
        a.question = Some(otaq_core::board::questions::HumanQuestion {
            id: "q1".into(),
            text: "Import closed issues too?".into(),
            options: Vec::new(),
            recommended: None,
            asked_at: 0,
            via_lead: None,
        });
        assert_eq!(
            (header(&a).pill.icon, header(&a).pill.label.as_str()),
            (IconName::MessageCircleQuestionMark, "Waiting for you")
        );
        a.question = None;
        a.mid_turn = false;
        assert_eq!(header(&a).pill.label, "Idle");

        let mut new = agent("New agent");
        new.name_source = None;
        assert!(header(&new).unnamed);

        a.name_source = Some(NameSource::User);
        assert_eq!(
            header(&a).named_by_you,
            Some("named by you \u{b7} the agent can\u{2019}t rename it")
        );
        a.state = AgentState::Ended;
        assert_eq!(
            (header(&a).pill.label, header(&a).resume.unwrap().primary),
            ("Ended".to_owned(), false)
        );
        a.state = AgentState::EndedByRestart;
        let h = header(&a);
        assert_eq!(
            (h.pill.label.as_str(), h.resume.unwrap().primary),
            ("Ended by restart", true)
        );
    }

    #[test]
    fn created_and_changed_lists_links_and_delete() {
        let v = created(&changed());
        assert_eq!(v.count.as_deref(), Some("4"));
        assert_eq!(
            v.summary.as_deref(),
            Some("Created SHOP-35, SHOP-36 \u{b7} edited SHOP-31 \u{b7} 1 dependency")
        );
        let rows: Vec<(&str, &str, bool, Option<&str>)> = v
            .rows
            .iter()
            .map(|r| (r.key.as_str(), r.text.as_str(), r.edited, r.delete))
            .collect();
        assert_eq!(
            rows,
            [
                ("SHOP-35", "Slow checkout query", false, None),
                ("SHOP-36", "Cache product prices", false, Some("Delete")),
                ("SHOP-31", "edited", true, None),
            ]
        );
        let none = created(&CreatedAndChanged::default());
        assert_eq!((none.count, none.summary), (None, None));
        assert!(none.empty.starts_with("Nothing yet."));
    }

    #[test]
    fn closing_mid_turn_says_what_stays() {
        let d = close_dialog(&agent("Checkout slowness"), &changed());
        assert_eq!(
            d.title,
            "Close \u{201c}Checkout slowness\u{201d} while it\u{2019}s working?"
        );
        assert_eq!(
            d.text,
            "The tickets it created stay where they are (SHOP-35 in Ready for Dev, SHOP-36 in Backlog). You can \
             resume the conversation from Recently ended."
        );
        assert_eq!(d.confirm, "Stop and close");
        let bare = close_dialog(&agent("x"), &CreatedAndChanged::default());
        assert_eq!(
            bare.text,
            "You can resume the conversation from Recently ended."
        );
    }

    #[test]
    fn recently_ended_says_when_and_how() {
        let now = 1_791_600_000_000;
        let mut a = agent("Footer audit");
        a.state = AgentState::Ended;
        a.ended_at = Some(now - 86_400_000);
        let mut b = agent("Import GitHub issues");
        b.state = AgentState::EndedByRestart;
        b.ended_at = Some(now - 60_000);
        let r = recent(&[a, b], "shop-web", now);
        assert_eq!(r.title, "RECENTLY ENDED \u{b7} SHOP-WEB");
        assert_eq!(r.rows[0].when, "yesterday \u{b7} ended normally");
        assert!(r.rows[1].when.ends_with(" \u{b7} ended by restart"));
        assert_eq!(r.rows[1].when.len(), "11:42 \u{b7} ended by restart".len());
        assert!(recent(&[], "shop-web", now).rows.is_empty());
        assert_eq!(ended_when(now - 10 * 86_400_000, now).split(' ').count(), 2);
    }
}
