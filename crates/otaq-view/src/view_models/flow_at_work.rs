//! The onboarding agent at work (T-0430; ONB-05, d0108, d0086, d0088; *14 → Row · Flow step: the agent at work*
//! `s14-14`/`s14-15`, after update AJ's second and third passes): the card beside the agent's terminal while the Flow
//! step's agent reads how the user works, and its failures.
//!
//! - **Working:** "Reading how you work", the elapsed time ("Working · 2 min", from its session's start) and **Stop
//!   and use a preset**. No per-source rows: Otaq doesn't limit or follow what the agent reads (d0108 changelog
//!   2026-10-21); its terminal shows that.
//! - **Stopped at the plan limit** (the agent's `limit_stopped`): "The agent stopped", the reset line and **Try again
//!   at 3:45 pm**, on once the limit resets: its tool's Resume, which types "continue" into the same session.
//! - **Not signed in** (the Project agent's tool is Found · not signed in, d0086; Otaq can tell for every tool but
//!   Claude Code, whose own sign-in shows in its terminal): "The agent couldn't start" with **Sign in…**.
//! - **Ended without a proposal** (its session ended, by `/exit` or a crash, before it proposed): **Try again**
//!   resumes the same conversation (d0088). A proposal moves the step on before any of this shows.
//!
//! Every failure keeps **Skip, use the default** · **Use a preset instead** one click away.

use otaq_core::agent_tools::Tool;
use otaq_core::agent_tools::connected::AgentToolState;
use otaq_core::project_agents::{AgentState, ProjectAgent};
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::theme::Status as Legend;
use crate::view_models::clock::{duration, in_label, reset_label};
use crate::view_models::onboarding_page::{SKIP_USE_THE_DEFAULT, USE_A_PRESET_INSTEAD};
use crate::view_models::status::PillView;
use crate::view_models::tool_problems::{SIGN_IN, Tools, tool_word};

/// Which card shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "FlowAtWorkKind")]
pub enum Kind {
    Working,
    /// At its tool's plan limit.
    Stopped,
    /// Its tool is Found · not signed in: it didn't start.
    NotSignedIn,
    /// Its session ended before it proposed.
    Ended,
}

/// What the card's main button does. `FlowAtWorkAction` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "FlowAtWorkAction")]
pub enum Action {
    /// `limits.resume_tool { tool }`: "continue" typed into the same session (USG-06, today's limit path).
    ResumeTool { tool: String },
    /// `pane.resume { id }`: the same conversation, in a new session (d0088).
    Resume { agent: String },
    /// `agent_tools.sign_in { tool }` (the Tools step's Sign in…); once its terminal closes, the step checks again
    /// and starts the agent.
    SignIn { tool: String },
}

/// The card's main button: Try again at 3:45 pm, Try again, Sign in….
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowAtWorkButton")]
pub struct Button {
    pub label: String,
    pub icon: IconName,
    /// Off until the limit resets.
    pub enabled: bool,
    pub action: Action,
}

/// The card beside the onboarding agent's terminal, and what its terminal's header and foot say. `FlowAtWorkView` on
/// the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowAtWorkView")]
pub struct View {
    pub kind: Kind,
    /// "Reading how you work", "The agent stopped", "The agent couldn't start", "The agent ended without a proposal".
    pub title: String,
    pub text: String,
    /// Working: "Working · 2 min".
    pub elapsed: Option<String>,
    /// The red line of a failure: "Claude Code limit reached · resets at 3:45 pm (in 1 h 12 min).", "Codex: not
    /// signed in.".
    pub problem: Option<String>,
    /// Working: "Stop and use a preset" (ends the agent, which goes to Recently ended, and opens the presets).
    pub stop: Option<String>,
    /// A failure's "Skip, use the default" and "Use a preset instead".
    pub skip: Option<String>,
    pub use_preset: Option<String>,
    pub retry: Option<Button>,
    /// The terminal header's state when it isn't the pane's: "Stopped" at the limit.
    pub pill: Option<PillView>,
    /// The terminal's foot when it isn't the pane's: "Read-only · the session ended".
    pub foot: Option<String>,
}

