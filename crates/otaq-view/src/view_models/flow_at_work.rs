//! Agents at work during Flow setup (T-0434; ASK-06, d0108 "Each connected tool's agent looks for itself, when the
//! user asks", d0086, d0088; *14 → Row · Flow step: the agent at work* `s14-14`/`s14-15`, update AL): the tabs over
//! the flow agents' terminals and the **Agents at work** card, from the project's Flow setup
//! ([`otaq_core::project_agents::first_flow::FlowSetup`]). The onboarding Flow step and a later project's Flow setup
//! page show the same view.
//!
//! - **The tabs:** one per asked tool ("Claude Code · Working · 2 min"); the shown one's terminal fills the left. Its
//!   header is the agent's own ("Flow setup · Claude Code"); at the limit it reads "Stopped", and an ended one's foot
//!   says "Read-only · the session ended".
//! - **The card:** a row per connected tool (and per tool found but not signed in) with its state and buttons:
//!   Working · 2 min (Watch / Watching, **Stop**: the agent ends to Recently ended), Not asked (**Ask <Tool>**), the
//!   plan limit ("Plan limit · resets 3:45 pm · picks up where it stopped", **Try again at 3:45 pm**: its tool's Resume
//!   once the limit resets, "continue" into the same session), not signed in (**Sign in…**, then it is asked), ended
//!   without a proposal (**Try again** resumes the same conversation; its terminal read-only), and Proposed. One
//!   tool's failure never touches another row. Under them **Stop all and use a preset** · **Skip, use the default**.

use otaq_core::agent_tools::Tool;
use otaq_core::project_agents::first_flow::{FlowSetup, FlowSetupRow, FlowSetupTool};
use otaq_core::tools::Os;
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::theme::{Palette, Status as Legend};
use crate::view_models::clock::{duration, reset_label};
use crate::view_models::onboarding_page::SKIP_USE_THE_DEFAULT;
use crate::view_models::status::PillView;
use crate::view_models::tool_avatar::ToolAvatarView;
use crate::view_models::tool_problems::{SIGN_IN, Tools};

/// Where a tool's row stands. `FlowAtWorkRowState` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "FlowAtWorkRowState")]
pub enum RowState {
    NotAsked,
    Working,
    PlanLimit,
    NotSignedIn,
    Ended,
    Proposed,
}

/// What a row's button does. `FlowAtWorkAction` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "FlowAtWorkAction")]
pub enum Action {
    /// `onboarding.ask { project, tool }`: Ask <Tool>, and Try again at a limit no agent was asked under.
    Ask { tool: String },
    /// `pane.end { id, confirm }`: Stop; the agent ends to Recently ended.
    Stop { agent: String },
    /// `limits.resume_tool { tool }`: "continue" typed into the same session (USG-06, the limit's path).
    ResumeTool { tool: String },
    /// `pane.resume { id }`: the same conversation, in a new session (d0088).
    Resume { agent: String },
    /// `agent_tools.sign_in { tool }` (the Tools step's Sign in…); once its terminal closes the tool is checked again
    /// and asked.
    SignIn { tool: String },
}

/// A row's button: Stop, Ask OpenCode, Try again at 3:45 pm, Try again, Sign in…. `FlowAtWorkButton` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowAtWorkButton")]
pub struct Button {
    pub label: String,
    pub icon: IconName,
    /// Off until the limit resets.
    pub enabled: bool,
    /// Drawn quiet (Stop) rather than outlined.
    pub quiet: bool,
    pub action: Action,
}

/// One tool's row in the card. `FlowAtWorkRow` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowAtWorkRow")]
pub struct Row {
    /// The tool's id (`claude`).
    pub tool: String,
    pub avatar: ToolAvatarView,
    /// "Claude Code".
    pub name: String,
    pub state: RowState,
    /// "Working · 2 min", "Not asked", "Plan limit · resets 3:45 pm · picks up where it stopped", "Couldn’t start ·
    /// not signed in", "Ended without a proposal", "Proposed".
    pub status: String,
    /// The status in the error colour (a failure).
    pub error: bool,
    /// Its flow agent, whose terminal Watch shows; `None` before it was asked.
    pub agent: Option<String>,
    /// Its open proposal (Proposed).
    pub proposal: Option<u32>,
    pub buttons: Vec<Button>,
}

