//! Claude Code's hooks as Otaq's events (ADP-02, ADP-03, PLG-04; d0009, d0020, d0042, d0046, d0088): the SDD
//! §11.4 table without Otaq's own rule. Whether a turn end is Idle, Working in background or "turn ended after a
//! submit" is decided in `signals/`.
//!
//! Payload fields follow the Claude Code hooks reference (code.claude.com/docs/en/hooks, read 2026-10-10; Claude
//! Code 2.1.296): every event has `session_id`, `cwd`, `hook_event_name`; `Notification` has `message`, `title`
//! and `notification_type`; `Stop` has `last_assistant_message`, `background_tasks` (entries with `id`, `type`,
//! `status`, `description`, and `command`, `agent_type`, `server`/`tool` or `name` by type) and `session_crons`
//! (entries with `id`, `schedule`, `recurring`, `prompt`); `StopFailure` has `error`, `error_details` and
//! `last_assistant_message`; `UserPromptSubmit` has `prompt`; `SessionStart` has `source` and `model` (checked in the
//! SDK schema of the installed Claude Code 2.1.302). Fields beyond these are read defensively; the service logs every
//! raw payload at debug level, so they can be checked against real sessions (SDD §26).
//!
//! The model that ran (AGT-04) comes from `SessionStart`'s `model` and, as a second source, from the status line's
//! `model.id`: `otaq statusline` sends the part of its JSON that matters here ([`status_report`]) as the event
//! [`STATUS_EVENT`], which no hook of the plugin uses. A model Claude Code can't run (AGT-05) is a `StopFailure`
//! whose `error` is `model_not_found` (the error kinds in the same schema), or, failing that, whose message starts
//! [`MODEL_ISSUE`]; the payload is unverified against a real one (SDD §11.4, §26), so every `StopFailure` is
//! logged raw at debug level.

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
        SessionStart = "SessionStart",
    }
}

/// The event `otaq statusline` sends the status line's model as (Claude Code's own `hook_event_name` for the
/// status-line JSON). Not a hook: the plugin doesn't wire it.
pub const STATUS_EVENT: &str = "Status";

/// `StopFailure`'s error kind for a model Claude Code can't use.
pub const MODEL_NOT_FOUND: &str = "model_not_found";

/// How Claude Code's message for a model it can't use starts ("There's an issue with the selected model
/// (claude-fable-5-1). It may not exist or you may not have access to it.").
pub const MODEL_ISSUE: &str = "There's an issue with the selected model";

/// What of a status-line JSON goes to the service: its event name and `model.id`, when it has one. The rest
/// (the windows go as `usage.record_limits`) stays out of the hook log.
pub fn status_report(payload: &Value) -> Option<Value> {
    let model = text(payload.get("model")?, "id")?;
    Some(serde_json::json!({"hook_event_name": STATUS_EVENT, "model": {"id": model}}))
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
    if event == STATUS_EVENT {
        return payload
            .get("model")
            .and_then(|m| text(m, "id"))
            .map(|model| ToolEvent::ModelKnown { model })
            .into_iter()
            .collect();
    }
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
            // Raw, so a real payload can be captured from the log (the model error's is unverified, SDD §26).
            tracing::debug!(%payload, "Claude Code StopFailure");
            let error = text(payload, "error").unwrap_or_else(|| "unknown".to_owned());
            let message = text(payload, "last_assistant_message");
            let details = text(payload, "error_details");
            if let Some(refused) = model_refused(&error, message.as_deref(), details.as_deref()) {
                return vec![refused];
            }
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
        HookEvent::SessionStart => match text(payload, "model") {
            Some(model) => ToolEvent::ModelKnown { model },
            // Not a shape Otaq knows: nothing changes (ADP-11).
            None => {
                tracing::debug!(%payload, "Claude Code SessionStart without a model");
                ToolEvent::Unrecognised {
                    event: event.to_owned(),
                }
            }
        },
    };
    vec![event]
}

/// A `StopFailure` on the model: the error kind `model_not_found`, or a message that starts [`MODEL_ISSUE`]. The
/// message shown is the rendered one, else the details.
fn model_refused(error: &str, message: Option<&str>, details: Option<&str>) -> Option<ToolEvent> {
    let texts = [message, details];
    let says_so = texts.iter().flatten().any(|m| m.starts_with(MODEL_ISSUE));
    if error != MODEL_NOT_FOUND && !says_so {
        return None;
    }
    let message = message.or(details).unwrap_or(MODEL_ISSUE).to_owned();
    Some(ToolEvent::ModelRefused {
        model: texts.iter().flatten().find_map(|m| named_model(m)),
        message,
    })
}

