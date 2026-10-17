//! Claude Code's hooks as Otaq's events (ADP-02, ADP-03, PLG-04; d0009, d0020, d0042, d0046, d0088): the SDD
//! §11.4 table without Otaq's own rule. Whether a turn end is Idle, Working in background or "turn ended after a
//! submit" is decided in `signals/`.
//!
//! Payload fields follow the Claude Code hooks reference (code.claude.com/docs/en/hooks, read 2026-10-10; Claude
//! Code 2.1.296): every event has `session_id`, `cwd`, `hook_event_name`; `Notification` has `message`, `title`
//! and `notification_type`; `Stop` has `last_assistant_message`, `background_tasks` (entries with `id`, `type`,
//! `status`, `description`, and `command`, `agent_type`, `server`/`tool` or `name` by type) and `session_crons`
//! (entries with `id`, `schedule`, `recurring`, `prompt`); `StopFailure` has `error`, `error_details` and
//! `last_assistant_message`; `UserPromptSubmit` has `prompt`. Fields beyond these are read defensively; the service
//! logs every raw payload at debug level, so they can be checked against real sessions (SDD §26).

use serde_json::Value;

use crate::agent_tools::ToolEvent;
use crate::domain::stored_enum;
use crate::signals::background::{BackgroundTask, BackgroundWork, Wakeup};