/// One tab over the terminals: an asked tool's flow agent. `FlowAtWorkTab` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowAtWorkTab")]
pub struct Tab {
    pub agent: String,
    pub tool: String,
    pub avatar: ToolAvatarView,
    pub name: String,
    /// "Working · 2 min", "Stopped", "Ended", "Proposed".
    pub status: String,
    /// The dot's colours.
    pub dot: Palette,
    /// The terminal header's state when it isn't the pane's: "Stopped" at the limit.
    pub pill: Option<PillView>,
    /// The terminal's foot when it isn't the pane's: "Read-only · the session ended".
    pub foot: Option<String>,
}

/// The tabs and the Agents at work card. `FlowAtWorkView` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowAtWorkView")]
pub struct View {
    /// "Agents at work".
    pub title: String,
    pub text: String,
    pub rows: Vec<Row>,
    /// In the rows' order; empty before any tool was asked.
    pub tabs: Vec<Tab>,
    /// A row's Watch button, and the words in its place while its terminal shows.
    pub watch: String,
    pub watching: String,
    /// "Stop all and use a preset": ends [`Self::running`], then the presets.
    pub stop_all: String,
    /// The flow agents Stop all ends: working, or stopped at the limit.
    pub running: Vec<String>,
    /// "Skip, use the default" (`onboarding.use_default`).
    pub skip: String,
    /// How many flow agents work now, for the title bar ("2 agents at work").
    pub working: usize,
}

pub const AGENTS_AT_WORK: &str = "Agents at work";
pub const WATCH: &str = "Watch";
pub const WATCHING: &str = "Watching";
pub const STOP: &str = "Stop";
pub const STOP_ALL: &str = "Stop all and use a preset";
pub const NOT_ASKED: &str = "Not asked";
pub const ENDED_WITHOUT_A_PROPOSAL: &str = "Ended without a proposal";
pub const PROPOSED: &str = "Proposed";
pub const TRY_AGAIN: &str = "Try again";
pub const READ_ONLY: &str = "Read-only \u{b7} the session ended";

/// "this Mac" on macOS, else "this computer".
pub fn computer(os: Os) -> &'static str {
    match os {
        Os::Mac => "this Mac",
        Os::Windows | Os::Linux => "this computer",
    }
}

/// "Ask Claude Code".
pub fn ask_label(tool: Tool) -> String {
    format!("Ask {}", tool.display_name())
}

/// When a limit resets in a sentence: "at 3:45 pm" within a day, else "Mon" or "Nov 1".
fn at_word(at: i64, now: i64) -> String {
    let label = reset_label(at, now);
    if at - now < 24 * 3_600_000 {
        format!("at {label}")
    } else {
        label
    }
}

/// The reset time still ahead for `tool`'s limit: none once it passed or was seen to reset.
fn reset_ahead(tool: Tool, resets_at: Option<i64>, tools: &Tools) -> Option<i64> {
    let seen = tools
        .limits
        .iter()
        .any(|l| l.tool == tool && l.reset_seen_at.is_some());
    resets_at.filter(|at| *at > tools.now && !seen)
}

fn button(label: impl Into<String>, icon: IconName, action: Action) -> Button {
    Button {
        label: label.into(),
        icon,
        enabled: true,
        quiet: false,
        action,
    }
}