/// The model a model error names: "…the selected model (claude-fable-5-1)…", "The model claude-fable-5-1 is not
/// available…", "API Error (claude-fable-5-1): …".
fn named_model(message: &str) -> Option<String> {
    let between = |start: &str, end: &str| {
        let from = message.find(start)? + start.len();
        let len = message[from..].find(end)?;
        Some(message[from..from + len].trim().to_owned())
    };
    between(&format!("{MODEL_ISSUE} ("), ")")
        .or_else(|| between("The model ", " is not available"))
        .or_else(|| {
            let found = between("(", "):")?;
            (!found.contains(char::is_whitespace)).then_some(found)
        })
        .filter(|m| !m.is_empty())
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
            (
                "SessionStart-startup.json",
                vec![ToolEvent::ModelKnown {
                    model: "claude-opus-5-5".into(),
                }],
            ),
            (
                "StopFailure-model_not_found.json",
                vec![ToolEvent::ModelRefused {
                    model: Some("claude-fable-5-1".into()),
                    message: "There's an issue with the selected model (claude-fable-5-1). It may not exist or \
                              you may not have access to it. Run /model to pick a different model."
                        .into(),
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
        // The model events are the service's own, not a status (T-0336, T-0337).
        for file in [
            "SessionStart-startup.json",
            "StopFailure-model_not_found.json",
        ] {
            assert_eq!(signal_of(file, none()), Signal::Ignored, "{file}");
        }
    }

    /// The status line's `model.id` is the second source of the model that ran.
    #[test]
    fn the_status_line_model_is_model_known() {
        let path: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "tests",
            "fixtures",
            "claude",
            "statusline",
            "with_rate_limits.json",
        ]
        .iter()
        .collect();
        let payload: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let report = status_report(&payload).unwrap();
        assert_eq!(
            report,
            json!({"hook_event_name": "Status", "model": {"id": "claude-opus-5-5"}})
        );
        assert_eq!(
            map_hook(STATUS_EVENT, &report),
            [ToolEvent::ModelKnown {
                model: "claude-opus-5-5".into()
            }]
        );
        assert_eq!(
            status_report(&json!({"model": {"display_name": "Opus"}})),
            None
        );
        assert_eq!(map_hook(STATUS_EVENT, &json!({})), []);
    }

    /// A model error is recognised by its kind, else by its message; a limit and other failures stay what they
    /// were.
    #[test]
    fn a_model_error_is_model_refused_and_other_failures_are_not() {
        let issue = "There's an issue with the selected model (claude-fable-0-0). It may not exist or you may \
                     not have access to it.";
        // The message alone, under another kind.
        assert_eq!(
            map_hook(
                "StopFailure",
                &json!({"error": "invalid_request", "last_assistant_message": issue})
            ),
            [ToolEvent::ModelRefused {
                model: Some("claude-fable-0-0".into()),
                message: issue.into()
            }]
        );
        // The kind alone, with the deployment's wording.
        let deployment = "The model claude-fable-5-1 is not available on your Bedrock deployment. Try switching \
                          to claude-opus-5-5, or ask your admin to enable this model.";
        assert_eq!(
            map_hook(
                "StopFailure",
                &json!({"error": "model_not_found", "last_assistant_message": deployment})
            ),
            [ToolEvent::ModelRefused {
                model: Some("claude-fable-5-1".into()),
                message: deployment.into()
            }]
        );
        // No message at all: Claude Code's sentence, no model.
        assert_eq!(
            map_hook("StopFailure", &json!({"error": "model_not_found"})),
            [ToolEvent::ModelRefused {
                model: None,
                message: MODEL_ISSUE.into()
            }]
        );
        assert!(matches!(
            map_hook("StopFailure", &fixture("StopFailure-session_limit.json"))[..],
            [ToolEvent::LimitReached { .. }]
        ));
        assert!(matches!(
            map_hook("StopFailure", &fixture("StopFailure-other.json"))[..],
            [ToolEvent::Failed { .. }]
        ));
    }

    #[test]
    fn session_start_without_a_model_is_unrecognised() {
        assert_eq!(
            map_hook(
                "SessionStart",
                &json!({"hook_event_name": "SessionStart", "source": "startup"})
            ),
            [ToolEvent::Unrecognised {
                event: "SessionStart".into()
            }]
        );
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
                },
                resets_at: None
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
            map_hook("PreToolUse", &json!({"hook_event_name": "PreToolUse"})),
            [ToolEvent::Unrecognised {
                event: "PreToolUse".into()
            }]
        );
    }

    #[test]
    fn hook_event_names_are_claude_codes() {
        let names: Vec<_> = HookEvent::ALL.iter().map(|e| e.as_str()).collect();
        assert_eq!(
            names,
            [
                "Notification",
                "Stop",
                "StopFailure",
                "UserPromptSubmit",
                "SessionStart"
            ]
        );
    }
}