pub const READING: &str = "Reading how you work";
pub const READING_TEXT: &str = "This can take a few minutes. Watch the agent in its terminal, or come back later: the proposal waits for you.";
pub const STOP_AND_USE_A_PRESET: &str = "Stop and use a preset";
pub const STOPPED: &str = "The agent stopped";
pub const COULDNT_START: &str = "The agent couldn\u{2019}t start";
pub const ENDED: &str = "The agent ended without a proposal";
pub const ENDED_TEXT: &str =
    "Its conversation ended before it proposed a flow. Try again to pick up where it stopped.";
pub const TRY_AGAIN: &str = "Try again";
pub const READ_ONLY: &str = "Read-only \u{b7} the session ended";

/// An empty card of `kind` titled `title`, for the builders below.
fn card(kind: Kind, title: &str, text: String) -> View {
    View {
        kind,
        title: title.to_owned(),
        text,
        elapsed: None,
        problem: None,
        stop: None,
        skip: None,
        use_preset: None,
        retry: None,
        pill: None,
        foot: None,
    }
}

/// A failure's card: Skip, use the default · Use a preset instead · `retry`.
fn failure(kind: Kind, title: &str, text: String, retry: Button) -> View {
    View {
        skip: Some(SKIP_USE_THE_DEFAULT.to_owned()),
        use_preset: Some(USE_A_PRESET_INSTEAD.to_owned()),
        retry: Some(retry),
        ..card(kind, title, text)
    }
}

/// Whether `tool` is Found · not signed in (d0086). Never for Claude Code, which has no check: its adapter reports it
/// Connected, and its own sign-in shows in the agent's terminal.
pub fn not_signed_in(tool: Tool, tools: &Tools) -> bool {
    tools
        .status(tool)
        .is_some_and(|s| matches!(s.state, AgentToolState::FoundNotSignedIn { .. }))
}

/// The Project agent's `tool` isn't signed in: the agent didn't start (`s14-14 → Not signed in`).
pub fn cannot_start(tool: Tool) -> View {
    let name = tool_word(tool);
    View {
        problem: Some(format!("{name}: not signed in.")),
        ..failure(
            Kind::NotSignedIn,
            COULDNT_START,
            format!("{name} isn\u{2019}t signed in on this computer."),
            Button {
                label: SIGN_IN.to_owned(),
                icon: IconName::LogIn,
                enabled: true,
                action: Action::SignIn {
                    tool: tool.as_str().to_owned(),
                },
            },
        )
    }
}