/// One tool's row, and its tab when it was asked.
fn row(entry: &FlowSetupTool, tools: &Tools) -> (Row, Option<Tab>) {
    let tool = entry.tool;
    let id = tool.as_str().to_owned();
    let now = tools.now;
    let mut row = Row {
        tool: id.clone(),
        avatar: ToolAvatarView::of(tool, None),
        name: tool.display_name().to_owned(),
        state: RowState::NotAsked,
        status: String::new(),
        error: false,
        agent: None,
        proposal: None,
        buttons: Vec::new(),
    };
    let tab = |agent: &str, status: String, dot: Palette| Tab {
        agent: agent.to_owned(),
        tool: id.clone(),
        avatar: ToolAvatarView::of(tool, None),
        name: tool.display_name().to_owned(),
        status,
        dot,
        pill: None,
        foot: None,
    };
    let tab = match &entry.row {
        FlowSetupRow::NotAsked => {
            row.status = NOT_ASKED.to_owned();
            row.buttons.push(button(
                ask_label(tool),
                IconName::Sparkles,
                Action::Ask { tool: id.clone() },
            ));
            None
        }
        FlowSetupRow::Working { agent, since } => {
            let elapsed = format!("Working \u{b7} {}", duration(now - since));
            row.state = RowState::Working;
            row.status = elapsed.clone();
            row.agent = Some(agent.clone());
            row.buttons.push(Button {
                quiet: true,
                ..button(
                    STOP,
                    IconName::CircleStop,
                    Action::Stop {
                        agent: agent.clone(),
                    },
                )
            });
            Some(tab(agent, elapsed, Palette::Working))
        }
        FlowSetupRow::PlanLimit { agent, resets_at } => {
            let ahead = reset_ahead(tool, *resets_at, tools);
            let mut status = "Plan limit".to_owned();
            if let Some(at) = ahead {
                status.push_str(&format!(" \u{b7} resets {}", reset_label(at, now)));
            }
            if agent.is_some() {
                status.push_str(" \u{b7} picks up where it stopped");
            }
            row.state = RowState::PlanLimit;
            row.status = status;
            row.error = true;
            row.agent = agent.clone();
            let label = match ahead {
                Some(at) => format!("{TRY_AGAIN} {}", at_word(at, now)),
                None => TRY_AGAIN.to_owned(),
            };
            // An agent asked before the limit picks up where it stopped; none asked yet is asked once it resets.
            let action = match agent {
                Some(_) => Action::ResumeTool { tool: id.clone() },
                None => Action::Ask { tool: id.clone() },
            };
            row.buttons.push(Button {
                enabled: ahead.is_none(),
                ..button(label, IconName::Clock, action)
            });
            agent.as_deref().map(|agent| Tab {
                pill: Some(PillView {
                    icon: IconName::CirclePause,
                    label: "Stopped".to_owned(),
                    ..PillView::legend(Legend::Queued, None)
                }),
                ..tab(agent, "Stopped".to_owned(), Palette::Error)
            })
        }
        FlowSetupRow::NotSignedIn => {
            row.state = RowState::NotSignedIn;
            row.status = otaq_core::project_agents::first_flow::NOT_SIGNED_IN.to_owned();
            row.error = true;
            row.buttons.push(button(
                SIGN_IN,
                IconName::LogIn,
                Action::SignIn { tool: id.clone() },
            ));
            None
        }
        FlowSetupRow::Ended { agent } => {
            row.state = RowState::Ended;
            row.status = ENDED_WITHOUT_A_PROPOSAL.to_owned();
            row.error = true;
            row.agent = Some(agent.clone());
            row.buttons.push(button(
                TRY_AGAIN,
                IconName::RotateCcw,
                Action::Resume {
                    agent: agent.clone(),
                },
            ));
            Some(Tab {
                foot: Some(READ_ONLY.to_owned()),
                ..tab(agent, "Ended".to_owned(), Palette::Queued)
            })
        }
        FlowSetupRow::Proposed { agent, proposal } => {
            row.state = RowState::Proposed;
            row.status = PROPOSED.to_owned();
            row.agent = Some(agent.clone());
            row.proposal = Some(*proposal);
            Some(tab(agent, PROPOSED.to_owned(), Palette::Approved))
        }
    };
    (row, tab)
}

/// The tabs and the card for `setup`, with the tools as they are now (`tools.now`, their limits), worded for `os`.
pub fn view(setup: &FlowSetup, tools: &Tools, os: Os) -> View {
    let (rows, tabs): (Vec<Row>, Vec<Option<Tab>>) =
        setup.tools.iter().map(|t| row(t, tools)).unzip();
    let running = setup
        .tools
        .iter()
        .filter_map(|t| match &t.row {
            FlowSetupRow::Working { agent, .. } => Some(agent.clone()),
            FlowSetupRow::PlanLimit { agent, .. } => agent.clone(),
            _ => None,
        })
        .collect();
    View {
        title: AGENTS_AT_WORK.to_owned(),
        text: format!(
            "Each looks at how you use it on {} and at this repo. Watch any of them in its terminal, or come back \
             later: the proposals wait for you.",
            computer(os)
        ),
        working: rows.iter().filter(|r| r.state == RowState::Working).count(),
        rows,
        tabs: tabs.into_iter().flatten().collect(),
        watch: WATCH.to_owned(),
        watching: WATCHING.to_owned(),
        stop_all: STOP_ALL.to_owned(),
        running,
        skip: SKIP_USE_THE_DEFAULT.to_owned(),
    }
}

/// One frame of the gallery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowAtWorkGalleryCell")]
pub struct GalleryCell {
    pub name: String,
    /// The tab shown (Watching), as in the frame.
    pub watching: Option<String>,
    pub view: View,
}

