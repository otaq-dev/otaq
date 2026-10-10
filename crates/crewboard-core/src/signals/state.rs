//! The orange family, its clearing rules and the last signal (ATT-01, ATT-02, ATT-04, ATT-05; d0042, d0027), as
//! a pure state machine.
//!
//! - **Asks you** (`crewboard ask human`), **Needs you** (a permission or elicitation prompt) and **Idle** (the turn
//!   ended with nothing pending) keep their label and text: the question, the prompt, the agent's last line.
//! - They **clear** on the user's input: Enter in the ticket's terminal, an answer from the inbox (M7), or the agent
//!   running any `crewboard` command; the ticket is Working again.
//! - **Working in background** is neutral and counts as working; any `crewboard` command or a new turn ends it.
//! - Every hook and every command updates the **last signal**; it is **stale** after [`STALE_AFTER_MS`].
//!
//! Only tickets whose agent is at work ([`super::LIVE`]) change status; the last signal moves for any ticket.

use super::LIVE;
use super::hooks::Signal;
use crate::domain::Status;

/// A working card's last signal turns into a warning after this long (brief 04: "active 4 min ago" is fine,
/// 5 min isn't). One constant, in case 5 min proves wrong (SDD §26).
pub const STALE_AFTER_MS: i64 = 5 * 60 * 1000;

/// A ticket's signal state, as stored on the ticket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalState {
    pub status: Status,
    /// The question, prompt, last line or background summary.
    pub text: Option<String>,
    /// When the current status began.
    pub since: Option<i64>,
    pub wakeup_at: Option<i64>,
    pub last_signal_at: Option<i64>,
}

impl Default for SignalState {
    fn default() -> Self {
        Self {
            status: Status::Backlog,
            text: None,
            since: None,
            wakeup_at: None,
            last_signal_at: None,
        }
    }
}

/// What can change the state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// A hook of the ticket's agent, mapped.
    Hook(Signal),
    /// `crewboard ask human` with this question.
    AskHuman(String),
    /// Any other `crewboard` command run by the agent.
    Command,
    /// Enter pressed in the ticket's terminal (the relay, T-0051).
    Enter,
    /// The user answered from the inbox, the card or the panel (ATT-02, ATT-08): clears like Enter.
    Answered,
}

/// The state after `input` at `now` (Unix milliseconds).
pub fn transition(state: &SignalState, input: &Input, now: i64) -> SignalState {
    let mut next = state.clone();
    if !matches!(input, Input::Enter | Input::Answered) {
        next.last_signal_at = Some(now);
    }
    if !LIVE.contains(&state.status) {
        return next;
    }
    let set =
        |next: &mut SignalState, status: Status, text: Option<String>, wakeup: Option<i64>| {
            if next.status != status || next.text != text {
                next.since = Some(now);
            }
            next.status = status;
            next.text = text;
            next.wakeup_at = wakeup;
        };
    let working = |next: &mut SignalState| {
        if next.status != Status::Working {
            set(next, Status::Working, None, None);
        }
    };
    match input {
        Input::Hook(Signal::NeedsYou { prompt }) => {
            set(&mut next, Status::NeedsYou, Some(prompt.clone()), None);
        }
        Input::Hook(Signal::WorkingInBackground { work }) => {
            let since = (state.status == Status::WorkingInBackground)
                .then_some(state.since)
                .flatten();
            set(
                &mut next,
                Status::WorkingInBackground,
                work.tasks_text(),
                work.next_wakeup(now),
            );
            // Still the same background stretch: keep counting from its start.
            if let Some(since) = since {
                next.since = Some(since);
            }
        }
        Input::Hook(Signal::Idle { last_line }) => {
            set(&mut next, Status::Idle, last_line.clone(), None);
        }
        Input::Hook(Signal::Working) => working(&mut next),
        Input::Hook(Signal::LimitReached { hit }) => {
            set(
                &mut next,
                Status::WaitingForLimitReset,
                Some(hit.text.clone()),
                None,
            );
        }
        Input::Hook(Signal::TurnEnded | Signal::Failed { .. } | Signal::Ignored) => {}
        Input::AskHuman(question) => {
            set(&mut next, Status::AsksYou, Some(question.clone()), None);
        }
        Input::Command => {
            if state.status.is_orange()
                || matches!(
                    state.status,
                    Status::WorkingInBackground | Status::WaitingForLimitReset
                )
            {
                working(&mut next);
            }
        }
        Input::Enter | Input::Answered => {
            if state.status.is_orange() {
                working(&mut next);
            }
        }
    }
    next
}

