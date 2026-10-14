//! Flags: facts about a ticket that are independent of its status (SDD §7.2). A card shows them next to the
//! status; which one wins the card's pill slot is the card's concern (T-0031).

use serde::{Deserialize, Serialize};

use super::{TicketKey, stored_enum};

/// "Sent back · 3 comments · round 2" (d0034).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentBack {
    pub comments: u32,
    pub round: u32,
}

/// Every flag of SDD §7.2. Absent flags are `None`, `false` or zero.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flags {
    /// When the ticket was pinned, in Unix milliseconds (d0047). Pinned is not a status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_at: Option<i64>,
    /// Sent back · N comments · round K (d0034).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent_back: Option<SentBack>,
    /// Conflicts with `<base>`: the base branch it conflicts with (d0038).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflicts_with: Option<String>,
    /// Changed since you reviewed it (d0043).
    #[serde(default, skip_serializing_if = "is_false")]
    pub changed_since_review: bool,
    /// N messages waiting to be typed into the session (d0048).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub messages_waiting: u32,
    /// Run is running, on this port: `▶ :4020` (d0035).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_port: Option<u16>,
    /// Updated by agent (d0019).
    #[serde(default, skip_serializing_if = "is_false")]
    pub updated_by_agent: bool,
    /// Answering another ticket's question: "answering SHOP-40.2" (d0030).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answering: Option<TicketKey>,
    /// Asked another ticket and waits for the answer: "asked SHOP-40.1 · waiting" (d0030). The oldest open
    /// question; it lasts until the answer was typed into this ticket's session (MSG-06).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asked: Option<TicketKey>,
    /// The oldest open question's text ("Waiting for an answer: …").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asked_question: Option<String>,
    /// More open questions besides the oldest: "asked SHOP-40.1 +1 · waiting".
    #[serde(default, skip_serializing_if = "is_zero")]
    pub asked_more: u32,
    /// When the agent last signalled, in Unix milliseconds (d0027).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_signal_at: Option<i64>,
    /// Tokens used so far (d0020).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
    /// The ticket ran a conversation whose token count couldn't be read (Claude Code's file is missing or
    /// changed format): shown as "—" (USG-01). Not a flag of its own.
    #[serde(default, skip_serializing_if = "is_false")]
    pub tokens_unknown: bool,
    /// PR #N (d0015, d0017).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_number: Option<u32>,
    /// Back from Approved for PR rework and not submitted again: "Back for rework · Send back to start the agent"
    /// (d0041). Once resubmitted, Approve offers Push to PR.
    #[serde(default, skip_serializing_if = "is_false")]
    pub pr_rework: bool,
    /// "PR provider not detected" (d0050).
    #[serde(default, skip_serializing_if = "is_false")]
    pub pr_provider_not_detected: bool,
    /// In Review because the user submitted it (d0052): the note, possibly empty ("Submitted by you · “…”").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submitted_by_you: Option<String>,
    /// A parked ticket back in Dev, about to continue its conversation: "Resuming conversation…" (d0052).
    #[serde(default, skip_serializing_if = "is_false")]
    pub resuming_conversation: bool,
    /// How it reached Review (d0063): "Validated · round 2", "Submitted by you"…
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_entry: Option<super::ReviewEntryFlag>,
    /// A working card whose last validation round failed and went back to the agent: "Validation failed ·
    /// round 1 of 3".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_round: Option<super::ValidationRound>,
    /// Validating… or Needs you · validator: the round the validator checks, "validator · round 1 of 3" (d0063).
    /// Not a flag of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validating_round: Option<super::ValidationRound>,
    /// The last failed round's findings (its report lines that aren't notes), while the agent fixes them or after
    /// the last round failed: "· 2 findings", and the first one on "Validation failed 3 times" (d0063). Not a flag
    /// of its own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
    /// The files the recorded conflict touches: "Merge conflict · fixing · lib/wishlist.ts" (d0064). Not a flag
    /// of its own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflict_files: Vec<String>,
    /// Done through Auto-merge: "Merged automatically" (d0064).
    #[serde(default, skip_serializing_if = "is_false")]
    pub merged_automatically: bool,
    /// A Backlog ticket (not an epic) whose dependencies are all merged: the quiet "Ready to start" hint. It never
    /// starts by itself (d0066, SCH-02).
    #[serde(default, skip_serializing_if = "is_false")]
    pub ready_to_start: bool,
    /// A needs-you status while the user's terminal for the waiting session has focus (ATT-03, d0042; per session,
    /// so the validator's prompt calms only with the validator's terminal, d0063): the card stays calm, no inbox
    /// item, no notification; the terminal strip shows "your turn".
    #[serde(default, skip_serializing_if = "is_false")]
    pub your_turn: bool,
}

