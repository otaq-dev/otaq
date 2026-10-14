//! The hook-to-status mapping of SDD §11.4 (PLG-04, ATT-01, ATT-04; d0009, d0020, d0042, d0046), as a pure
//! function from (event, payload, turn facts) to a [`Signal`].
//!
//! Payload fields follow the Claude Code hooks reference (code.claude.com/docs/en/hooks, read 2026-10-10; Claude
//! Code 2.1.296): every event has `session_id`, `cwd`, `hook_event_name`; `Notification` has `message`, `title`
//! and `notification_type`; `Stop` has `last_assistant_message`, `background_tasks` (entries with `id`, `type`,
//! `status`, `description`, and `command`, `agent_type`, `server`/`tool` or `name` by type) and `session_crons`
//! (entries with `id`, `schedule`, `recurring`, `prompt`); `StopFailure` has `error`, `error_details` and
//! `last_assistant_message`; `UserPromptSubmit` has `prompt`. Fields beyond these are read defensively and the
//! raw payload is logged at debug level, so they can be checked against real sessions (SDD §26).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::background::{BackgroundTask, BackgroundWork, Wakeup};
use crate::domain::stored_enum;

stored_enum! {
    /// The hooks the plugin wires to `otaq hook <event>` (PLG-04).
    pub enum HookEvent ("hook event") {
        Notification = "Notification",
        Stop = "Stop",
        StopFailure = "StopFailure",
        UserPromptSubmit = "UserPromptSubmit",
    }
}

/// `Notification` types that mean the agent waits for the user (d0042). `idle_prompt`, `auth_success` and the
/// others are ignored.
pub const NEEDS_YOU_TYPES: &[&str] = &[
    "permission_prompt",
    "elicitation_dialog",
    "elicitation_url_dialog",
    "agent_needs_input",
];

/// Starts the stored prompt of a request raised inside a helper agent (the card shows a bot icon for it).
pub const HELPER_PROMPT_PREFIX: &str = "helper agent asks: ";

/// What happened in the session's current turn, as far as the Stop mapping cares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnFacts {
    /// `otaq review submit` or `otaq ask human` ran in this turn: its Stop is not Idle.
    pub submitted_or_asked: bool,
}

