//! The terminal's header strip and the typing prompts (SES-11, SES-06, SES-07; d0048, d0040): the session state,
//! where the agent works, its `$PORT`, "1 message waiting · Deliver now", the attach command, and the
//! "SHOP-23 has unsent text in its terminal" prompt with Keep it and cancel / Clear it and send.

use crewboard_core::domain::{Status, TicketKey};
use crewboard_core::naming;
use crewboard_core::protocol::{DeliverNowParams, Request, SessionTarget};

use crate::terminal::model::Phase;

/// How the strip's state dot and label read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Blue: the agent works.
    Working,
    /// Orange: the agent waits for you.
    Attention,
    Muted,
}

/// The strip's first item: "Live", "your turn", "Waiting for you", "Reconnecting…", "Ended". `your_turn`: the
/// terminal has focus and the agent waits, so only a subtle "your turn" (ATT-03, d0042).
pub fn state(phase: &Phase, status: Status, your_turn: bool) -> (&'static str, Tone) {
    match phase {
        Phase::Live if your_turn => ("your turn", Tone::Muted),
        Phase::Live if status.is_orange() => ("Waiting for you", Tone::Attention),
        Phase::Live => ("Live", Tone::Working),
        Phase::Attaching => ("Connecting…", Tone::Muted),
        Phase::Reconnecting => ("Reconnecting…", Tone::Muted),
        Phase::Ended => ("Ended", Tone::Muted),
        Phase::NoSession => ("No session", Tone::Muted),
    }
}

/// The terminal's focus as the service is told it (ATT-03): the keyboard is in the terminal and the Terminal tab is
/// on screen. Panel close, a tab switch, a click outside and ⇧⌘` all make it false.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerminalFocus {
    pub has_focus: bool,
    pub hidden: bool,
}

impl TerminalFocus {
    pub fn reported(self) -> bool {
        self.has_focus && !self.hidden
    }

    /// Applies a change; returns the focus to report when what the service knows changes.
    pub fn apply(&mut self, change: FocusChange) -> Option<bool> {
        let before = self.reported();
        match change {
            FocusChange::Gained => self.has_focus = true,
            FocusChange::Lost => self.has_focus = false,
            FocusChange::Hidden => self.hidden = true,
            FocusChange::Shown => self.hidden = false,
        }
        (self.reported() != before).then_some(self.reported())
    }
}

/// What happened to the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusChange {
    Gained,
    Lost,
    Hidden,
    Shown,
}

/// The worktree as the strip shows it: `…/wt/shop-web/SHOP-19`.
pub fn short_path(worktree: &str) -> String {
    let parts: Vec<&str> = worktree.split('/').filter(|p| !p.is_empty()).collect();
    match parts.len() {
        0..=3 => worktree.to_owned(),
        n => format!("…/{}", parts[n - 3..].join("/")),
    }
}

/// `agent :4019`.
pub fn agent_port(port: u16) -> String {
    format!("agent :{port}")
}

/// `crewboard attach SHOP-19`, as copied by the strip's chip.
pub fn attach_command(key: &TicketKey) -> String {
    format!("{} attach {key}", naming::COMMAND_NAME)
}

/// "1 message waiting" / "3 messages waiting"; `None` when nothing waits.
pub fn waiting_label(count: u32) -> Option<String> {
    match count {
        0 => None,
        1 => Some("1 message waiting".to_owned()),
        n => Some(format!("{n} messages waiting")),
    }
}

/// "Instructions: shop-web" (d0040).
pub fn instructions_link(project_name: &str) -> String {
    format!("Instructions: {project_name}")
}

/// The question asked when a click would type into a line holding the user's unsent text (SES-07).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsentPrompt {
    pub ticket: TicketKey,
    /// The terminal's input line as it is now, when the app can read it.
    pub typed: Option<String>,
    /// What the click wants to send, as the prompt describes it.
    pub sending: String,
    /// The click that asked; "Clear it and send" repeats it with the line cleared first.
    pub click: Click,
}

/// The user's click that stopped at the unsent-text question (SES-07, d0048).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Click {
    DeliverNow,
    /// Send back with this note (REV-04).
    SendBack {
        note: String,
    },
    /// Send to agent to fix (GIT-03).
    SendToFix,
    /// An answer to the agent's question from the inbox, card or panel (ATT-08, ATT-12).
    Answer {
        choice: crewboard_core::board::questions::Choice,
    },
}

impl UnsentPrompt {
    /// For any other click: `sending` says what it types.
    pub fn for_click(
        ticket: TicketKey,
        typed: Option<String>,
        sending: String,
        click: Click,
    ) -> Self {
        Self {
            ticket,
            typed,
            sending,
            click,
        }
    }