/// Agents at work in the component gallery (`gallery.onboarding_flow_at_work`): the five frames of `s14-14`, on the
/// frames' times.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowAtWorkGallery")]
pub struct Gallery {
    pub cells: Vec<GalleryCell>,
}

pub fn gallery() -> Gallery {
    use fixtures::{CC, OC, frames};
    let cells = frames()
        .into_iter()
        .map(|(name, setup, tools)| {
            let view = view(&setup, &tools, Os::Mac);
            // The frames watch Claude Code's terminal when it has one, else OpenCode's.
            let watching = [CC, OC]
                .into_iter()
                .find(|a| view.tabs.iter().any(|t| t.agent == *a))
                .map(str::to_owned);
            GalleryCell {
                name: name.to_owned(),
                watching,
                view,
            }
        })
        .collect();
    Gallery { cells }
}

/// The example's Flow setup (pocket-notes, `s14-14`'s shop-web), for the gallery and the tests.
pub mod fixtures {
    use otaq_core::agent_tools::Tool;
    use otaq_core::agent_tools::connected::AgentToolState;
    use otaq_core::project_agents::first_flow::{FlowSetup, FlowSetupRow, FlowSetupTool};

    use crate::view_models::tool_problems::Tools;
    use crate::view_models::tool_problems::fixtures::{connected, status};

    /// 2026-10-21 12:33 UTC.
    pub const NOW: i64 = 1_792_585_980_000;
    /// The flow agents' ids.
    pub const CC: &str = "a-cc";
    pub const OC: &str = "a-oc";
    pub const CLAUDE: Tool = Tool::ALL[0];
    pub const OPENCODE: Tool = Tool::ALL[3];

    /// Claude Code and OpenCode connected, at `NOW`.
    pub fn tools() -> Tools {
        let mut tools = Tools::from_statuses(vec![
            connected(CLAUDE),
            status(Tool::ALL[1], AgentToolState::NotInstalled),
            status(Tool::ALL[2], AgentToolState::NotInstalled),
            connected(OPENCODE),
        ]);
        tools.now = NOW;
        tools
    }

    pub fn working(agent: &str, minutes: i64) -> FlowSetupRow {
        FlowSetupRow::Working {
            agent: agent.into(),
            since: NOW - minutes * 60_000 - 5_000,
        }
    }

    /// shop-web's Flow setup with Claude Code's and OpenCode's rows.
    pub fn setup(claude: FlowSetupRow, opencode: FlowSetupRow) -> FlowSetup {
        FlowSetup {
            project: "SHOP".into(),
            project_name: "shop-web".into(),
            since: Some(NOW - 10 * 60_000),
            tools: vec![
                FlowSetupTool {
                    tool: CLAUDE,
                    row: claude,
                },
                FlowSetupTool {
                    tool: OPENCODE,
                    row: opencode,
                },
            ],
            proposals: Vec::new(),
        }
    }

    /// The reset of the plan limit frame: 72 minutes after `NOW`.
    pub const RESETS_AT: i64 = NOW + 72 * 60_000;