stored_enum! {
    /// The hooks Otaq's plugin wires to `hook --tool claude <event>` (PLG-04).
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

/// Maps one hook to Otaq's events (SDD §11.4): none (another notification type), or one.
pub fn map_hook(event: &str, payload: &Value) -> Vec<ToolEvent> {
    let Ok(hook) = event.parse::<HookEvent>() else {
        tracing::debug!(event, %payload, "unrecognised Claude Code hook");
        return vec![ToolEvent::Unrecognised {
            event: event.to_owned(),
        }];
    };
    let event = match hook {
        HookEvent::Notification => {
            let kind = text(payload, "notification_type").unwrap_or_default();
            if !NEEDS_YOU_TYPES.contains(&kind.as_str()) {
                return Vec::new();
            }
            ToolEvent::NeedsYou {
                prompt: text(payload, "message")
                    .or_else(|| text(payload, "title"))
                    .unwrap_or_else(|| "Waiting for your answer".to_owned()),
                // Inside a subagent Claude Code adds `agent_id` / `agent_type` to the payload.
                helper_agent: payload.get("agent_id").is_some()
                    || payload.get("agent_type").is_some(),
            }
        }
        HookEvent::Stop => ToolEvent::TurnEnded {
            background: background_work(payload),
            last_message: text(payload, "last_assistant_message"),
        },
        HookEvent::StopFailure => {
            let error = text(payload, "error").unwrap_or_else(|| "unknown".to_owned());
            let message = text(payload, "last_assistant_message");
            let details = text(payload, "error_details");
            // The limit text is the rendered message; some versions may carry it in the details only.
            let hit = [message.as_deref(), details.as_deref()]
                .into_iter()
                .flatten()
                .find_map(|m| super::limits::recognise(&error, m));
            match hit {
                // The reset time comes from the status line's window or the limit text (USG-04).
                Some(hit) => ToolEvent::LimitReached {
                    hit,
                    resets_at: None,
                },
                None => ToolEvent::Failed { error, message },
            }
        }
        HookEvent::UserPromptSubmit => ToolEvent::NewTurn {
            prompt: payload
                .get("prompt")
                .and_then(Value::as_str)
                .map(str::to_owned),
        },
    };
    vec![event]
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
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;
    use crate::signals::{Signal, TurnFacts, signal};
    use crate::usage::limit::{LimitHit, LimitKind};

    /// The hook's events, then Otaq's rule on each (as the service applies them).
    fn map(event: HookEvent, payload: &Value, turn: TurnFacts) -> Signal {
        let events = map_hook(event.as_str(), payload);
        assert!(events.len() <= 1, "{events:?}");
        events
            .first()
            .and_then(|e| signal(e, turn))
            .unwrap_or(Signal::Ignored)
    }

    fn none() -> TurnFacts {
        TurnFacts::default()
    }

    fn fixture(name: &str) -> Value {
        let path: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "tests",
            "fixtures",
            "claude",
            "hooks",
            name,
        ]
        .iter()
        .collect();
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    /// Every saved payload, one per row of SDD §11.4, becomes the expected events.
    #[test]
    fn each_fixture_becomes_its_events() {
        let needs_you = |prompt: &str, helper_agent| {
            vec![ToolEvent::NeedsYou {
                prompt: prompt.into(),
                helper_agent,
            }]
        };
        let limit = |kind, text: &str| {
            vec![ToolEvent::LimitReached {
                hit: LimitHit {
                    kind,
                    text: text.into(),
                },
                resets_at: None,
            }]
        };
        let cases: Vec<(&str, Vec<ToolEvent>)> = vec![
            (
                "Notification-permission_prompt.json",
                needs_you("Claude needs your permission to use Bash", false),
            ),
            (
                "Notification-elicitation_dialog.json",
                needs_you("Claude Code needs your input", false),
            ),
            (
                "Notification-helper_agent.json",
                needs_you("Claude needs your permission to use Bash", true),
            ),
            ("Notification-idle_prompt.json", vec![]),
            ("Notification-auth_success.json", vec![]),
            (
                "Stop-idle.json",
                vec![ToolEvent::TurnEnded {
                    background: BackgroundWork::default(),
                    last_message: Some(
                        "I split the name into first and last.\n\nShould I also update the tests?"
                            .into(),
                    ),
                }],
            ),
            (
                "Stop-background.json",
                vec![ToolEvent::TurnEnded {
                    background: BackgroundWork {
                        tasks: vec![
                            BackgroundTask {
                                kind: "shell".into(),
                                description: Some("test run".into()),
                                helper_agent: false,
                            },
                            BackgroundTask {
                                kind: "subagent".into(),
                                description: Some("explore the parser".into()),
                                helper_agent: true,
                            },
                        ],
                        wakeups: vec![],
                    },
                    last_message: Some("Running the tests in the background.".into()),
                }],
            ),
            (
                "Stop-cron.json",
                vec![ToolEvent::TurnEnded {
                    background: BackgroundWork {
                        tasks: vec![],
                        wakeups: vec![Wakeup {
                            schedule: "10 16 * * *".into(),
                            recurring: false,
                        }],
                    },
                    last_message: Some("I'll check the build at 4:10pm.".into()),
                }],
            ),
            (
                "StopFailure-session_limit.json",
                limit(
                    LimitKind::Session,
                    "You've hit your session limit · resets 3:45pm",
                ),
            ),
            (
                "StopFailure-weekly_limit.json",
                limit(
                    LimitKind::Weekly,
                    "You've hit your weekly limit · resets Mon 12:00am",
                ),
            ),
            (
                "StopFailure-capacity.json",
                vec![ToolEvent::Failed {
                    error: "rate_limit".into(),
                    message: Some("API Error: Rate limit reached".into()),
                }],
            ),
            (
                "StopFailure-other.json",
                vec![ToolEvent::Failed {
                    error: "server_error".into(),
                    message: Some("API Error: 500".into()),
                }],
            ),
            (
                "UserPromptSubmit.json",
                vec![ToolEvent::NewTurn {
                    prompt: Some("yes, update them".into()),
                }],
            ),
        ];
        for (file, expected) in cases {
            let payload = fixture(file);
            let event = payload["hook_event_name"].as_str().unwrap().to_owned();
            assert!(file.starts_with(&event), "{file}");
            assert_eq!(map_hook(&event, &payload), expected, "{file}");
        }
    }

    /// The saved payloads give today's signals through Otaq's rule.
    #[test]
    fn each_fixture_gives_todays_signal() {
        let asked = TurnFacts {
            submitted_or_asked: true,
        };
        let signal_of = |file: &str, turn| {
            let payload = fixture(file);
            map(
                payload["hook_event_name"]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap(),
                &payload,
                turn,
            )
        };
        assert!(matches!(
            signal_of("Notification-permission_prompt.json", none()),
            Signal::NeedsYou { .. }
        ));
        assert_eq!(
            signal_of("Notification-helper_agent.json", none()),
            Signal::NeedsYou {
                prompt: "helper agent asks: Claude needs your permission to use Bash".into()
            }
        );
        assert_eq!(
            signal_of("Notification-idle_prompt.json", none()),
            Signal::Ignored
        );
        assert_eq!(
            signal_of("Stop-idle.json", none()),
            Signal::Idle {
                last_line: Some("Should I also update the tests?".into())
            }
        );
        assert_eq!(signal_of("Stop-idle.json", asked), Signal::TurnEnded);
        for file in ["Stop-background.json", "Stop-cron.json"] {
            assert!(
                matches!(signal_of(file, asked), Signal::WorkingInBackground { .. }),
                "{file}"
            );
        }
        for file in [
            "StopFailure-session_limit.json",
            "StopFailure-weekly_limit.json",
        ] {
            assert!(
                matches!(signal_of(file, none()), Signal::LimitReached { .. }),
                "{file}"
            );
        }
        for file in ["StopFailure-capacity.json", "StopFailure-other.json"] {
            assert!(
                matches!(signal_of(file, none()), Signal::Failed { .. }),
                "{file}"
            );
        }
        assert!(matches!(
            signal_of("UserPromptSubmit.json", asked),
            Signal::Working { .. }
        ));
    }

    #[test]
    fn request_notifications_are_needs_you_and_others_are_ignored() {
        for kind in NEEDS_YOU_TYPES {
            let payload = json!({"hook_event_name": "Notification", "notification_type": kind,
                "message": "Claude needs your permission to use Bash", "title": "Permission needed"});
            assert_eq!(
                map(HookEvent::Notification, &payload, none()),
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
                map(HookEvent::Notification, &payload, none()),
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
            map(HookEvent::Notification, &payload, none()),
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
        let Signal::WorkingInBackground { work } = map(HookEvent::Stop, &payload, none()) else {
            panic!()
        };
        assert_eq!(work.tasks.len(), 2);
        assert!(work.tasks[1].helper_agent);

        let crons = json!({"background_tasks": [], "session_crons": [
            {"id": "cron-001", "schedule": "10 16 * * *", "recurring": false, "prompt": "check the build"}]});
        assert!(matches!(
            map(HookEvent::Stop, &crons, none()),
            Signal::WorkingInBackground { .. }
        ));
    }

    #[test]
    fn stop_with_nothing_pending_is_idle_unless_submitted_or_asked() {
        let payload = json!({"hook_event_name": "Stop", "stop_hook_active": false,
            "last_assistant_message": "I split the name.\n\nShould I also update the tests?\n",
            "background_tasks": [], "session_crons": []});
        assert_eq!(
            map(HookEvent::Stop, &payload, none()),
            Signal::Idle {
                last_line: Some("Should I also update the tests?".into())
            }
        );
        let asked = TurnFacts {
            submitted_or_asked: true,
        };
        assert_eq!(map(HookEvent::Stop, &payload, asked), Signal::TurnEnded);
        // Finished tasks don't count; missing lists are empty.
        let done =
            json!({"background_tasks": [{"id": "t", "type": "shell", "status": "completed"}]});
        assert_eq!(
            map(HookEvent::Stop, &done, none()),
            Signal::Idle { last_line: None }
        );
    }

    #[test]
    fn stop_failure_is_the_limit_or_a_logged_failure_and_prompts_start_a_turn() {
        let payload = json!({"hook_event_name": "StopFailure", "error": "rate_limit",
            "error_details": "429 Too Many Requests",
            "last_assistant_message": "You've hit your session limit · resets 3:45pm"});
        assert_eq!(
            map(HookEvent::StopFailure, &payload, none()),
            Signal::LimitReached {
                hit: LimitHit {
                    kind: LimitKind::Session,
                    text: "You've hit your session limit · resets 3:45pm".into()
                }
            }
        );
        let capacity = json!({"hook_event_name": "StopFailure", "error": "rate_limit",
            "error_details": "429 Too Many Requests",
            "last_assistant_message": "API Error: Rate limit reached"});
        assert_eq!(
            map(HookEvent::StopFailure, &capacity, none()),
            Signal::Failed {
                error: "rate_limit".into(),
                message: Some("API Error: Rate limit reached".into())
            }
        );
        let prompt = json!({"hook_event_name": "UserPromptSubmit", "prompt": "yes, update them"});
        assert_eq!(
            map(HookEvent::UserPromptSubmit, &prompt, none()),
            Signal::Working {
                prompt: Some("yes, update them".into())
            }
        );
    }

    #[test]
    fn an_unknown_hook_is_unrecognised() {
        assert_eq!(
            map_hook("SessionStart", &json!({"hook_event_name": "SessionStart"})),
            [ToolEvent::Unrecognised {
                event: "SessionStart".into()
            }]
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