/// The card for the onboarding agent `agent`, with the tools as they are now (`tools.now`).
pub fn view(agent: &ProjectAgent, tools: &Tools) -> View {
    let now = tools.now;
    if agent.state != AgentState::Running {
        return View {
            foot: Some(READ_ONLY.to_owned()),
            ..failure(
                Kind::Ended,
                ENDED,
                ENDED_TEXT.to_owned(),
                Button {
                    label: TRY_AGAIN.to_owned(),
                    icon: IconName::RotateCcw,
                    enabled: true,
                    action: Action::Resume {
                        agent: agent.id.clone(),
                    },
                },
            )
        };
    }
    if agent.limit_stopped {
        return stopped(agent.tool, tools);
    }
    let started = agent
        .session
        .as_ref()
        .map_or(agent.created_at, |s| s.started_at);
    View {
        elapsed: Some(format!("Working \u{b7} {}", duration(now - started))),
        stop: Some(STOP_AND_USE_A_PRESET.to_owned()),
        ..card(Kind::Working, READING, READING_TEXT.to_owned())
    }
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

/// Stopped at `tool`'s plan limit (`s14-14 → The agent stopped`): Try again waits for the reset while its time is
/// known and still ahead.
fn stopped(tool: Tool, tools: &Tools) -> View {
    let now = tools.now;
    let name = tool_word(tool);
    let limit = tools.limits.iter().find(|l| l.tool == tool);
    let ahead = limit
        .filter(|l| l.reset_seen_at.is_none())
        .and_then(|l| l.resets_at)
        .filter(|at| *at > now);
    let unknown = limit.is_some_and(|l| l.reset_seen_at.is_none() && l.resets_at.is_none());
    let before = format!("{name} reached its plan limit before the proposal was ready.");
    let picks_up = "it picks up where it stopped.";
    let (text, problem, label) = match ahead {
        Some(at) => {
            let when = at_word(at, now);
            (
                format!("{before} Try again {when} \u{b7} {picks_up}"),
                Some(format!(
                    "{name} limit reached \u{b7} resets {when} (in {}).",
                    in_label(at - now)
                )),
                format!("{TRY_AGAIN} {when}"),
            )
        }
        None if unknown => (
            format!("{before} Try again once it resets \u{b7} {picks_up}"),
            Some(format!(
                "{name} limit reached \u{b7} resets at an unknown time."
            )),
            TRY_AGAIN.to_owned(),
        ),
        // It has reset (or its limit was cleared): Try again continues it now.
        None => (
            format!("{before} {TRY_AGAIN} \u{b7} {picks_up}"),
            None,
            TRY_AGAIN.to_owned(),
        ),
    };
    View {
        problem,
        pill: Some(PillView {
            icon: IconName::CirclePause,
            label: "Stopped".to_owned(),
            ..PillView::legend(Legend::Queued, None)
        }),
        ..failure(
            Kind::Stopped,
            STOPPED,
            text,
            Button {
                label,
                icon: IconName::Clock,
                enabled: ahead.is_none(),
                action: Action::ResumeTool {
                    tool: tool.as_str().to_owned(),
                },
            },
        )
    }
}

/// One card of the gallery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowAtWorkGalleryCell")]
pub struct GalleryCell {
    pub name: String,
    pub view: View,
}

/// The agent at work in the component gallery, next to the Flow step's (`gallery.onboarding_flow_at_work`): the
/// three cards of `s14-14` and the ended-without-a-proposal card, on the frame's times.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowAtWorkGallery")]
pub struct Gallery {
    pub cells: Vec<GalleryCell>,
}

pub fn gallery() -> Gallery {
    use crate::view_models::tool_problems::fixtures::{CLAUDE, CODEX, at_limit, several};

    let now = fixtures::NOW;
    let agent = fixtures::agent(now - 2 * 60_000);
    let mut limited = agent.clone();
    limited.limit_stopped = true;
    let mut ended = agent.clone();
    ended.state = AgentState::Ended;
    let tools_at = |mut tools: Tools| {
        tools.now = now;
        tools
    };
    let cells = [
        ("Working", view(&agent, &tools_at(several()))),
        (
            "Plan limit",
            view(
                &limited,
                &tools_at(at_limit(CLAUDE, Some(now + 72 * 60_000))),
            ),
        ),
        ("Not signed in", cannot_start(CODEX)),
        (
            "Ended without a proposal",
            view(&ended, &tools_at(several())),
        ),
    ]
    .into_iter()
    .map(|(name, view)| GalleryCell {
        name: name.to_owned(),
        view,
    })
    .collect();
    Gallery { cells }
}

/// The example's onboarding agent, for the gallery and the tests.
pub mod fixtures {
    use otaq_core::project_agents::{AgentState, NameSource, ProjectAgent};

    use crate::view_models::tool_problems::fixtures::CLAUDE;

    /// 2026-10-21 12:33 UTC.
    pub const NOW: i64 = 1_792_585_980_000;

    /// "Flow setup", running on the first tool since `started` (no session yet: its start is `created_at`).
    pub fn agent(started: i64) -> ProjectAgent {
        ProjectAgent {
            id: "a1".into(),
            project: "SHOP".into(),
            name: "Flow setup".into(),
            name_source: Some(NameSource::Agent),
            state: AgentState::Running,
            tool: CLAUDE,
            tool_session_id: Some("c1".into()),
            model: Some("sonnet".into()),
            got_request: true,
            mid_turn: true,
            limit_stopped: false,
            created_at: started,
            ended_at: None,
            closed: false,
            question: None,
            needs_you: None,
            focused: false,
            tokens: Default::default(),
            session: None,
            epic: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use otaq_core::agent_tools::connected::AgentToolState;

    use super::fixtures::{NOW, agent};
    use super::*;
    use crate::view_models::tool_problems::fixtures::{
        CLAUDE, CODEX, COPILOT, at_limit, several, status,
    };

    fn at(mut tools: Tools, now: i64) -> Tools {
        tools.now = now;
        tools
    }

    /// Every word a card shows, in order, for the "no per-source rows" check.
    fn words(v: &View) -> Vec<String> {
        [
            Some(v.title.clone()),
            Some(v.text.clone()),
            v.elapsed.clone(),
            v.problem.clone(),
            v.stop.clone(),
            v.skip.clone(),
            v.use_preset.clone(),
            v.retry.as_ref().map(|b| b.label.clone()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    #[test]
    fn working_reads_how_you_work_with_the_elapsed_time_and_no_sources_s14_14() {
        let v = view(&agent(NOW - 2 * 60_000 - 5_000), &at(several(), NOW));
        assert_eq!(v.kind, Kind::Working);
        assert_eq!(
            words(&v),
            [
                "Reading how you work",
                "This can take a few minutes. Watch the agent in its terminal, or come back later: the proposal \
                 waits for you.",
                "Working · 2 min",
                "Stop and use a preset",
            ]
        );
        assert_eq!(
            (&v.pill, &v.foot),
            (&None, &None),
            "the pane's header and foot"
        );
        // No per-source rows (d0108 changelog 2026-10-21): nothing names history, the repo or GitHub.
        for word in words(&v) {
            for source in ["history", "repo", "GitHub", "messages read", "waiting"] {
                assert!(!word.contains(source), "{word:?} names {source}");
            }
        }
    }

    #[test]
    fn the_elapsed_time_counts_from_its_sessions_start_in_whole_minutes() {
        let elapsed = |started: i64, now: i64| view(&agent(started), &at(several(), now)).elapsed;
        assert_eq!(elapsed(NOW, NOW).as_deref(), Some("Working · 0 min"));
        assert_eq!(
            elapsed(NOW - 59_999, NOW).as_deref(),
            Some("Working · 0 min")
        );
        assert_eq!(
            elapsed(NOW - 41 * 60_000, NOW).as_deref(),
            Some("Working · 41 min")
        );
        assert_eq!(
            elapsed(NOW - 125 * 60_000, NOW).as_deref(),
            Some("Working · 2 h 5 min")
        );
        // A resumed agent counts from its newest session, not its first start.
        let mut resumed = agent(NOW - 3 * 3_600_000);
        resumed.session = Some(otaq_core::session::SessionInfo {
            started_at: NOW - 60_000,
            ..session()
        });
        assert_eq!(
            view(&resumed, &at(several(), NOW)).elapsed.as_deref(),
            Some("Working · 1 min")
        );
    }

    fn session() -> otaq_core::session::SessionInfo {
        serde_json::from_value(serde_json::json!({
            "id": "s1", "kind": "project_agent", "project": "SHOP", "state": "running", "started_at": 0
        }))
        .unwrap()
    }

    #[test]
    fn the_plan_limit_stops_it_until_the_reset_s14_14() {
        let mut a = agent(NOW - 10 * 60_000);
        a.limit_stopped = true;
        let tools = at(at_limit(CLAUDE, Some(NOW + 72 * 60_000)), NOW);
        let v = view(&a, &tools);
        assert_eq!(v.kind, Kind::Stopped);
        assert_eq!(v.title, "The agent stopped");
        let reset = reset_label(NOW + 72 * 60_000, NOW);
        assert_eq!(
            v.text,
            format!(
                "Claude Code reached its plan limit before the proposal was ready. Try again at {reset} · it picks \
                 up where it stopped."
            )
        );
        assert_eq!(
            v.problem,
            Some(format!(
                "Claude Code limit reached · resets at {reset} (in 1 h 12 min)."
            ))
        );
        assert_eq!(
            (v.skip.as_deref(), v.use_preset.as_deref()),
            (Some("Skip, use the default"), Some("Use a preset instead"))
        );
        let retry = v.retry.unwrap();
        assert_eq!(retry.label, format!("Try again at {reset}"));
        assert!(!retry.enabled, "off until the limit resets");
        assert_eq!(
            retry.action,
            Action::ResumeTool {
                tool: CLAUDE.as_str().into()
            }
        );
        assert_eq!(v.pill.unwrap().label, "Stopped");
        assert_eq!(v.stop, None);

        // The reset time came: Try again continues it.
        let later = view(&a, &at(tools.clone(), NOW + 73 * 60_000));
        let retry = later.retry.unwrap();
        assert!(retry.enabled);
        assert_eq!(retry.label, "Try again");
        assert_eq!(later.problem, None);
        // Seen reset, or the limit cleared by the banner's Resume: the same.
        let mut seen = tools.clone();
        seen.limits[0].reset_seen_at = Some(NOW);
        assert!(view(&a, &seen).retry.unwrap().enabled);
        assert!(view(&a, &at(several(), NOW)).retry.unwrap().enabled);
        // A reset at an unknown time: Try again is the user's call.
        let unknown = view(&a, &at(at_limit(CLAUDE, None), NOW));
        assert_eq!(
            unknown.problem.as_deref(),
            Some("Claude Code limit reached · resets at an unknown time.")
        );
        assert!(unknown.retry.unwrap().enabled);
    }

    #[test]
    fn a_tool_that_isnt_signed_in_keeps_the_agent_from_starting_s14_14() {
        let v = cannot_start(CODEX);
        assert_eq!(v.kind, Kind::NotSignedIn);
        assert_eq!(
            words(&v),
            [
                "The agent couldn’t start",
                "Codex isn’t signed in on this computer.",
                "Codex: not signed in.",
                "Skip, use the default",
                "Use a preset instead",
                "Sign in…",
            ]
        );
        assert_eq!(
            v.retry.unwrap().action,
            Action::SignIn {
                tool: CODEX.as_str().into()
            }
        );
        // Copilot is named by its short name, as everywhere.
        assert_eq!(
            cannot_start(COPILOT).text,
            "Copilot isn’t signed in on this computer."
        );

        // Found · not signed in only: Claude Code is never (no check, d0086), nor a connected or missing tool.
        let mut tools = several();
        tools.statuses[1] = status(CODEX, AgentToolState::FoundNotSignedIn { version: None });
        assert!(not_signed_in(CODEX, &tools));
        assert!(!not_signed_in(CLAUDE, &tools));
        assert!(!not_signed_in(COPILOT, &tools));
        assert!(!not_signed_in(Tool::ALL[3], &tools), "not installed");
    }

    #[test]
    fn ending_before_a_proposal_offers_to_resume_the_same_conversation() {
        for state in [AgentState::Ended, AgentState::EndedByRestart] {
            let mut a = agent(NOW - 5 * 60_000);
            a.state = state;
            let v = view(&a, &at(several(), NOW));
            assert_eq!(v.kind, Kind::Ended);
            assert_eq!(
                words(&v),
                [
                    "The agent ended without a proposal",
                    "Its conversation ended before it proposed a flow. Try again to pick up where it stopped.",
                    "Skip, use the default",
                    "Use a preset instead",
                    "Try again",
                ]
            );
            let retry = v.retry.unwrap();
            assert!(retry.enabled);
            assert_eq!(retry.action, Action::Resume { agent: "a1".into() });
            assert_eq!(v.foot.as_deref(), Some("Read-only · the session ended"));
            assert_eq!(v.pill, None, "the pane's Ended");
        }
    }

    #[test]
    fn the_gallery_has_the_four_cards() {
        let g = gallery();
        let kinds: Vec<Kind> = g.cells.iter().map(|c| c.view.kind).collect();
        assert_eq!(
            kinds,
            [Kind::Working, Kind::Stopped, Kind::NotSignedIn, Kind::Ended]
        );
        assert_eq!(g.cells[0].view.elapsed.as_deref(), Some("Working · 2 min"));
        assert!(
            g.cells[1]
                .view
                .problem
                .as_deref()
                .unwrap()
                .ends_with("(in 1 h 12 min).")
        );
    }
}