/// Whether a card's last signal is stale at `now`: a working agent silent for more than [`STALE_AFTER_MS`]. Working
/// in background has live background activity and is never stale; the orange family waits for the user.
pub fn is_stale(status: Status, last_signal_at: Option<i64>, now: i64) -> bool {
    status == Status::Working && last_signal_at.is_some_and(|at| now - at > STALE_AFTER_MS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signals::background::{BackgroundTask, BackgroundWork};

    fn working() -> SignalState {
        SignalState {
            status: Status::Working,
            since: Some(0),
            last_signal_at: Some(0),
            ..Default::default()
        }
    }

    fn with(status: Status, text: &str) -> SignalState {
        SignalState {
            status,
            text: Some(text.into()),
            since: Some(10),
            last_signal_at: Some(10),
            wakeup_at: None,
        }
    }

    fn background() -> Signal {
        Signal::WorkingInBackground {
            work: BackgroundWork {
                tasks: vec![BackgroundTask {
                    kind: "shell".into(),
                    description: Some("test run".into()),
                    helper_agent: false,
                }],
                wakeups: vec![],
            },
        }
    }

    #[test]
    fn hooks_set_each_status_with_its_text() {
        let s = transition(
            &working(),
            &Input::Hook(Signal::NeedsYou {
                prompt: "Allow `rm -rf .next`?".into(),
            }),
            100,
        );
        assert_eq!(s.status, Status::NeedsYou);
        assert_eq!(s.text.as_deref(), Some("Allow `rm -rf .next`?"));
        assert_eq!((s.since, s.last_signal_at), (Some(100), Some(100)));

        let s = transition(&working(), &Input::Hook(background()), 100);
        assert_eq!(s.status, Status::WorkingInBackground);
        assert_eq!(s.text.as_deref(), Some("background: test run"));
        // A later Stop of the same background stretch keeps its start.
        let later = transition(&s, &Input::Hook(background()), 400);
        assert_eq!((later.since, later.last_signal_at), (Some(100), Some(400)));

        let s = transition(
            &working(),
            &Input::Hook(Signal::Idle {
                last_line: Some("Should I also update the tests?".into()),
            }),
            100,
        );
        assert_eq!(s.status, Status::Idle);
        assert_eq!(s.text.as_deref(), Some("Should I also update the tests?"));

        let s = transition(&with(Status::Idle, "x"), &Input::Hook(Signal::Working), 200);
        assert_eq!(
            (s.status, s.text, s.since),
            (Status::Working, None, Some(200))
        );

        for quiet in [
            Signal::TurnEnded,
            Signal::Ignored,
            Signal::Failed {
                error: "rate_limit".into(),
                message: None,
            },
        ] {
            let before = with(Status::AsksYou, "Which email provider?");
            let s = transition(&before, &Input::Hook(quiet), 300);
            assert_eq!((s.status, s.text.clone()), (before.status, before.text));
            assert_eq!(s.last_signal_at, Some(300), "every hook is a signal");
        }
    }

    #[test]
    fn ask_human_stores_the_question() {
        let s = transition(
            &working(),
            &Input::AskHuman("Which email provider should I use?".into()),
            50,
        );
        assert_eq!(s.status, Status::AsksYou);
        assert_eq!(
            s.text.as_deref(),
            Some("Which email provider should I use?")
        );
    }

    #[test]
    fn enter_clears_the_orange_family_only() {
        for status in Status::ORANGE {
            let s = transition(&with(*status, "q"), &Input::Enter, 500);
            assert_eq!((s.status, s.text), (Status::Working, None), "{status}");
            assert_eq!(
                s.last_signal_at,
                Some(10),
                "Enter is the user, not the agent"
            );
        }
        let bg = transition(&working(), &Input::Hook(background()), 100);
        assert_eq!(
            transition(&bg, &Input::Enter, 200).status,
            Status::WorkingInBackground
        );
    }

    #[test]
    fn an_answer_clears_like_enter() {
        for status in Status::ORANGE {
            let s = transition(&with(*status, "q"), &Input::Answered, 500);
            assert_eq!((s.status, s.text), (Status::Working, None), "{status}");
            assert_eq!(s.last_signal_at, Some(10));
        }
    }

    #[test]
    fn any_command_clears_orange_and_background() {
        for status in Status::ORANGE {
            let s = transition(&with(*status, "q"), &Input::Command, 500);
            assert_eq!((s.status, s.last_signal_at), (Status::Working, Some(500)));
        }
        let bg = transition(&working(), &Input::Hook(background()), 100);
        assert_eq!(
            transition(&bg, &Input::Command, 200).status,
            Status::Working
        );
        let s = transition(&working(), &Input::Command, 700);
        assert_eq!(
            (s.status, s.since, s.last_signal_at),
            (Status::Working, Some(0), Some(700))
        );
    }

    #[test]
    fn tickets_that_are_not_at_work_only_record_the_signal() {
        for status in [
            Status::ReadyForReview,
            Status::Queued,
            Status::Parked,
            Status::Merged,
        ] {
            let before = SignalState {
                status,
                ..Default::default()
            };
            let s = transition(&before, &Input::Hook(Signal::Idle { last_line: None }), 900);
            assert_eq!(
                (s.status, s.last_signal_at),
                (status, Some(900)),
                "{status}"
            );
            let s = transition(&before, &Input::AskHuman("q".into()), 900);
            assert_eq!(s.status, status);
        }
    }

    #[test]
    fn stale_after_five_minutes_of_silence() {
        let at = 1_000_000;
        assert!(!is_stale(Status::Working, Some(at), at + 4 * 60_000));
        assert!(!is_stale(Status::Working, Some(at), at + STALE_AFTER_MS));
        assert!(is_stale(Status::Working, Some(at), at + STALE_AFTER_MS + 1));
        assert!(!is_stale(
            Status::WorkingInBackground,
            Some(at),
            at + 60 * 60_000
        ));
        assert!(!is_stale(Status::Idle, Some(at), at + 60 * 60_000));
        assert!(!is_stale(Status::Working, None, at));
    }

    #[test]
    fn the_limit_waits_neutrally_until_a_new_turn_or_command() {
        let hit = crate::usage::limit::LimitHit {
            kind: crate::usage::limit::LimitKind::Session,
            text: "You've hit your session limit · resets 3:45pm".into(),
        };
        let stopped = transition(&working(), &Input::Hook(Signal::LimitReached { hit }), 10);
        assert_eq!(stopped.status, Status::WaitingForLimitReset);
        assert!(!stopped.status.is_orange());
        assert_eq!(
            stopped.text.as_deref(),
            Some("You've hit your session limit · resets 3:45pm")
        );
        // Enter alone doesn't change it; the turn that follows does.
        assert_eq!(
            transition(&stopped, &Input::Enter, 20).status,
            Status::WaitingForLimitReset
        );
        assert_eq!(
            transition(&stopped, &Input::Hook(Signal::Working), 20).status,
            Status::Working
        );
        assert_eq!(
            transition(&stopped, &Input::Command, 20).status,
            Status::Working
        );
    }
}