fn is_false(b: &bool) -> bool {
    !b
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

stored_enum! {
    /// The name of each flag, for code that ranks or lists them.
    pub enum Flag ("flag") {
        Pinned = "pinned",
        SentBack = "sent_back",
        ConflictsWithBase = "conflicts_with_base",
        ChangedSinceReview = "changed_since_review",
        MessagesWaiting = "messages_waiting",
        RunRunning = "run_running",
        UpdatedByAgent = "updated_by_agent",
        Answering = "answering",
        Asked = "asked",
        LastSignal = "last_signal",
        Tokens = "tokens",
        PrNumber = "pr_number",
        PrRework = "pr_rework",
        PrProviderNotDetected = "pr_provider_not_detected",
        SubmittedByYou = "submitted_by_you",
        ResumingConversation = "resuming_conversation",
        ReviewEntry = "review_entry",
        ValidationRound = "validation_round",
        MergedAutomatically = "merged_automatically",
        YourTurn = "your_turn",
    }
}

impl Flags {
    pub fn is_pinned(&self) -> bool {
        self.pinned_at.is_some()
    }

    /// The flags that are set, in declaration order.
    pub fn set(&self) -> Vec<Flag> {
        let present = [
            (Flag::Pinned, self.pinned_at.is_some()),
            (Flag::SentBack, self.sent_back.is_some()),
            (Flag::ConflictsWithBase, self.conflicts_with.is_some()),
            (Flag::ChangedSinceReview, self.changed_since_review),
            (Flag::MessagesWaiting, self.messages_waiting > 0),
            (Flag::RunRunning, self.run_port.is_some()),
            (Flag::UpdatedByAgent, self.updated_by_agent),
            (Flag::Answering, self.answering.is_some()),
            (Flag::Asked, self.asked.is_some()),
            (Flag::LastSignal, self.last_signal_at.is_some()),
            (Flag::Tokens, self.tokens.is_some()),
            (Flag::PrNumber, self.pr_number.is_some()),
            (Flag::PrRework, self.pr_rework),
            (Flag::PrProviderNotDetected, self.pr_provider_not_detected),
            (Flag::SubmittedByYou, self.submitted_by_you.is_some()),
            (Flag::ResumingConversation, self.resuming_conversation),
            (Flag::ReviewEntry, self.review_entry.is_some()),
            (Flag::ValidationRound, self.validation_round.is_some()),
            (Flag::MergedAutomatically, self.merged_automatically),
            (Flag::YourTurn, self.your_turn),
        ];
        present
            .into_iter()
            .filter_map(|(flag, on)| on.then_some(flag))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_set() -> Flags {
        Flags {
            pinned_at: Some(1),
            sent_back: Some(SentBack {
                comments: 3,
                round: 2,
            }),
            conflicts_with: Some("main".into()),
            changed_since_review: true,
            messages_waiting: 2,
            run_port: Some(4020),
            updated_by_agent: true,
            answering: Some("SHOP-40.2".parse().unwrap()),
            asked: Some("SHOP-40.1".parse().unwrap()),
            asked_question: Some("Which endpoint?".into()),
            asked_more: 1,
            last_signal_at: Some(5),
            tokens: Some(18_400),
            tokens_unknown: false,
            pr_number: Some(482),
            pr_rework: true,
            pr_provider_not_detected: true,
            submitted_by_you: Some("Paging works".into()),
            resuming_conversation: true,
            review_entry: Some(crate::domain::ReviewEntryFlag {
                entry: crate::domain::ReviewEntry::Validated,
                round: 2,
            }),
            validation_round: Some(crate::domain::ValidationRound { round: 1, of: 3 }),
            validating_round: Some(crate::domain::ValidationRound { round: 2, of: 3 }),
            findings: vec!["src/cart.ts:12 the total ignores the coupon".into()],
            conflict_files: vec!["lib/wishlist.ts".into()],
            merged_automatically: true,
            ready_to_start: true,
            your_turn: true,
        }
    }

    #[test]
    fn every_flag_is_reported_once() {
        assert_eq!(all_set().set(), Flag::ALL);
        assert!(Flags::default().set().is_empty());
    }

    #[test]
    fn round_trips_and_stays_small_when_empty() {
        let flags = all_set();
        let json = serde_json::to_string(&flags).unwrap();
        assert_eq!(serde_json::from_str::<Flags>(&json).unwrap(), flags);
        assert_eq!(serde_json::to_string(&Flags::default()).unwrap(), "{}");
        for flag in Flag::ALL {
            assert_eq!(flag.as_str().parse::<Flag>().unwrap(), *flag);
        }
    }
}