    /// For Deliver now: the waiting messages.
    pub fn deliver_now(ticket: TicketKey, typed: Option<String>, waiting: u32) -> Self {
        let sending = match waiting {
            0 | 1 => "The waiting message".to_owned(),
            n => format!("The {n} waiting messages"),
        };
        Self {
            ticket,
            typed,
            sending,
            click: Click::DeliverNow,
        }
    }

    pub fn title(&self) -> String {
        format!("{} has unsent text in its terminal", self.ticket)
    }

    pub fn explanation(&self) -> &'static str {
        "Clear it and send clears the typed text, then sends. Keep it and cancel leaves the terminal as it is; \
         nothing is sent."
    }
}

/// The user's answer to the prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Keep,
    Clear,
}

/// The request for a Deliver now click and the prompt's answer: Keep sends nothing (and clears nothing); Clear
/// asks the service to clear the line, then deliver.
pub fn deliver_now_request(ticket: &TicketKey, choice: Option<Choice>) -> Option<Request> {
    let target = SessionTarget {
        session: None,
        key: Some(ticket.to_string()),
        ..Default::default()
    };
    match choice {
        Some(Choice::Keep) => None,
        Some(Choice::Clear) => Some(Request::SessionDeliverNow(DeliverNowParams {
            target,
            clear_first: true,
        })),
        None => Some(Request::SessionDeliverNow(DeliverNowParams {
            target,
            clear_first: false,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_follows_the_keyboard_and_the_tab() {
        let mut f = TerminalFocus::default();
        assert_eq!(f.apply(FocusChange::Gained), Some(true));
        assert_eq!(f.apply(FocusChange::Hidden), Some(false), "another tab");
        assert_eq!(f.apply(FocusChange::Shown), Some(true));
        assert_eq!(f.apply(FocusChange::Gained), None, "no repeat");
        assert_eq!(
            f.apply(FocusChange::Lost),
            Some(false),
            "click outside, ⇧⌘`"
        );
        assert_eq!(f.apply(FocusChange::Hidden), None);
        assert_eq!(f.apply(FocusChange::Shown), None);
    }

    fn key() -> TicketKey {
        "SHOP-23".parse().unwrap()
    }

    #[test]
    fn states() {
        assert_eq!(
            state(&Phase::Live, Status::Working, false),
            ("Live", Tone::Working)
        );
        assert_eq!(
            state(&Phase::Live, Status::NeedsYou, false),
            ("Waiting for you", Tone::Attention)
        );
        assert_eq!(
            state(&Phase::Reconnecting, Status::Working, false).0,
            "Reconnecting…"
        );
        assert_eq!(
            state(&Phase::Ended, Status::ReadyForReview, false).0,
            "Ended"
        );
        // The terminal has focus and the agent waits: only "your turn" (ATT-03).
        assert_eq!(
            state(&Phase::Live, Status::Idle, true),
            ("your turn", Tone::Muted)
        );
    }

    #[test]
    fn strip_texts() {
        assert_eq!(
            short_path("/Users/me/.data/wt/shop-web/SHOP-19"),
            "…/wt/shop-web/SHOP-19"
        );
        assert_eq!(short_path("wt/a/B-1"), "wt/a/B-1");
        assert_eq!(agent_port(4019), "agent :4019");
        assert_eq!(
            attach_command(&"SHOP-19".parse().unwrap()),
            format!("{} attach SHOP-19", naming::COMMAND_NAME)
        );
        assert_eq!(instructions_link("shop-web"), "Instructions: shop-web");
    }

    #[test]
    fn chip_counts() {
        assert_eq!(waiting_label(0), None);
        assert_eq!(waiting_label(1).as_deref(), Some("1 message waiting"));
        assert_eq!(waiting_label(3).as_deref(), Some("3 messages waiting"));
    }

    #[test]
    fn prompt_texts_and_outcomes() {
        let prompt = UnsentPrompt::deliver_now(key(), Some("> Only the profile form".into()), 1);
        assert_eq!(prompt.title(), "SHOP-23 has unsent text in its terminal");
        assert_eq!(prompt.sending, "The waiting message");
        assert_eq!(
            UnsentPrompt::deliver_now(key(), None, 2).sending,
            "The 2 waiting messages"
        );
        // Keep it and cancel: nothing is sent and nothing cleared.
        assert_eq!(deliver_now_request(&key(), Some(Choice::Keep)), None);
        // Clear it and send: the service clears the line, then delivers.
        assert!(matches!(
            deliver_now_request(&key(), Some(Choice::Clear)),
            Some(Request::SessionDeliverNow(DeliverNowParams {
                clear_first: true,
                ..
            }))
        ));
        // The first click never clears.
        assert!(matches!(
            deliver_now_request(&key(), None),
            Some(Request::SessionDeliverNow(DeliverNowParams {
                clear_first: false,
                ..
            }))
        ));
    }
}