/// What a hook means for the ticket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "signal", rename_all = "snake_case")]
pub enum Signal {
    /// A permission prompt or another request for the user, also from inside a helper agent.
    NeedsYou { prompt: String },
    /// The turn ended with background tasks or wakeups pending (neutral).
    WorkingInBackground { work: BackgroundWork },
    /// The turn ended with nothing pending and no submit or ask: the agent waits (turquoise).
    Idle {
        /// The last line the agent wrote, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_line: Option<String>,
    },
    /// The turn ended after a submit or an ask: the status those set stays.
    TurnEnded,
    /// A new turn started.
    Working,
    /// The turn ended on the plan's usage limit (`StopFailure` with `rate_limit` and the limit text, USG-03): the
    /// agent shows Waiting for limit reset and the limit becomes active (d0020, d0046).
    LimitReached { hit: crate::usage::limit::LimitHit },
    /// The turn failed for another reason (`StopFailure`): logged; the status stays.
    Failed {
        error: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    /// Nothing to show (another notification type).
    Ignored,
}

impl Signal {
    /// Whether the agent's turn has ended with this signal: it is idle between turns, so automatic deliveries
    /// may be typed (SES-06).
    pub fn ends_turn(&self) -> bool {
        matches!(
            self,
            Self::WorkingInBackground { .. }
                | Self::Idle { .. }
                | Self::TurnEnded
                | Self::LimitReached { .. }
        )
    }
}

/// Maps one hook to its signal (SDD §11.4).
pub fn map_hook(event: HookEvent, payload: &Value, turn: TurnFacts) -> Signal {
    match event {
        HookEvent::Notification => {
            let kind = text(payload, "notification_type").unwrap_or_default();
            if NEEDS_YOU_TYPES.contains(&kind.as_str()) {
                let prompt = text(payload, "message")
                    .or_else(|| text(payload, "title"))
                    .unwrap_or_else(|| "Waiting for your answer".to_owned());
                // Inside a subagent Claude Code adds `agent_id` / `agent_type` to the payload.
                let helper =
                    payload.get("agent_id").is_some() || payload.get("agent_type").is_some();
                Signal::NeedsYou {
                    prompt: if helper {
                        format!("{HELPER_PROMPT_PREFIX}{prompt}")
                    } else {
                        prompt
                    },
                }
            } else {
                Signal::Ignored
            }
        }
        HookEvent::Stop => {
            let work = background_work(payload);
            if !work.is_empty() {
                Signal::WorkingInBackground { work }
            } else if turn.submitted_or_asked {
                Signal::TurnEnded
            } else {
                Signal::Idle {
                    last_line: text(payload, "last_assistant_message")
                        .as_deref()
                        .and_then(last_line),
                }
            }
        }
        HookEvent::StopFailure => {
            let error = text(payload, "error").unwrap_or_else(|| "unknown".to_owned());
            let message = text(payload, "last_assistant_message");
            let details = text(payload, "error_details");
            // The limit text is the rendered message; some versions may carry it in the details only.
            let hit = [message.as_deref(), details.as_deref()]
                .into_iter()
                .flatten()
                .find_map(|m| crate::usage::limit::recognise(&error, m));
            match hit {
                Some(hit) => Signal::LimitReached { hit },
                None => Signal::Failed { error, message },
            }
        }
        HookEvent::UserPromptSubmit => Signal::Working,
    }
}

/// A non-empty string field.
fn text(payload: &Value, field: &str) -> Option<String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// The last non-empty line of a message, without Markdown list or heading marks.
pub fn last_line(message: &str) -> Option<String> {
    message
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .map(|l| l.trim_start_matches(['#', '-', '*', '>', ' ']).to_owned())
        .filter(|l| !l.is_empty())
}

/// The `background_tasks` and `session_crons` of a `Stop` payload. Tasks that are no longer running are left
/// out (a `status` other than running or pending).
fn background_work(payload: &Value) -> BackgroundWork {
    let list = |field: &str| {
        payload
            .get(field)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let tasks = list("background_tasks")
        .iter()
        .filter(|t| {
            t.get("status")
                .and_then(Value::as_str)
                .is_none_or(|s| matches!(s, "running" | "pending" | "in_progress" | "started"))
        })
        .map(|t| BackgroundTask {
            kind: text(t, "type").unwrap_or_default(),
            description: text(t, "description")
                .or_else(|| text(t, "name"))
                .or_else(|| text(t, "command"))
                .or_else(|| text(t, "tool")),
            helper_agent: t.get("agent_type").is_some()
                || text(t, "type").is_some_and(|k| k.contains("agent")),
        })
        .collect();
    let wakeups = list("session_crons")
        .iter()
        .filter_map(|c| {
            Some(Wakeup {
                schedule: text(c, "schedule")?,
                recurring: c.get("recurring").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect();
    BackgroundWork { tasks, wakeups }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn none() -> TurnFacts {
        TurnFacts::default()
    }

    #[test]
    fn request_notifications_are_needs_you_and_others_are_ignored() {
        for kind in NEEDS_YOU_TYPES {
            let payload = json!({"hook_event_name": "Notification", "notification_type": kind,
                "message": "Claude needs your permission to use Bash", "title": "Permission needed"});
            assert_eq!(
                map_hook(HookEvent::Notification, &payload, none()),
                Signal::NeedsYou {
                    prompt: "Claude needs your permission to use Bash".into()
                },
                "{kind}"
            );
        }
        for kind in [
            "idle_prompt",
            "auth_success",
            "elicitation_complete",
            "agent_completed",
        ] {
            let payload = json!({"notification_type": kind, "message": "x"});
            assert_eq!(
                map_hook(HookEvent::Notification, &payload, none()),
                Signal::Ignored,
                "{kind}"
            );
        }
    }

    #[test]
    fn a_prompt_inside_a_helper_agent_is_needs_you() {
        let payload = json!({"notification_type": "permission_prompt", "agent_id": "a1",
            "agent_type": "general-purpose", "message": "Claude needs your permission to use Bash"});
        assert_eq!(
            map_hook(HookEvent::Notification, &payload, none()),
            Signal::NeedsYou {
                prompt: "helper agent asks: Claude needs your permission to use Bash".into()
            }
        );
    }

    #[test]
    fn stop_with_background_work_is_working_in_background() {
        let payload = json!({"hook_event_name": "Stop", "last_assistant_message": "Waiting for the tests.",
            "background_tasks": [
                {"id": "task-001", "type": "shell", "status": "running", "description": "test run",
                 "command": "cargo test"},
                {"id": "task-002", "type": "subagent", "status": "running", "description": "explore",
                 "agent_type": "Explore"}
            ],
            "session_crons": []});
        let Signal::WorkingInBackground { work } = map_hook(HookEvent::Stop, &payload, none())
        else {
            panic!()
        };
        assert_eq!(work.tasks.len(), 2);
        assert!(work.tasks[1].helper_agent);

        let crons = json!({"background_tasks": [], "session_crons": [
            {"id": "cron-001", "schedule": "10 16 * * *", "recurring": false, "prompt": "check the build"}]});
        assert!(matches!(
            map_hook(HookEvent::Stop, &crons, none()),
            Signal::WorkingInBackground { .. }
        ));
    }

    #[test]
    fn stop_with_nothing_pending_is_idle_unless_submitted_or_asked() {
        let payload = json!({"hook_event_name": "Stop", "stop_hook_active": false,
            "last_assistant_message": "I split the name.\n\nShould I also update the tests?\n",
            "background_tasks": [], "session_crons": []});
        assert_eq!(
            map_hook(HookEvent::Stop, &payload, none()),
            Signal::Idle {
                last_line: Some("Should I also update the tests?".into())
            }
        );
        let asked = TurnFacts {
            submitted_or_asked: true,
        };
        assert_eq!(
            map_hook(HookEvent::Stop, &payload, asked),
            Signal::TurnEnded
        );
        // Finished tasks don't count; missing lists are empty.
        let done =
            json!({"background_tasks": [{"id": "t", "type": "shell", "status": "completed"}]});
        assert_eq!(
            map_hook(HookEvent::Stop, &done, none()),
            Signal::Idle { last_line: None }
        );
    }

    #[test]
    fn stop_failure_is_the_limit_or_a_logged_failure_and_prompts_start_a_turn() {
        let payload = json!({"hook_event_name": "StopFailure", "error": "rate_limit",
            "error_details": "429 Too Many Requests",
            "last_assistant_message": "You've hit your session limit · resets 3:45pm"});
        assert_eq!(
            map_hook(HookEvent::StopFailure, &payload, none()),
            Signal::LimitReached {
                hit: crate::usage::limit::LimitHit {
                    kind: crate::usage::limit::LimitKind::Session,
                    text: "You've hit your session limit · resets 3:45pm".into()
                }
            }
        );
        let capacity = json!({"hook_event_name": "StopFailure", "error": "rate_limit",
            "error_details": "429 Too Many Requests",
            "last_assistant_message": "API Error: Rate limit reached"});
        assert_eq!(
            map_hook(HookEvent::StopFailure, &capacity, none()),
            Signal::Failed {
                error: "rate_limit".into(),
                message: Some("API Error: Rate limit reached".into())
            }
        );
        let prompt = json!({"hook_event_name": "UserPromptSubmit", "prompt": "yes, update them"});
        assert_eq!(
            map_hook(HookEvent::UserPromptSubmit, &prompt, none()),
            Signal::Working
        );
    }

    #[test]
    fn which_signals_end_the_turn() {
        assert!(Signal::Idle { last_line: None }.ends_turn());
        assert!(Signal::TurnEnded.ends_turn());
        assert!(!Signal::Working.ends_turn());
        assert!(
            !Signal::NeedsYou {
                prompt: String::new()
            }
            .ends_turn()
        );
    }

    #[test]
    fn hook_event_names_are_claude_codes() {
        let names: Vec<_> = HookEvent::ALL.iter().map(|e| e.as_str()).collect();
        assert_eq!(
            names,
            ["Notification", "Stop", "StopFailure", "UserPromptSubmit"]
        );
    }
}