    /// The five frames of `s14-14`, by name, with their tools.
    pub fn frames() -> Vec<(&'static str, FlowSetup, Tools)> {
        let tools = tools();
        let mut signed_out = tools.clone();
        signed_out.statuses[3] =
            status(OPENCODE, AgentToolState::FoundNotSignedIn { version: None });
        signed_out.connected.retain(|t| *t != OPENCODE);
        vec![
            (
                "Two agents at work",
                setup(working(CC, 2), working(OC, 1)),
                tools.clone(),
            ),
            (
                "Asking another tool later",
                setup(working(CC, 2), FlowSetupRow::NotAsked),
                tools.clone(),
            ),
            (
                "One agent stops, the other goes on",
                setup(
                    FlowSetupRow::PlanLimit {
                        agent: Some(CC.into()),
                        resets_at: Some(RESETS_AT),
                    },
                    working(OC, 3),
                ),
                tools.clone(),
            ),
            (
                "Not signed in",
                setup(working(CC, 2), FlowSetupRow::NotSignedIn),
                signed_out,
            ),
            (
                "One agent ended without a proposal",
                setup(FlowSetupRow::Ended { agent: CC.into() }, working(OC, 2)),
                tools,
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{CC, NOW, OC, RESETS_AT, frames, setup, tools, working};
    use super::*;

    /// Each row as the card draws it: name, status, buttons.
    fn rows(v: &View) -> Vec<(String, String, Vec<String>)> {
        v.rows
            .iter()
            .map(|r| {
                (
                    r.name.clone(),
                    r.status.clone(),
                    r.buttons.iter().map(|b| b.label.clone()).collect(),
                )
            })
            .collect()
    }

    fn frame(name: &str) -> View {
        let (_, setup, tools) = frames().into_iter().find(|f| f.0 == name).unwrap();
        view(&setup, &tools, Os::Mac)
    }

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn two_agents_at_work_s14_14() {
        let v = frame("Two agents at work");
        assert_eq!(v.title, "Agents at work");
        assert_eq!(
            v.text,
            "Each looks at how you use it on this Mac and at this repo. Watch any of them in its terminal, or come \
             back later: the proposals wait for you."
        );
        assert_eq!(
            rows(&v),
            [
                (
                    "Claude Code".into(),
                    "Working · 2 min".into(),
                    strings(&["Stop"])
                ),
                (
                    "OpenCode".into(),
                    "Working · 1 min".into(),
                    strings(&["Stop"])
                ),
            ]
        );
        assert!(v.rows.iter().all(|r| !r.error && r.buttons[0].quiet));
        assert_eq!(
            v.rows[1].buttons[0].action,
            Action::Stop { agent: OC.into() }
        );
        // Watch / Watching: the row's terminal, the page knows which shows.
        assert_eq!(
            (v.watch.as_str(), v.watching.as_str()),
            ("Watch", "Watching")
        );
        assert_eq!(v.rows[0].agent.as_deref(), Some(CC));
        // One tab per asked tool, in the rows' order.
        let tabs: Vec<(&str, &str, &str)> = v
            .tabs
            .iter()
            .map(|t| (t.agent.as_str(), t.name.as_str(), t.status.as_str()))
            .collect();
        assert_eq!(
            tabs,
            [
                (CC, "Claude Code", "Working · 2 min"),
                (OC, "OpenCode", "Working · 1 min")
            ]
        );
        assert!(
            v.tabs
                .iter()
                .all(|t| t.dot == Palette::Working && t.pill.is_none() && t.foot.is_none())
        );
        assert_eq!(v.tabs[0].avatar.letters, "CC");
        assert_eq!(v.tabs[1].avatar.letters, "OC");
        assert_eq!(
            (v.stop_all.as_str(), v.skip.as_str()),
            ("Stop all and use a preset", "Skip, use the default")
        );
        assert_eq!(v.running, [CC, OC]);
        assert_eq!(v.working, 2);
    }

    #[test]
    fn a_tool_not_asked_yet_can_be_asked_while_the_other_works_s14_14() {
        let v = frame("Asking another tool later");
        assert_eq!(
            rows(&v),
            [
                (
                    "Claude Code".into(),
                    "Working · 2 min".into(),
                    strings(&["Stop"])
                ),
                (
                    "OpenCode".into(),
                    "Not asked".into(),
                    strings(&["Ask OpenCode"])
                ),
            ]
        );
        let ask = &v.rows[1].buttons[0];
        assert!(ask.enabled && !ask.quiet);
        assert_eq!(
            ask.action,
            Action::Ask {
                tool: "opencode".into()
            }
        );
        assert_eq!(v.rows[1].agent, None);
        assert_eq!(v.tabs.len(), 1, "a tab only for the asked tool");
        assert_eq!(v.running, [CC]);
    }

    #[test]
    fn the_plan_limit_stops_one_and_the_other_goes_on_s14_14() {
        let v = frame("One agent stops, the other goes on");
        let reset = reset_label(RESETS_AT, NOW);
        assert_eq!(
            rows(&v),
            [
                (
                    "Claude Code".into(),
                    format!("Plan limit · resets {reset} · picks up where it stopped"),
                    vec![format!("Try again at {reset}")]
                ),
                (
                    "OpenCode".into(),
                    "Working · 3 min".into(),
                    strings(&["Stop"])
                ),
            ]
        );
        let retry = &v.rows[0].buttons[0];
        assert!(!retry.enabled, "off until the limit resets");
        assert_eq!(
            retry.action,
            Action::ResumeTool {
                tool: "claude".into()
            }
        );
        assert!(v.rows[0].error);
        assert_eq!(v.rows[0].state, RowState::PlanLimit);
        let tab = &v.tabs[0];
        assert_eq!((tab.status.as_str(), tab.dot), ("Stopped", Palette::Error));
        assert_eq!(tab.pill.as_ref().unwrap().label, "Stopped");
        assert_eq!(v.running, [CC, OC], "Stop all ends the stopped one too");
        assert_eq!(v.working, 1);

        // The reset came: Try again picks up now.
        let (_, s, mut later) = frames().remove(2);
        later.now = RESETS_AT + 60_000;
        let v = view(&s, &later, Os::Mac);
        assert_eq!(v.rows[0].status, "Plan limit · picks up where it stopped");
        assert_eq!(v.rows[0].buttons[0].label, "Try again");
        assert!(v.rows[0].buttons[0].enabled);

        // At the limit before it was asked: Try again asks it once the limit resets.
        let fresh = setup(
            working(CC, 2),
            FlowSetupRow::PlanLimit {
                agent: None,
                resets_at: Some(RESETS_AT),
            },
        );
        let v = view(&fresh, &tools(), Os::Mac);
        assert_eq!(v.rows[1].status, format!("Plan limit · resets {reset}"));
        assert_eq!(
            v.rows[1].buttons[0].action,
            Action::Ask {
                tool: "opencode".into()
            }
        );
        assert_eq!(v.tabs.len(), 1);
        // A reset at an unknown time: Try again is the user's call.
        let unknown = setup(
            FlowSetupRow::PlanLimit {
                agent: Some(CC.into()),
                resets_at: None,
            },
            FlowSetupRow::NotAsked,
        );
        let v = view(&unknown, &tools(), Os::Mac);
        assert_eq!(v.rows[0].status, "Plan limit · picks up where it stopped");
        assert!(v.rows[0].buttons[0].enabled);
    }

    #[test]
    fn a_tool_not_signed_in_offers_sign_in_s14_14() {
        let v = frame("Not signed in");
        assert_eq!(
            rows(&v),
            [
                (
                    "Claude Code".into(),
                    "Working · 2 min".into(),
                    strings(&["Stop"])
                ),
                (
                    "OpenCode".into(),
                    "Couldn’t start · not signed in".into(),
                    strings(&["Sign in…"])
                ),
            ]
        );
        assert!(v.rows[1].error);
        assert_eq!(
            v.rows[1].buttons[0].action,
            Action::SignIn {
                tool: "opencode".into()
            }
        );
        assert_eq!(v.tabs.len(), 1);
    }

    #[test]
    fn one_ended_without_a_proposal_offers_try_again_and_reads_only_s14_14() {
        let v = frame("One agent ended without a proposal");
        assert_eq!(
            rows(&v),
            [
                (
                    "Claude Code".into(),
                    "Ended without a proposal".into(),
                    strings(&["Try again"])
                ),
                (
                    "OpenCode".into(),
                    "Working · 2 min".into(),
                    strings(&["Stop"])
                ),
            ]
        );
        assert_eq!(
            v.rows[0].buttons[0].action,
            Action::Resume { agent: CC.into() }
        );
        let tab = &v.tabs[0];
        assert_eq!((tab.status.as_str(), tab.dot), ("Ended", Palette::Queued));
        assert_eq!(tab.foot.as_deref(), Some("Read-only · the session ended"));
        assert_eq!(tab.pill, None, "the pane's Ended");
        assert_eq!(v.running, [OC]);
    }

    #[test]
    fn a_proposal_shows_its_row_as_proposed() {
        let s = setup(
            FlowSetupRow::Proposed {
                agent: CC.into(),
                proposal: 7,
            },
            working(OC, 4),
        );
        let v = view(&s, &tools(), Os::Mac);
        assert_eq!(v.rows[0].status, "Proposed");
        assert_eq!(v.rows[0].proposal, Some(7));
        assert!(v.rows[0].buttons.is_empty());
        assert_eq!(v.tabs[0].status, "Proposed");
        assert_eq!(v.running, [OC]);
    }

    #[test]
    fn the_elapsed_time_counts_in_whole_minutes_and_other_systems_say_this_computer() {
        let s = setup(working(CC, 125), FlowSetupRow::NotAsked);
        let v = view(&s, &tools(), Os::Linux);
        assert_eq!(v.rows[0].status, "Working · 2 h 5 min");
        assert!(v.text.contains("on this computer and at this repo"));
    }

    #[test]
    fn the_gallery_has_the_five_frames() {
        let g = gallery();
        let names: Vec<&str> = g.cells.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Two agents at work",
                "Asking another tool later",
                "One agent stops, the other goes on",
                "Not signed in",
                "One agent ended without a proposal"
            ]
        );
        assert!(g.cells.iter().all(|c| c.watching.as_deref() == Some(CC)));
    }
}
