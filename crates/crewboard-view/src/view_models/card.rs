//! What a ticket card shows (*crewboard.pen → 04 · Ticket card variants*, UI-03). A card answers four
//! questions at a glance: what is it (key + title), is it OK (the pill, empty when the status is the column's
//! normal one), does it need me (orange border + callout), what is it waiting on (the meta row).
//!
//! **One pill, one callout** (*Which flag wins the pill slot*): the highest flag takes the pill, lower flags
//! fall back to the meta row, so nothing is lost:
//! 1 orange (Needs you · Asks you · Idle · Needs you · validator · Failed 3 times · No result · Keeps
//! conflicting, and the other Auto-merge stops: Base diverged, Checkout dirty) · 2 Interrupted · Start interrupted
//! (· Resume failed) · 3 Parent closed · 4 Waiting for limit reset · 5 Validating… · Working in background (calm;
//! with the working cards) · 6 stale signal (quiet 5+ min) · 8 conflicts with the base · 9 PR open · 10 Parked ·
//! has work · 11 none. Rank 7 is "asked another agent" (MSG-06): "Asked SHOP-40.1" in the pill, else "asked
//! SHOP-40.1 · waiting" in the meta row. **Pinned is not a status**: a pin icon sits next to the key and never
//! takes the pill; neither does the priority (P1, P2, P4 after the key, d0065).
//!
//! **Markers on a working card use the callout, not the pill** (*Note · M12*, d0063, d0064): "Validation failed ·
//! round N of M" ranks like "Sent back", and "Merge conflict · fixing" (its own status) replaces "Conflicts with
//! main". **How a ticket reached Review** ("Validated · round N", "Validation skipped by you", "Submitted by you ·
//! not validated", "Did not pass · failed 3 times · moved by you"), "Merged automatically" and "Ready to start" use
//! the Entry row under the title; "Merged automatically" wins over the Review entry on a Done card.
//!
//! Data that arrives in later milestones (questions M4, waits-for text M5, review flags M6, usage M8, Run M9,
//! PR M10) comes in through [`CardExtras`] and [`crewboard_core::domain::Flags`]; fixtures fill it until then.

use crewboard_core::board::tickets::{Base, Ticket};
use crewboard_core::domain::{Column, Priority, Status, TicketKey};
use crewboard_core::signals::background::{minutes, wakeup_text};
use crewboard_core::signals::{HELPER_PROMPT_PREFIX, is_stale};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;
use crate::theme::Status as Legend;
use crate::view_models::{priority, status};

/// The last signal turns amber after this long (*Badges & pills → Last signal*); the card's pill says "Quiet N
/// min" once the core calls it stale ([`crewboard_core::signals::STALE_AFTER_MS`], ATT-05).
pub const SIGNAL_AMBER_MS: i64 = 3 * 60 * 1000;

/// Facts about a ticket that aren't on [`Ticket`] yet or come from elsewhere on the board.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardExtras {
    /// The project's base branch; a ticket on another base says so ("on release/2.3").
    pub project_base: Option<String>,
    /// The epic's title, for the epic chip on its children.
    pub epic_title: Option<String>,
    /// Queued: what it waits for ("waits for SHOP-20 to merge", T-0077, T-0083).
    pub waits_for: Option<String>,
    /// A parent only waits for the user's merge (Review, PR open): the waiting text is a callout with the
    /// Queued pill (*Queued · waits for merge*); otherwise it sits in the meta row (*Queued · waiting on deps*).
    pub waits_for_merge: bool,
    /// Every dependency is merged and the scheduler is starting it (*Queued · ready to start*: "Starting…").
    pub starting: bool,
    /// The question or prompt of an orange ticket.
    pub question: Option<String>,
    /// The prompt comes from a helper agent inside the session.
    pub helper_agent: bool,
    /// Parent closed: the parent's key.
    pub closed_parent: Option<TicketKey>,
    /// Held: why ("`develop` and `origin/develop` have diverged. Push or pull, then retry.").
    pub held_reason: Option<String>,
    /// Held by the plan's usage limit (SCH-04): it starts by itself after the reset, so no Retry.
    pub held_by_limit: bool,
    /// Start interrupted: the step that failed ("setup command failed").
    pub start_failure: Option<String>,
    /// Working in background: what runs ("test run, 1 helper agent · 6 min") or a wakeup time.
    pub background: Option<String>,
    pub background_wakeup: bool,
    /// The key the ticket had before Move to epic.
    pub old_key: Option<TicketKey>,
    /// Lines added, removed and files changed (Review).
    pub diff: Option<(u32, u32, u32)>,
    /// The PR provider's name ("GitHub").
    pub pr_provider: Option<String>,
    /// The user's terminal has focus on this ticket and the agent ended its turn: calm "your turn".
    pub your_turn: bool,
    /// The branch name shown on a parked ticket.
    pub branch: Option<String>,
    /// The current name of the Project agent that created it ("by Project agent “Checkout slowness”", d0068).
    pub agent_name: Option<String>,
}

impl CardExtras {
    /// The extras that come from the agent's live signal (T-0075; d0042): the question, prompt or last line of
    /// an orange ticket (and whether a helper agent asks), or what a ticket works on in the background with how
    /// long it has run ("test run, 1 helper agent · 6 min"), or when it wakes up ("wakes up at 4:10pm").
    pub fn with_signal(mut self, ticket: &Ticket, now_ms: i64) -> Self {
        let Some(signal) = &ticket.signal else {
            return self;
        };
        if ticket.status.is_orange() {
            match signal
                .text
                .as_deref()
                .and_then(|t| t.strip_prefix(HELPER_PROMPT_PREFIX))
            {
                Some(prompt) => {
                    self.helper_agent = true;
                    self.question = Some(prompt.to_owned());
                }
                None => self.question = signal.text.clone(),
            }
        } else if ticket.status == Status::WorkingInBackground {
            match (&signal.text, signal.wakeup_at) {
                (Some(text), _) => {
                    let tasks = text.strip_prefix("background: ").unwrap_or(text);
                    let since = signal.since.unwrap_or(now_ms);
                    self.background = Some(format!("{tasks} · {}", minutes(now_ms - since)));
                }
                (None, Some(at)) => {
                    self.background = Some(wakeup_text(at));
                    self.background_wakeup = true;
                }
                (None, None) => {}
            }
        }
        self
    }
}

/// The hint on an unblocked Backlog card.
pub const READY_TO_START: &str = "Ready to start";

/// The tone of a callout or a meta item. `CardTone` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "CardTone")]
pub enum Tone {
    Muted,
    Attention,
    Error,
    Warning,
    Working,
    Review,
    Running,
    Epic,
    Accent,
    /// Secondary text (an Entry's text next to a coloured icon).
    Secondary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pill {
    pub legend: Legend,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CardCallout")]
pub struct Callout {
    pub icon: IconName,
    pub tone: Tone,
    pub text: String,
    /// Mono text (a command or prompt).
    pub mono: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CardMetaItem")]
pub struct MetaItem {
    pub icon: IconName,
    pub text: String,
    pub tone: Tone,
    /// Shown on hover ("—" tokens say why).
    pub tooltip: Option<&'static str>,
}

/// A quiet line under the title, the frame's "Entry": an icon and a short text ("Ready to start", "Validated ·
/// round 2").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CardEntry")]
pub struct Entry {
    pub icon: IconName,
    pub text: String,
    /// The icon's colour.
    pub tone: Tone,
    /// The text's colour.
    pub text_tone: Tone,
}

/// A bar under the title: messages waiting to be typed, background work, or the validator at work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bar {
    MessagesWaiting(u32),
    Background {
        text: String,
        wakeup: bool,
    },
    /// Validating…: "validator · round 1 of 3 · 2 min" (d0063).
    Validator(String),
}

/// On the page: `"resume"`, `{ "pick_option": 2 }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CardActionKind {
    /// "Deliver now" on the message-waiting bar (SES-06).
    DeliverNow,
    /// A click on an answer option (ATT-12): the panel opens with it picked (the last number is Other…).
    PickOption(u32),
    /// "Send answer" with the preselected (recommended) option.
    SendAnswer,
    Resume,
    /// Resume failed: the same resume again.
    RetryResume,
    /// Resume failed: a new session in the same worktree, after the panel's confirmation.
    StartFromScratch,
    RetryStart,
    Review,
    /// A ticket back from PR rework and submitted again: its new commits go to the PR (REV-14).
    PushToPr,
    MarkMerged,
    MarkClosed,
    Reopen,
    Retry,
    /// The `▶ :4020` chip: its menu (Open, Stop; RUN-01).
    RunChip,
    /// Validation ended without a result: a fresh validator (VAL-07).
    RetryValidation,
    /// Auto-merge stopped: run the merge again (AMG-03, AMG-04).
    RetryMerge,
    /// Auto-merge stopped: back to Ready for Review, still validated, not merged again.
    MoveToReview,
}

impl CardActionKind {
    /// The validation action a card button sends (T-0187, T-0189), if it is one.
    pub fn validation_action(self) -> Option<crewboard_core::domain::ValidationAction> {
        use crewboard_core::domain::ValidationAction as A;
        match self {
            Self::RetryValidation => Some(A::Retry),
            Self::RetryMerge => Some(A::RetryMerge),
            Self::MoveToReview => Some(A::MoveToReview),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct CardAction {
    pub kind: CardActionKind,
    pub label: &'static str,
    pub icon: Option<IconName>,
    pub primary: bool,
}

/// The card's outline. `CardOutline` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "CardOutline")]
pub enum Outline {
    Normal,
    /// Orange: blocked on the user.
    Attention,
    /// Red: a closed parent blocks it.
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardViewModel {
    pub key: TicketKey,
    /// "SHOP-51 → SHOP-40.4" after Move to epic.
    pub key_label: String,
    pub pinned: bool,
    /// The Priority chip after the key: P1, P2 and P4; `None` for Normal (TKT-09, d0065). An epic child shows the
    /// priority it follows from its epic the same way.
    pub priority: Option<Priority>,
    pub title: String,
    pub pill: Option<Pill>,
    pub callout: Option<Callout>,
    /// A quiet line under the title (*04 · Backlog · P4 · Ready to start*).
    pub entry: Option<Entry>,
    pub bars: Vec<Bar>,
    /// The epic chip ("Wishlist").
    pub epic: Option<String>,
    pub meta: Vec<MetaItem>,
    pub outline: Outline,
    pub actions: Vec<CardAction>,
    /// Done cards fade back: (column-coloured fill, opacity).
    pub faded: Option<f32>,
    /// Asks you with answer options (ATT-12, d0061): listed compactly under the question.
    pub options: Option<crate::view_models::answer_options::AnswerOptions>,
    /// A Project agent just created it: the subtle highlight that fades (*18 → live board feedback*).
    pub fresh: bool,
}

/// "setup command failed" → "Setup command failed".
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Why a token count shows "—" (USG-01): never an error, only a display that can't be filled.
pub const TOKENS_UNKNOWN: &str = "Token count unknown: Claude Code's session file couldn't be read";

/// "182k", "1.84M", "12.4M", "950".
pub fn tokens_label(tokens: u64) -> String {
    if tokens >= 10_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.)
    } else if tokens >= 1_000_000 {
        format!("{:.2}M", tokens as f64 / 1_000_000.)
    } else if tokens >= 1_000 {
        format!("{}k", (tokens as f64 / 1_000.).round() as u64)
    } else {
        tokens.to_string()
    }
}

/// "40s", "4m", "2h".
pub fn age_label(ms: i64) -> String {
    let secs = (ms / 1000).max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

/// Whether a ticket shows a time that moves on its own: a last signal or a background run on a Dev card.
pub fn ticks(ticket: &Ticket) -> bool {
    crewboard_core::signals::LIVE.contains(&ticket.status) && ticket.flags.last_signal_at.is_some()
}

/// The statuses a column already implies; their pill stays empty.
pub fn base_statuses(column: Column) -> &'static [Status] {
    match column {
        Column::Backlog => &[Status::Backlog],
        Column::ReadyForDev => &[Status::Queued],
        Column::Dev => &[Status::Working],
        Column::ReadyForReview => &[Status::ReadyForReview],
        // Approved and Done hold more than one end state, so their pills always show.
        Column::Approved | Column::Done => &[],
    }
}

fn is_working(status: Status) -> bool {
    matches!(status, Status::Working)
}

impl CardViewModel {
    pub fn from(ticket: &Ticket, extras: &CardExtras, now_ms: i64) -> Self {
        // The user is in its terminal: the card stays calm, "your turn · in terminal" (ATT-03, d0042).
        if ticket.flags.your_turn {
            let calm = Ticket {
                status: Status::Working,
                signal: None,
                question: None,
                flags: crewboard_core::domain::Flags {
                    your_turn: false,
                    ..ticket.flags.clone()
                },
                ..ticket.clone()
            };
            let extras = CardExtras {
                your_turn: true,
                ..extras.clone()
            };
            let mut card = Self::from(&calm, &extras, now_ms);
            card.pill = Some(Pill {
                legend: Legend::AgentWorking,
                label: Legend::AgentWorking.label().to_owned(),
            });
            return card;
        }
        let status = ticket.status;
        let flags = &ticket.flags;
        let quiet_ms = flags.last_signal_at.map(|at| now_ms - at);
        let stale = is_stale(status, flags.last_signal_at, now_ms);

        // The pill: the highest flag, else the status when it isn't the column's normal one.
        // An orange ticket interrupted by a restart keeps its label (d0045).
        let kept = ticket.interrupted_from;
        let status_pill = || {
            let (legend, label) = status::pill(kept.unwrap_or(status));
            Pill {
                legend,
                label: kept
                    .is_none()
                    .then(|| short_label(ticket))
                    .flatten()
                    .unwrap_or_else(|| label.to_owned()),
            }
        };
        let pill = if status.is_orange()
            || matches!(
                status,
                Status::Interrupted
                    | Status::StartInterrupted
                    | Status::ResumeFailed
                    | Status::ParentClosed
                    | Status::WaitingForLimitReset
                    | Status::Validating
                    | Status::WorkingInBackground
            ) {
            Some(status_pill())
        } else if status == Status::MergeConflictFixing {
            // A marker on a working card: the callout says it, the pill stays free (*Note · M12*).
            asked_pill(flags).map(|label| Pill {
                legend: Legend::Asked,
                label,
            })
        } else if stale {
            Some(Pill {
                legend: Legend::Parked,
                label: format!("Quiet {} min", quiet_ms.unwrap_or(0) / 60_000),
            })
        } else if let Some(label) = asked_pill(flags) {
            Some(Pill {
                legend: Legend::Asked,
                label,
            })
        } else if let Some(base) = &flags.conflicts_with {
            Some(Pill {
                legend: Legend::Conflicts,
                label: format!("Conflicts with {base}"),
            })
        } else if let (Some(n), true) = (flags.pr_number, status == Status::PrOpen) {
            Some(Pill {
                legend: Legend::PrOpen,
                label: format!("PR #{n}"),
            })
        } else if status == Status::Parked || !base_statuses(ticket.column).contains(&status) {
            Some(status_pill())
        } else if status.is_queued() && flags.resuming_conversation {
            Some(Pill {
                legend: Legend::Queued,
                label: "Resuming conversation…".into(),
            })
        } else if status == Status::Queued && extras.starting {
            Some(Pill {
                legend: Legend::Queued,
                label: "Starting…".into(),
            })
        } else if status == Status::Queued && extras.waits_for_merge && extras.waits_for.is_some() {
            Some(status_pill())
        } else {
            None
        };

        let mut callout = callout(ticket, extras);

        let mut bars = Vec::new();
        if flags.messages_waiting > 0 {
            bars.push(Bar::MessagesWaiting(flags.messages_waiting));
        }
        if status == Status::WorkingInBackground
            && let Some(text) = &extras.background
        {
            bars.push(Bar::Background {
                text: if extras.background_wakeup {
                    text.clone()
                } else {
                    format!("background: {text}")
                },
                wakeup: extras.background_wakeup,
            });
        }
        if status == Status::Validating {
            bars.push(Bar::Validator(validator_bar(ticket, now_ms)));
        }

        let mut meta = meta_row(ticket, extras, quiet_ms);
        // Rank 7 (MSG-06): with the pill, the question waits in the callout; under a higher flag, in the meta row.
        let asked_has_pill = pill.as_ref().is_some_and(|p| p.legend == Legend::Asked);
        if asked_has_pill {
            if callout.is_none()
                && let Some(q) = &flags.asked_question
            {
                callout = Some(Callout {
                    icon: IconName::MessagesSquare,
                    tone: Tone::Accent,
                    text: format!(
                        "Waiting for an answer: \u{201c}{}\u{201d}",
                        q.lines().next().unwrap_or_default()
                    ),
                    mono: false,
                });
            }
        } else if let Some(text) = asked_meta(flags) {
            meta.push(MetaItem {
                icon: IconName::MessagesSquare,
                text,
                tone: Tone::Accent,
                tooltip: None,
            });
        }
        let outline = if status.is_orange() || kept.is_some() {
            Outline::Attention
        } else if status == Status::ParentClosed {
            Outline::Error
        } else {
            Outline::Normal
        };
        let faded = match status {
            Status::Merged => Some(0.55),
            Status::ClosedNotMerged => Some(0.8),
            _ => None,
        };
        let options = (status == Status::AsksYou)
            .then_some(ticket.question.as_ref())
            .flatten()
            .and_then(crate::view_models::answer_options::AnswerOptions::of);
        let key_label = match &extras.old_key {
            Some(old) => format!("{old} → {}", ticket.key),
            None => ticket.key.to_string(),
        };
        let entry = entry(flags);
        Self {
            key: ticket.key.clone(),
            key_label,
            pinned: flags.is_pinned(),
            priority: priority::on_card(ticket.priority),
            title: ticket.title.clone(),
            pill,
            callout,
            entry,
            bars,
            epic: extras.epic_title.clone(),
            meta,
            outline,
            actions: if kept.is_some() {
                // Answered from the inbox or the panel: that click resumes it.
                Vec::new()
            } else if options.as_ref().is_some_and(|o| o.pick.is_some()) {
                vec![CardAction {
                    kind: CardActionKind::SendAnswer,
                    label: "Send answer",
                    icon: Some(IconName::CornerDownLeft),
                    primary: true,
                }]
            } else {
                actions(status, flags.pr_number.is_some(), flags.pr_rework, extras)
            },
            faded,
            options,
            fresh: crate::view_models::agent_activity::fresh_from_project_agent(ticket, now_ms),
        }
    }
}

/// The card's short pill for an M12 stop (*04 → Row · validation & Auto-merge*): "Failed 3 times", "No result",
/// "Keeps conflicting"; the callout and the panel carry the full text.
fn short_label(ticket: &Ticket) -> Option<String> {
    use crewboard_core::domain::AutoMergeStop;
    Some(match ticket.status {
        Status::ValidationFailed => failed_times_short(ticket.validation_round),
        Status::ValidationNoResult => "No result".to_owned(),
        Status::AutoMergeStopped => match ticket.auto_merge_stop? {
            AutoMergeStop::KeepsConflicting => "Keeps conflicting",
            AutoMergeStop::BaseDiverged => "Base diverged",
            AutoMergeStop::CheckoutDirty => "Checkout dirty",
        }
        .to_owned(),
        _ => return None,
    })
}

/// "Failed 3 times", "Failed 1 time" (the core's "Validation failed 3 times" without its subject).
fn failed_times_short(times: u32) -> String {
    let full = crewboard_core::domain::validation::failed_times(times.max(1));
    capitalized(full.trim_start_matches("Validation "))
}

/// "validator · round 1 of 3 · 2 min": which round the validator checks and how long it has run.
fn validator_bar(ticket: &Ticket, now_ms: i64) -> String {
    let mut parts = vec!["validator".to_owned()];
    if let Some(r) = ticket.flags.validating_round {
        parts.push(format!("round {} of {}", r.round, r.of));
    }
    if let Some(since) = ticket.status_since {
        parts.push(minutes(now_ms - since));
    }
    parts.join(" \u{b7} ")
}

/// The Entry row: how the ticket got where it is (*Note · M12*). "Merged automatically" wins on a Done card, then
/// the Review entry (shown in Review, Approved and Done), then the Backlog hint.
fn entry(flags: &crewboard_core::domain::Flags) -> Option<Entry> {
    use crewboard_core::domain::ReviewEntry;
    let e = |icon, text: String, tone, text_tone| {
        Some(Entry {
            icon,
            text,
            tone,
            text_tone,
        })
    };
    if flags.merged_automatically {
        return e(
            IconName::Bot,
            crewboard_core::domain::validation::MERGED_AUTOMATICALLY.to_owned(),
            Tone::Muted,
            Tone::Muted,
        );
    }
    if let Some(flag) = flags.review_entry {
        let label = flag.entry.label(flag.round);
        return match flag.entry {
            ReviewEntry::Validated => {
                e(IconName::ShieldCheck, label, Tone::Review, Tone::Secondary)
            }
            ReviewEntry::Skipped => e(IconName::ShieldOff, label, Tone::Muted, Tone::Secondary),
            ReviewEntry::SubmittedByUser => e(
                IconName::UserCheck,
                format!("{label} \u{b7} not validated"),
                Tone::Muted,
                Tone::Secondary,
            ),
            // Reads as "did not pass": red, never like a pass.
            ReviewEntry::FailedMoved => {
                let mut rest = label.chars();
                let first = rest.next().map(|c| c.to_lowercase().to_string());
                e(
                    IconName::ShieldX,
                    format!(
                        "Did not pass \u{b7} {}{}",
                        first.unwrap_or_default(),
                        rest.as_str()
                    ),
                    Tone::Error,
                    Tone::Error,
                )
            }
        };
    }
    // An unblocked Backlog ticket: a hint only, it never starts by itself (d0066).
    if flags.ready_to_start {
        return e(
            IconName::CirclePlay,
            READY_TO_START.to_owned(),
            Tone::Muted,
            Tone::Muted,
        );
    }
    None
}

/// The Review entry is the user's Submit for review.
fn submitted_entry(flags: &crewboard_core::domain::Flags) -> bool {
    flags
        .review_entry
        .is_some_and(|e| e.entry == crewboard_core::domain::ReviewEntry::SubmittedByUser)
}

fn callout(ticket: &Ticket, x: &CardExtras) -> Option<Callout> {
    use crewboard_core::domain::{AutoMergeStop, validation};
    let c = |icon: IconName, tone: Tone, text: String| {
        Some(Callout {
            icon,
            tone,
            text,
            mono: false,
        })
    };
    let flags = &ticket.flags;
    match ticket.status {
        // The validator's permission prompt (VAL-11).
        Status::ValidatorNeedsYou => Some(Callout {
            icon: IconName::Shield,
            tone: Tone::Attention,
            text: format!(
                "validator: {}",
                x.question
                    .clone()
                    .unwrap_or_else(|| "Waiting for your permission".into())
            ),
            mono: true,
        }),
        // The last round failed: its first finding, else the count (VAL-06).
        Status::ValidationFailed => c(
            IconName::ShieldX,
            Tone::Attention,
            flags
                .findings
                .first()
                .cloned()
                .unwrap_or_else(|| validation::failed_times(ticket.validation_round.max(1))),
        ),
        // No verdict, or a discarded one: the core's text says which (VAL-02, VAL-07).
        Status::ValidationNoResult => c(
            IconName::ShieldQuestionMark,
            Tone::Attention,
            x.question
                .clone()
                .unwrap_or_else(|| validation::NO_RESULT.to_owned()),
        ),
        // "Keeps conflicting with <epic branch> · 3 tries", or the Local merge's message (AMG-03, AMG-04).
        Status::AutoMergeStopped => c(
            IconName::GitCompareArrows,
            Tone::Attention,
            match (ticket.auto_merge_stop, &flags.conflicts_with) {
                (Some(AutoMergeStop::KeepsConflicting), Some(target)) => format!(
                    "Keeps conflicting with {target} \u{b7} {} tries",
                    ticket.auto_merge_conflicts.max(1)
                ),
                _ => x
                    .question
                    .clone()
                    .unwrap_or_else(|| Status::AutoMergeStopped.label().to_owned()),
            },
        ),
        // A marker on a working card (*Note · M12*): the agent rebases with restack (AMG-03).
        Status::MergeConflictFixing => c(
            IconName::GitCompareArrows,
            Tone::Warning,
            match flags.conflict_files.as_slice() {
                [] => validation::MERGE_CONFLICT_FIXING.to_owned(),
                [file] => format!("{} \u{b7} {file}", validation::MERGE_CONFLICT_FIXING),
                [file, rest @ ..] => format!(
                    "{} \u{b7} {file} +{}",
                    validation::MERGE_CONFLICT_FIXING,
                    rest.len()
                ),
            },
        ),
        Status::NeedsYou => {
            let text = x
                .question
                .clone()
                .unwrap_or_else(|| "Waiting for your permission".into());
            if x.helper_agent {
                c(
                    IconName::Bot,
                    Tone::Attention,
                    format!("helper agent asks: {text}"),
                )
            } else {
                Some(Callout {
                    icon: IconName::Terminal,
                    tone: Tone::Attention,
                    text,
                    mono: true,
                })
            }
        }
        // PR rework (d0041): dragged back from Approved; Send back starts the agent.
        Status::ReadyForReview if flags.pr_rework => c(
            IconName::RotateCcw,
            Tone::Review,
            "Back for rework \u{b7} Send back to start the agent".into(),
        ),
        Status::AsksYou => c(
            IconName::MessageCircleQuestionMark,
            Tone::Attention,
            x.question
                .clone()
                .unwrap_or_else(|| "Asks you a question".into()),
        ),
        Status::Idle => c(
            IconName::CirclePause,
            Tone::Attention,
            match &x.question {
                Some(q) => format!("Ended its turn: “{q}”"),
                None => "Ended its turn".into(),
            },
        ),
        Status::Interrupted if ticket.interrupted_from.is_some() => c(
            IconName::ZapOff,
            Tone::Attention,
            "Session ended by restart · answering resumes it".into(),
        ),
        Status::Interrupted => c(
            IconName::ZapOff,
            Tone::Error,
            "Session lost after reboot".into(),
        ),
        Status::StartInterrupted => c(
            IconName::CirclePause,
            Tone::Error,
            match &x.start_failure {
                Some(step) => format!("{} · agent never started", capitalized(step)),
                None => "Start didn’t finish · agent never started".into(),
            },
        ),
        Status::ResumeFailed => c(
            IconName::ZapOff,
            Tone::Error,
            "Resume failed · retry or start from scratch".into(),
        ),
        Status::ParentClosed => c(
            IconName::CircleSlash,
            Tone::Error,
            match &x.closed_parent {
                Some(p) => format!(
                    "{p} was closed without merging. Can’t be approved until you reopen it or remove the dependency."
                ),
                None => "A parent was closed without merging".into(),
            },
        ),
        Status::Parked => c(
            IconName::Archive,
            Tone::Warning,
            "Agent stopped · branch, files and conversation kept".into(),
        ),
        Status::Held if x.held_by_limit => c(
            IconName::Gauge,
            Tone::Muted,
            x.held_reason.clone().unwrap_or_else(|| "Held".into()),
        ),
        Status::Held => c(
            IconName::GitCompareArrows,
            Tone::Warning,
            x.held_reason.clone().unwrap_or_else(|| "Held".into()),
        ),
        Status::Queued if x.waits_for_merge && x.waits_for.is_some() => c(
            IconName::Hourglass,
            Tone::Muted,
            x.waits_for.clone().unwrap_or_default(),
        ),
        _ => {
            if let Some(answering) = &flags.answering {
                c(
                    IconName::MessageCircleReply,
                    Tone::Working,
                    format!("answering {answering} · resumed, answer-only"),
                )
            } else if let Some(base) = &flags.conflicts_with {
                c(
                    IconName::GitCompareArrows,
                    Tone::Warning,
                    format!("Conflicts with {base} · found after the last merge"),
                )
            } else if flags.changed_since_review {
                // After a push the PR's content differs from the review (REV-06, T-0145).
                c(
                    IconName::TriangleAlert,
                    Tone::Warning,
                    "Changed since you reviewed it · the pushed content differs".to_owned(),
                )
            } else if let Some(note) = flags
                .submitted_by_you
                .as_ref()
                // With no note, the Entry row already says "Submitted by you · not validated" (d0063).
                .filter(|note| !note.trim().is_empty() || !submitted_entry(flags))
            {
                c(
                    IconName::UserCheck,
                    Tone::Review,
                    crate::view_models::close::submitted_line(note),
                )
            } else if let Some(marker) = flags.validation_round {
                // Ranks like "Sent back": the findings went back to the agent (VAL-04).
                let text = match flags.findings.len() {
                    0 => marker.to_string(),
                    1 => format!("{marker} \u{b7} 1 finding"),
                    n => format!("{marker} \u{b7} {n} findings"),
                };
                c(IconName::ShieldAlert, Tone::Warning, text)
            } else if let Some(sent) = flags.sent_back {
                c(
                    IconName::Undo2,
                    Tone::Warning,
                    format!(
                        "Sent back · {} comment{} · round {}",
                        sent.comments,
                        if sent.comments == 1 { "" } else { "s" },
                        sent.round
                    ),
                )
            } else if let Some(old) = &x.old_key {
                let epic = ticket
                    .epic
                    .as_ref()
                    .map_or_else(String::new, |e| format!(" {e}"));
                c(
                    IconName::Layers,
                    Tone::Epic,
                    format!("Moved into epic{epic} · old key {old} still works"),
                )
            } else {
                None
            }
        }
    }
}

fn meta_row(ticket: &Ticket, x: &CardExtras, quiet_ms: Option<i64>) -> Vec<MetaItem> {
    let item = |icon: IconName, text: String, tone: Tone| MetaItem {
        icon,
        text,
        tone,
        tooltip: None,
    };
    let flags = &ticket.flags;
    let mut meta = Vec::new();
    // A waiting line in the meta row already names the parents (*Queued · waiting on deps*).
    let waits_in_meta = x.waits_for.as_ref().filter(|_| !x.waits_for_merge);
    if !ticket.depends_on.is_empty() && waits_in_meta.is_none() {
        let keys: Vec<String> = ticket.depends_on.iter().map(ToString::to_string).collect();
        meta.push(item(IconName::Link2, deps_label(&keys, 2), Tone::Muted));
    }
    if let Some(w) = waits_in_meta {
        meta.push(item(IconName::Hourglass, w.clone(), Tone::Muted));
    }
    if let Some(origin) = crate::view_models::agent_activity::origin_label(ticket) {
        meta.push(item(IconName::Sparkles, origin.to_owned(), Tone::Epic));
    }
    if let Some(by) =
        crate::view_models::agent_activity::project_agent_label(ticket, x.agent_name.as_deref())
    {
        meta.push(item(IconName::Bot, by, Tone::Muted));
    }
    if let (Base::Branch(base), Some(project)) = (&ticket.base, &x.project_base)
        && base != project
        && !matches!(ticket.status, Status::Merged | Status::ClosedNotMerged)
    {
        meta.push(item(
            IconName::GitBranch,
            format!("on {base}"),
            Tone::Warning,
        ));
    }
    if let Some(branch) = &x.branch {
        meta.push(item(IconName::GitBranch, branch.clone(), Tone::Muted));
    }
    if let Some((added, removed, files)) = x.diff {
        meta.push(item(
            IconName::FileDiff,
            format!("+{added} −{removed}"),
            Tone::Review,
        ));
        meta.push(item(IconName::File, format!("{files} files"), Tone::Muted));
    }
    if let Some(provider) = x.pr_provider.clone().or_else(|| pr_provider_label(ticket)) {
        meta.push(item(IconName::GitPullRequest, provider, Tone::Muted));
    }
    if x.your_turn {
        meta.push(item(
            IconName::Terminal,
            "your turn · in terminal".into(),
            Tone::Accent,
        ));
    } else if let Some(q) = quiet_ms
        && !matches!(
            ticket.status,
            Status::Merged
                | Status::ClosedNotMerged
                | Status::WaitingForLimitReset
                // The agent isn't what this card waits on (*Row · validation & Auto-merge*).
                | Status::Validating
                | Status::ValidationFailed
                | Status::ValidationNoResult
                | Status::AutoMergeStopped
        )
    {
        let tone = if q >= SIGNAL_AMBER_MS && is_working(ticket.status) {
            Tone::Warning
        } else {
            Tone::Muted
        };
        meta.push(item(IconName::Activity, age_label(q), tone));
    }
    if let Some(tokens) = flags.tokens {
        meta.push(item(IconName::Coins, tokens_label(tokens), Tone::Muted));
    } else if flags.tokens_unknown {
        meta.push(MetaItem {
            tooltip: Some(TOKENS_UNKNOWN),
            ..item(IconName::Coins, "—".to_owned(), Tone::Muted)
        });
    }
    if let Some(port) = flags.run_port {
        meta.push(item(IconName::Play, format!(":{port}"), Tone::Running));
    }
    meta
}

/// Rank 7 (MSG-06): "Asked SHOP-40.1", "Asked SHOP-40.1 +1".
pub fn asked_pill(flags: &crewboard_core::domain::Flags) -> Option<String> {
    let to = flags.asked.as_ref()?;
    Some(match flags.asked_more {
        0 => format!("Asked {to}"),
        n => format!("Asked {to} +{n}"),
    })
}

/// The meta-row fallback when a higher flag holds the pill: "asked SHOP-40.1 · waiting", "asked SHOP-40.1 +1 ·
/// waiting".
pub fn asked_meta(flags: &crewboard_core::domain::Flags) -> Option<String> {
    let to = flags.asked.as_ref()?;
    Some(match flags.asked_more {
        0 => format!("asked {to} \u{b7} waiting"),
        n => format!("asked {to} +{n} \u{b7} waiting"),
    })
}

/// "GitHub" / "Azure DevOps" for a ticket with a PR, from the PR's URL (no setting, REV-11).
pub fn pr_provider_label(ticket: &Ticket) -> Option<String> {
    ticket.flags.pr_number?;
    let url = ticket.work.as_ref()?.pr_url.as_deref()?;
    crewboard_core::pr::provider::Provider::detect(url).map(|p| p.name().to_owned())
}

/// Dependency badges: the first `shown` keys, then "+N" (T-0037).
pub fn deps_label(keys: &[String], shown: usize) -> String {
    if keys.len() <= shown {
        keys.join(" ")
    } else {
        format!("{} +{}", keys[..shown].join(" "), keys.len() - shown)
    }
}

fn actions(status: Status, has_pr: bool, pr_rework: bool, x: &CardExtras) -> Vec<CardAction> {
    let a = |kind, label, icon, primary| CardAction {
        kind,
        label,
        icon,
        primary,
    };
    match status {
        Status::Interrupted => vec![a(
            CardActionKind::Resume,
            "Resume",
            Some(IconName::Play),
            true,
        )],
        // *16 → Resume · in progress → resumed → failed*: Retry, and Start from scratch as the primary.
        Status::ResumeFailed => vec![
            a(
                CardActionKind::RetryResume,
                "Retry",
                Some(IconName::RefreshCw),
                false,
            ),
            a(
                CardActionKind::StartFromScratch,
                "Start from scratch",
                Some(IconName::RotateCcw),
                true,
            ),
        ],
        Status::StartInterrupted => vec![a(
            CardActionKind::RetryStart,
            "Retry start",
            Some(IconName::RotateCcw),
            true,
        )],
        // Back from PR rework and submitted again: the new commits go to the PR (REV-14).
        Status::ReadyForReview if has_pr && !pr_rework => vec![a(
            CardActionKind::PushToPr,
            "Push to PR",
            Some(IconName::Upload),
            true,
        )],
        Status::ReadyForReview if has_pr => Vec::new(),
        Status::ReadyForReview => vec![a(
            CardActionKind::Review,
            "Review",
            Some(IconName::Eye),
            true,
        )],
        Status::PrOpen if has_pr => vec![
            a(CardActionKind::MarkMerged, "Mark merged", None, false),
            a(CardActionKind::MarkClosed, "Mark closed", None, false),
        ],
        Status::ClosedNotMerged => vec![a(
            CardActionKind::Reopen,
            "Reopen",
            Some(IconName::RotateCcw),
            false,
        )],
        Status::Held if x.held_reason.is_some() && !x.held_by_limit => vec![a(
            CardActionKind::Retry,
            "Retry",
            Some(IconName::RotateCcw),
            false,
        )],
        // VAL-07: a fresh validator; Move to Review anyway stays in the panel.
        Status::ValidationNoResult => vec![a(
            CardActionKind::RetryValidation,
            "Retry validation",
            Some(IconName::RotateCcw),
            true,
        )],
        // AMG-03, AMG-04.
        Status::AutoMergeStopped => vec![
            a(
                CardActionKind::RetryMerge,
                "Retry merge",
                Some(IconName::RotateCcw),
                true,
            ),
            a(CardActionKind::MoveToReview, "Move to Review", None, false),
        ],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::{NOW, ticket};

    fn live(key: &str, status: Status, text: Option<&str>, since_min: i64) -> Ticket {
        let mut t = ticket(key, "Price table", status);
        t.flags.last_signal_at = Some(NOW - 40_000);
        t.signal = Some(crewboard_core::board::tickets::AgentSignal {
            text: text.map(Into::into),
            since: Some(NOW - since_min * 60_000),
            wakeup_at: None,
        });
        t
    }

    fn from_signal(t: &Ticket, now: i64) -> CardViewModel {
        CardViewModel::from(t, &main_project().with_signal(t, now), now)
    }

    /// T-0075: the live signal states map to the 04 frames' card content.
    #[test]
    fn signals_map_to_card_content() {
        // Working · needs you (SHOP-22): orange outline, mono prompt.
        let vm = from_signal(
            &live(
                "SHOP-22",
                Status::NeedsYou,
                Some("Allow `rm -rf .next`?"),
                0,
            ),
            NOW,
        );
        assert_eq!(vm.outline, Outline::Attention);
        assert_eq!(vm.pill.as_ref().unwrap().label, "Needs you");
        let callout = vm.callout.unwrap();
        assert_eq!(
            (callout.icon, callout.text.as_str(), callout.mono),
            (IconName::Terminal, "Allow `rm -rf .next`?", true)
        );

        // Dev · needs you · helper agent.
        let vm = from_signal(
            &live(
                "SHOP-19",
                Status::NeedsYou,
                Some("helper agent asks: Allow pnpm test --update-snapshots?"),
                0,
            ),
            NOW,
        );
        let callout = vm.callout.unwrap();
        assert_eq!(callout.icon, IconName::Bot);
        assert_eq!(
            callout.text,
            "helper agent asks: Allow pnpm test --update-snapshots?"
        );

        // Dev · Asks you (SHOP-23) and Dev · Idle.
        let q = "Split fullName everywhere, or only in the profile form?";
        let vm = from_signal(&live("SHOP-23", Status::AsksYou, Some(q), 0), NOW);
        assert_eq!(vm.callout.unwrap().text, q);
        let vm = from_signal(
            &live(
                "SHOP-19",
                Status::Idle,
                Some("Should I also update the tests?"),
                0,
            ),
            NOW,
        );
        assert_eq!(
            vm.callout.unwrap().text,
            "Ended its turn: “Should I also update the tests?”"
        );

        // Dev · working in background: calm, with how long it has run; it ticks with `now`.
        let t = live(
            "SHOP-19",
            Status::WorkingInBackground,
            Some("background: test run, 1 helper agent"),
            6,
        );
        let vm = from_signal(&t, NOW);
        assert_eq!(vm.outline, Outline::Normal);
        assert_eq!(
            vm.bars,
            [Bar::Background {
                text: "background: test run, 1 helper agent · 6 min".into(),
                wakeup: false
            }]
        );
        let later = from_signal(&t, NOW + 60_000);
        assert!(
            matches!(&later.bars[0], Bar::Background { text, .. } if text.ends_with("· 7 min"))
        );

        // Dev · background · scheduled wakeup.
        let mut t = live("SHOP-19", Status::WorkingInBackground, None, 0);
        let at = crewboard_core::signals::background::next_fire("10 16 * * *", NOW).unwrap();
        t.signal.as_mut().unwrap().wakeup_at = Some(at);
        let vm = from_signal(&t, NOW);
        assert_eq!(
            vm.bars,
            [Bar::Background {
                text: "wakes up at 4:10pm".into(),
                wakeup: true
            }]
        );
    }

    /// Working · stale signal: the meta age ticks with `now`, and past the core's threshold the pill says so.
    #[test]
    fn the_last_signal_ticks_and_turns_stale() {
        let mut t = ticket("BILL-4", "Invoice PDF export", Status::Working);
        t.flags.last_signal_at = Some(NOW);
        let at = |ms: i64| CardViewModel::from(&t, &main_project(), NOW + ms);
        assert!(
            at(40_000)
                .meta
                .iter()
                .any(|m| m.text == "40s" && m.tone == Tone::Muted)
        );
        assert!(
            at(4 * 60_000)
                .meta
                .iter()
                .any(|m| m.text == "4m" && m.tone == Tone::Warning)
        );
        assert_eq!(at(5 * 60_000).pill, None, "exactly 5 min isn't stale yet");
        assert_eq!(at(6 * 60_000).pill.unwrap().label, "Quiet 6 min");
        let mut bg = t.clone();
        bg.status = Status::WorkingInBackground;
        assert_ne!(
            CardViewModel::from(&bg, &main_project(), NOW + 60 * 60_000)
                .pill
                .unwrap()
                .label,
            "Quiet 60 min",
            "background work is never stale"
        );
        assert!(super::ticks(&t));
        let done = ticket("SHOP-17", "Add Stripe client", Status::Merged);
        assert!(!super::ticks(&done));
    }

    fn main_project() -> CardExtras {
        CardExtras {
            project_base: Some("main".into()),
            ..Default::default()
        }
    }

    #[test]
    fn needs_you_wins_the_pill_and_the_rest_moves_to_the_meta_row() {
        let mut t = ticket("SHOP-22", "Fix flaky login redirect", Status::NeedsYou);
        t.flags.run_port = Some(4024);
        t.depends_on = vec!["SHOP-17".parse().unwrap(), "SHOP-18".parse().unwrap()];
        let x = CardExtras {
            question: Some("Allow rm -rf .next?".into()),
            ..main_project()
        };
        let vm = CardViewModel::from(&t, &x, NOW);
        assert_eq!(vm.pill.as_ref().unwrap().label, "Needs you");
        assert_eq!(vm.outline, Outline::Attention);
        let callout = vm.callout.unwrap();
        assert_eq!(
            (callout.text.as_str(), callout.mono),
            ("Allow rm -rf .next?", true)
        );
        let meta: Vec<_> = vm.meta.iter().map(|m| m.text.as_str()).collect();
        assert_eq!(meta, ["SHOP-17 SHOP-18", ":4024"]);
        assert_eq!(vm.meta[1].tone, Tone::Running);
    }

    #[test]
    fn the_pill_is_empty_for_the_columns_normal_status() {
        for (status, title) in [
            (Status::Backlog, "Add order history page"),
            (Status::Working, "Price table"),
            (Status::Queued, "Order confirmation email"),
            (Status::ReadyForReview, "Checkout API"),
        ] {
            let vm = CardViewModel::from(&ticket("SHOP-31", title, status), &main_project(), NOW);
            assert_eq!(vm.pill, None, "{status}");
        }
        for status in [
            Status::Merged,
            Status::ClosedNotMerged,
            Status::Held,
            Status::WaitingForSlot,
        ] {
            let vm = CardViewModel::from(&ticket("SHOP-17", "x", status), &main_project(), NOW);
            assert!(vm.pill.is_some(), "{status}");
        }
    }

    fn queued(name: &str) -> CardViewModel {
        let v = crate::view_models::card_fixtures::variants()
            .into_iter()
            .find(|v| v.name == name)
            .unwrap();
        CardViewModel::from(&v.ticket, &v.extras, NOW)
    }

    #[test]
    fn queued_variants_say_what_they_wait_for() {
        // Waiting on deps: the line in the meta row, no separate dependency chip, no pill.
        let vm = queued("Queued · waiting on deps");
        assert_eq!(vm.pill, None);
        assert_eq!(vm.callout, None);
        let meta: Vec<_> = vm.meta.iter().map(|m| (m.icon, m.text.as_str())).collect();
        assert_eq!(meta, [(IconName::Hourglass, "waits for SHOP-20 to merge")]);

        // Waits for merge: Queued pill, the line as a callout, the parents in the meta row.
        let vm = queued("Queued · waits for merge");
        assert_eq!(vm.pill.unwrap().label, "Queued");
        let callout = vm.callout.unwrap();
        assert_eq!(callout.icon, IconName::Hourglass);
        assert_eq!(
            callout.text,
            "waits for SHOP-18, SHOP-19 to merge (SHOP-18 in Review)"
        );
        assert_eq!(vm.meta[0].text, "SHOP-18 SHOP-19");

        // Ready to start: "Starting…", and nothing about the description in the meta row (d0067).
        let vm = queued("Queued · ready to start");
        assert_eq!(vm.pill.unwrap().label, "Starting…");
        assert!(vm.meta.is_empty());

        // Held: the diverged message and Retry (which runs Retry start).
        let vm = queued("Queued · held: base diverged");
        assert!(vm.callout.unwrap().text.contains("have diverged"));
        assert_eq!(vm.actions[0].kind, CardActionKind::Retry);
    }

    #[test]
    fn held_by_the_usage_limit_says_so_and_has_no_retry() {
        let held = ticket("SHOP-30", "Coupons", Status::Held);
        let extras = CardExtras {
            held_reason: Some(crewboard_core::scheduler::PLAN_LIMIT_HELD.into()),
            held_by_limit: true,
            ..main_project()
        };
        let vm = CardViewModel::from(&held, &extras, NOW);
        let callout = vm.callout.unwrap();
        assert_eq!(callout.text, crewboard_core::scheduler::PLAN_LIMIT_HELD);
        assert_eq!(callout.tone, Tone::Muted);
        assert!(vm.actions.is_empty());
    }

    #[test]
    fn parked_and_resuming_cards() {
        let parked = ticket("SHOP-25", "Upgrade to Next.js 15", Status::Parked);
        let vm = CardViewModel::from(&parked, &main_project(), NOW);
        assert_eq!(vm.pill.unwrap().label, "Parked · has work");
        let mut back = ticket("SHOP-25", "Upgrade to Next.js 15", Status::Queued);
        back.flags.resuming_conversation = true;
        let vm = CardViewModel::from(&back, &main_project(), NOW);
        assert_eq!(vm.pill.unwrap().label, "Resuming conversation…");
    }

    #[test]
    fn a_question_with_options_lists_them_on_the_card() {
        let mut t = ticket("SHOP-21", "Order confirmation email", Status::AsksYou);
        t.question = Some(crewboard_core::board::questions::HumanQuestion {
            id: "q".into(),
            text: "Which email provider should the order confirmation use?".into(),
            options: vec!["Postmark".into(), "SES".into(), "Resend".into()],
            recommended: Some(1),
            asked_at: 0,
        });
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        let rows = vm.options.as_ref().unwrap().rows();
        assert_eq!(rows.len(), 4);
        assert!(rows[0].recommended && rows[0].picked);
        assert_eq!(vm.actions[0].kind, CardActionKind::SendAnswer);
        // Without a recommendation there is nothing to send from the card: a click on an option opens the panel.
        t.question.as_mut().unwrap().recommended = None;
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert!(vm.actions.is_empty());
        // Free-text questions have no list.
        t.question.as_mut().unwrap().options.clear();
        assert!(
            CardViewModel::from(&t, &main_project(), NOW)
                .options
                .is_none()
        );
    }

    #[test]
    fn a_focused_waiting_ticket_stays_calm() {
        let mut t = ticket("SHOP-19", "Price table", Status::Idle);
        t.flags.your_turn = true;
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(vm.pill.unwrap().label, "Agent Working");
        assert_eq!(vm.outline, Outline::Normal);
        assert!(vm.callout.is_none());
        assert!(vm.meta.iter().any(|m| m.text == "your turn · in terminal"));
        // Leaving the terminal: Idle again.
        t.flags.your_turn = false;
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(vm.outline, Outline::Attention);
    }

    /// *04 → Working · asked another agent* and the flag order (MSG-06): rank 7, below every orange status.
    #[test]
    fn asked_another_agent() {
        let mut t = ticket("SHOP-40.2", "Wishlist button", Status::Working);
        t.flags.asked = Some("SHOP-40.1".parse().unwrap());
        t.flags.asked_question = Some("Which endpoint returns the wishlist?".into());
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(vm.pill.as_ref().unwrap().label, "Asked SHOP-40.1");
        assert_eq!(
            vm.callout.unwrap().text,
            "Waiting for an answer: \u{201c}Which endpoint returns the wishlist?\u{201d}"
        );
        t.flags.asked_more = 1;
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(vm.pill.unwrap().label, "Asked SHOP-40.1 +1");

        // Idle wins the pill; the asked flag falls back to the meta row.
        t.status = Status::Idle;
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(vm.pill.unwrap().label, "Idle");
        assert!(
            vm.meta
                .iter()
                .any(|m| m.text == "asked SHOP-40.1 +1 \u{b7} waiting"),
            "{:?}",
            vm.meta
        );
        // Conflicts (rank 8) is below it.
        t.status = Status::Working;
        t.flags.asked_more = 0;
        t.flags.conflicts_with = Some("main".into());
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(vm.pill.unwrap().label, "Asked SHOP-40.1");
    }

    /// *04 → Done · answering a linked ticket* (MSG-03).
    #[test]
    fn answering_a_linked_ticket() {
        let mut t = ticket("SHOP-40.1", "Wishlist API", Status::Merged);
        t.flags.answering = Some("SHOP-40.2".parse().unwrap());
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(
            vm.callout.unwrap().text,
            "answering SHOP-40.2 \u{b7} resumed, answer-only"
        );
    }

    /// *04 → Approved · PR open, Review · PR rework, Review · after rework* (REV-13, REV-14).
    #[test]
    fn pr_cards_through_rework() {
        let mut t = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
        t.flags.pr_number = Some(481);
        t.work = Some(crewboard_core::board::tickets::Work {
            pr_url: Some("https://github.com/acme/shop-web/pull/481".into()),
            ..Default::default()
        });
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(vm.pill.as_ref().unwrap().label, "PR #481");
        assert!(vm.meta.iter().any(|m| m.text == "GitHub"));
        assert_eq!(
            vm.actions.iter().map(|a| a.kind).collect::<Vec<_>>(),
            [CardActionKind::MarkMerged, CardActionKind::MarkClosed]
        );

        t.status = Status::ReadyForReview;
        t.column = t.status.column();
        t.flags.pr_rework = true;
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(
            vm.callout.unwrap().text,
            "Back for rework · Send back to start the agent"
        );
        assert!(vm.actions.is_empty(), "no Review / Push until the fix");
        assert!(vm.meta.iter().any(|m| m.text == "GitHub"), "the PR stays");

        t.flags.pr_rework = false;
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(vm.actions[0].kind, CardActionKind::PushToPr);
        assert!(vm.actions[0].primary);
    }

    #[test]
    fn pushed_content_that_differs_shows_on_the_card() {
        let mut t = ticket("SHOP-26", "Checkout totals", Status::PrOpen);
        t.flags.pr_number = Some(481);
        t.flags.changed_since_review = true;
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(vm.pill.unwrap().label, "PR #481");
        assert_eq!(
            vm.callout.unwrap().text,
            "Changed since you reviewed it · the pushed content differs"
        );
    }

    #[test]
    fn priority_order() {
        let mut t = ticket("SHOP-19", "Price table", Status::Working);
        t.flags.last_signal_at = Some(NOW - 6 * 60_000);
        t.flags.conflicts_with = Some("main".into());
        t.flags.pr_number = Some(481);
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(
            vm.pill.unwrap().label,
            "Quiet 6 min",
            "stale beats conflicts and PR"
        );

        t.flags.last_signal_at = Some(NOW - 40_000);
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(vm.pill.unwrap().label, "Conflicts with main");

        let mut pr = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
        pr.flags.pr_number = Some(481);
        let vm = CardViewModel::from(&pr, &main_project(), NOW);
        assert_eq!(vm.pill.unwrap().label, "PR #481");
        assert_eq!(vm.actions.len(), 2);

        let interrupted = CardViewModel::from(
            &ticket("SHOP-25", "Upgrade", Status::Interrupted),
            &main_project(),
            NOW,
        );
        assert_eq!(interrupted.pill.unwrap().label, "Interrupted");
        assert_eq!(interrupted.actions[0].kind, CardActionKind::Resume);
        // An interrupted question keeps its label and is answered, not resumed, from the card.
        let mut asked = ticket("SHOP-23", "Split user full name", Status::Interrupted);
        asked.interrupted_from = Some(Status::AsksYou);
        let card = CardViewModel::from(&asked, &CardExtras::default(), 0);
        assert_eq!(card.pill.unwrap().label, "Asks you");
        assert_eq!(card.outline, Outline::Attention);
        assert_eq!(
            card.callout.unwrap().text,
            "Session ended by restart · answering resumes it"
        );
        assert!(card.actions.is_empty());
        let failed = CardViewModel::from(
            &ticket("BILL-4", "Invoice PDF export", Status::ResumeFailed),
            &CardExtras::default(),
            0,
        );
        assert_eq!(failed.actions[0].kind, CardActionKind::RetryResume);
        assert!(!failed.actions[0].primary);
        assert_eq!(failed.actions[1].kind, CardActionKind::StartFromScratch);
        assert!(failed.actions[1].primary);

        let mut asks = ticket("SHOP-23", "Split user full name", Status::AsksYou);
        asks.flags.conflicts_with = Some("main".into());
        let vm = CardViewModel::from(&asks, &main_project(), NOW);
        assert_eq!(
            vm.pill.unwrap().label,
            "Asks you",
            "orange beats everything"
        );
    }

    #[test]
    fn pinned_is_never_a_pill() {
        let mut t = ticket("SHOP-19", "Price table", Status::Working);
        t.flags.pinned_at = Some(1);
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert!(vm.pinned);
        assert_eq!(vm.pill, None);
        t.status = Status::Idle;
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert!(vm.pinned);
        assert_eq!(vm.pill.unwrap().label, "Idle");
    }

    /// *04 → Row · priority & Ready for Dev*: P1, P2 and P4 sit after the key, Normal shows nothing, and the
    /// priority never takes the pill.
    #[test]
    fn priority_chip_per_priority() {
        let mut t = ticket("SHOP-31", "Hotfix: checkout crash", Status::Backlog);
        let shown = |t: &Ticket| CardViewModel::from(t, &main_project(), NOW).priority;
        assert_eq!(shown(&t), None, "P3 Normal is hidden on cards");
        for p in [Priority::Urgent, Priority::High, Priority::Low] {
            t.priority = p;
            assert_eq!(shown(&t), Some(p));
            assert_eq!(CardViewModel::from(&t, &main_project(), NOW).pill, None);
        }
        // Pinned beats priority: a pinned P3 shows the pin and no chip; a P1 next to it shows its chip.
        let mut pinned = ticket("SHOP-47", "Gift wrap option", Status::Queued);
        pinned.flags.pinned_at = Some(1);
        let vm = CardViewModel::from(&pinned, &main_project(), NOW);
        assert!(vm.pinned && vm.priority.is_none());
        pinned.priority = Priority::Urgent;
        let vm = CardViewModel::from(&pinned, &main_project(), NOW);
        assert_eq!((vm.pinned, vm.priority), (true, Some(Priority::Urgent)));
    }

    /// An epic child shows the priority it follows from its epic, the same way as its own.
    #[test]
    fn an_epic_child_shows_the_inherited_priority() {
        let mut child = ticket("SHOP-40.2", "Wishlist button", Status::Backlog);
        child.epic = Some("SHOP-40".parse().unwrap());
        child.priority = Priority::High;
        child.priority_inherited = true;
        let vm = CardViewModel::from(&child, &main_project(), NOW);
        assert_eq!(vm.priority, Some(Priority::High));
    }

    /// *04 · Backlog · P4 · Ready to start*: an unblocked Backlog ticket shows the quiet hint (d0066).
    #[test]
    fn ready_to_start_entry() {
        let mut t = ticket("SHOP-24", "Update footer links", Status::Backlog);
        assert_eq!(CardViewModel::from(&t, &main_project(), NOW).entry, None);
        t.flags.ready_to_start = true;
        let entry = CardViewModel::from(&t, &main_project(), NOW).entry.unwrap();
        assert_eq!(
            (entry.icon, entry.text.as_str(), entry.tone),
            (IconName::CirclePlay, READY_TO_START, Tone::Muted)
        );
    }

    #[test]
    fn meta_row_details() {
        // An empty description shows no hint (d0067).
        let t = ticket("SHOP-24", "Update footer links", Status::Backlog);
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert!(vm.meta.is_empty());

        let mut hotfix = ticket("SHOP-31", "Hotfix: checkout crash", Status::Backlog);
        hotfix.base = Base::Branch("release/2.3".into());
        let vm = CardViewModel::from(&hotfix, &main_project(), NOW);
        assert_eq!(vm.meta.last().unwrap().text, "on release/2.3");

        let mut working = ticket("BILL-4", "Invoice PDF export", Status::Working);
        working.flags.last_signal_at = Some(NOW - 4 * 60_000);
        working.flags.tokens = Some(205_000);
        let vm = CardViewModel::from(&working, &main_project(), NOW);
        assert_eq!(vm.meta[0].text, "4m");
        assert_eq!(vm.meta[0].tone, Tone::Warning, "amber after 3 min");
        assert_eq!(vm.meta[1].text, "205k");
        assert_eq!(vm.pill, None, "the pill waits until 5 min");
    }

    #[test]
    fn labels() {
        assert_eq!(tokens_label(182_400), "182k");
        assert_eq!(tokens_label(1_250_000), "1.25M");
        assert_eq!(tokens_label(1_840_000), "1.84M");
        assert_eq!(tokens_label(12_400_000), "12.4M");
        assert_eq!(tokens_label(950), "950");
        assert_eq!(age_label(40_000), "40s");
        assert_eq!(age_label(3 * 60_000), "3m");
        assert_eq!(age_label(2 * 3_600_000), "2h");
        let keys: Vec<String> = ["SHOP-17", "SHOP-18", "SHOP-19"].map(String::from).to_vec();
        assert_eq!(deps_label(&keys, 2), "SHOP-17 SHOP-18 +1");
        assert_eq!(deps_label(&keys[..2], 2), "SHOP-17 SHOP-18");
    }

    #[test]
    fn done_cards_fade_and_parked_cards_explain() {
        let merged = CardViewModel::from(
            &ticket("SHOP-17", "Add Stripe client", Status::Merged),
            &main_project(),
            NOW,
        );
        assert_eq!(merged.faded, Some(0.55));
        let parked = CardViewModel::from(
            &ticket("SHOP-25", "Upgrade", Status::Parked),
            &main_project(),
            NOW,
        );
        assert_eq!(parked.pill.unwrap().label, "Parked · has work");
        assert!(parked.callout.unwrap().text.contains("conversation kept"));
        let mut moved = ticket("SHOP-40.4", "Wishlist share link", Status::Backlog);
        moved.epic = Some("SHOP-40".parse().unwrap());
        let x = CardExtras {
            old_key: Some("SHOP-51".parse().unwrap()),
            epic_title: Some("Wishlist".into()),
            ..main_project()
        };
        let vm = CardViewModel::from(&moved, &x, NOW);
        assert_eq!(vm.key_label, "SHOP-51 → SHOP-40.4");
        assert_eq!(vm.epic.as_deref(), Some("Wishlist"));
        assert_eq!(
            vm.callout.unwrap().text,
            "Moved into epic SHOP-40 · old key SHOP-51 still works"
        );
    }

    #[test]
    fn start_interrupted_names_the_failed_step_and_offers_retry_start() {
        let t = crate::view_models::card_fixtures::ticket(
            "SHOP-28",
            "Cart badge count",
            Status::StartInterrupted,
        );
        let plain = CardViewModel::from(&t, &CardExtras::default(), 0);
        assert_eq!(
            plain.callout.as_ref().unwrap().text,
            "Start didn’t finish · agent never started"
        );
        let extras = CardExtras {
            start_failure: Some("setup command failed".into()),
            ..CardExtras::default()
        };
        let vm = CardViewModel::from(&t, &extras, 0);
        assert_eq!(
            vm.callout.as_ref().unwrap().text,
            "Setup command failed · agent never started"
        );
        assert_eq!(vm.actions.len(), 1);
        assert_eq!(vm.actions[0].kind, CardActionKind::RetryStart);
        assert_eq!(vm.actions[0].label, "Retry start");
    }

    // T-0192: *04 → Row · validation & Auto-merge (0063, 0064)* and *Row · validation entries & Auto-merge
    // outcomes (0063, 0064)*, one test per variant, from the gallery's fixtures.

    fn m12(name: &str) -> CardViewModel {
        let v = crate::view_models::card_fixtures::m12_variants()
            .into_iter()
            .find(|v| v.name == name)
            .unwrap_or_else(|| panic!("no fixture {name}"));
        CardViewModel::from(&v.ticket, &v.extras, NOW)
    }

    fn pill_of(vm: &CardViewModel) -> Option<(Legend, &str)> {
        vm.pill.as_ref().map(|p| (p.legend, p.label.as_str()))
    }

    fn callout_of(vm: &CardViewModel) -> (IconName, Tone, &str) {
        let c = vm.callout.as_ref().expect("a callout");
        (c.icon, c.tone, c.text.as_str())
    }

    fn entry_of(vm: &CardViewModel) -> (IconName, Tone, Tone, &str) {
        let e = vm.entry.as_ref().expect("an entry");
        (e.icon, e.tone, e.text_tone, e.text.as_str())
    }

    fn actions_of(vm: &CardViewModel) -> Vec<(CardActionKind, &str, bool)> {
        vm.actions
            .iter()
            .map(|a| (a.kind, a.label, a.primary))
            .collect()
    }

    fn has_signal(vm: &CardViewModel) -> bool {
        vm.meta.iter().any(|m| m.icon == IconName::Activity)
    }

    #[test]
    fn m12_fixtures_cover_both_rows_in_order() {
        let names: Vec<&str> = crate::view_models::card_fixtures::m12_variants()
            .iter()
            .map(|v| v.name)
            .collect();
        assert_eq!(
            names,
            [
                "Dev · Validating…",
                "Working · validation failed · round 2 of 3",
                "Dev · Needs you · validator",
                "Dev · Validation failed 3 times",
                "Dev · Validation ended without a result",
                "Dev · Merge conflict · fixing",
                "Dev · Keeps conflicting",
                "Review · Validated · round 2",
                "Review · Validation skipped by you",
                "Review · Submitted by you",
                "Review · Failed 3 times · moved by you",
                "Done · Merged automatically",
            ]
        );
    }

    #[test]
    fn validating() {
        let vm = m12("Dev · Validating…");
        assert_eq!(pill_of(&vm), Some((Legend::Validating, "Validating…")));
        assert_eq!(Legend::Validating.spec().2, IconName::ShieldEllipsis);
        assert_eq!(
            vm.bars,
            [Bar::Validator("validator · round 1 of 3 · 2 min".into())]
        );
        assert_eq!(vm.callout, None);
        assert_eq!(vm.outline, Outline::Normal, "calm, not orange");
        assert!(!has_signal(&vm), "the agent isn't what it waits on");
        assert_eq!(vm.meta.last().unwrap().text, "67k");
        assert!(vm.actions.is_empty());
    }

    #[test]
    fn working_after_a_failed_round() {
        let vm = m12("Working · validation failed · round 2 of 3");
        assert_eq!(vm.pill, None, "a marker on a working card uses the callout");
        assert_eq!(
            callout_of(&vm),
            (
                IconName::ShieldAlert,
                Tone::Warning,
                "Validation failed · round 2 of 3 · 2 findings"
            )
        );
        assert_eq!(vm.outline, Outline::Normal);
        assert!(has_signal(&vm));
    }

    #[test]
    fn needs_you_validator() {
        let vm = m12("Dev · Needs you · validator");
        assert_eq!(
            pill_of(&vm),
            Some((Legend::ValidatorNeedsYou, "Needs you · validator"))
        );
        assert_eq!(Legend::ValidatorNeedsYou.spec().2, IconName::Hand);
        assert_eq!(
            callout_of(&vm),
            (
                IconName::Shield,
                Tone::Attention,
                "validator: Allow Bash(pnpm test)?"
            )
        );
        assert!(vm.callout.as_ref().unwrap().mono);
        assert_eq!(vm.outline, Outline::Attention);
        assert!(vm.bars.is_empty());
        assert!(vm.actions.is_empty());
    }

    #[test]
    fn validation_failed_3_times() {
        let vm = m12("Dev · Validation failed 3 times");
        assert_eq!(
            pill_of(&vm),
            Some((Legend::ValidationFailed, "Failed 3 times"))
        );
        assert_eq!(Legend::ValidationFailed.spec().2, IconName::ShieldX);
        assert_eq!(
            callout_of(&vm),
            (
                IconName::ShieldX,
                Tone::Attention,
                "The description asks for keyboard navigation; arrow keys do nothing"
            )
        );
        assert_eq!(vm.outline, Outline::Attention);
        assert!(!has_signal(&vm));
        // Without a report, the count.
        let mut t = ticket("SHOP-32", "Address autocomplete", Status::ValidationFailed);
        t.validation_round = 1;
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(vm.pill.unwrap().label, "Failed 1 time");
        assert_eq!(vm.callout.unwrap().text, "Validation failed 1 time");
    }

    #[test]
    fn validation_ended_without_a_result() {
        let vm = m12("Dev · Validation ended without a result");
        assert_eq!(
            pill_of(&vm),
            Some((Legend::ValidationNoResult, "No result"))
        );
        assert_eq!(
            Legend::ValidationNoResult.spec().2,
            IconName::ShieldQuestionMark
        );
        assert_eq!(
            callout_of(&vm),
            (
                IconName::ShieldQuestionMark,
                Tone::Attention,
                "Validation ended without a result"
            )
        );
        assert_eq!(
            actions_of(&vm),
            [(CardActionKind::RetryValidation, "Retry validation", true)]
        );
        assert_eq!(vm.actions[0].icon, Some(IconName::RotateCcw));
        assert_eq!(
            CardActionKind::RetryValidation.validation_action(),
            Some(crewboard_core::domain::ValidationAction::Retry)
        );
        // A discarded verdict says why (VAL-02).
        let mut t = ticket("SHOP-34", "Tax display", Status::ValidationNoResult);
        t.signal = Some(crewboard_core::board::tickets::AgentSignal {
            text: Some(crewboard_core::domain::validation::DISCARDED.into()),
            since: None,
            wakeup_at: None,
        });
        let vm = from_signal(&t, NOW);
        assert_eq!(
            vm.callout.unwrap().text,
            "Validation discarded: the validator changed files"
        );
    }

    #[test]
    fn merge_conflict_fixing() {
        let vm = m12("Dev · Merge conflict · fixing");
        assert_eq!(
            vm.pill, None,
            "not \"Conflicts with …\": the marker is in the callout"
        );
        assert_eq!(
            callout_of(&vm),
            (
                IconName::GitCompareArrows,
                Tone::Warning,
                "Merge conflict · fixing · lib/wishlist.ts"
            )
        );
        assert_eq!(vm.outline, Outline::Normal);
        assert_eq!(vm.epic.as_deref(), Some("Wishlist"));
        assert!(has_signal(&vm));
        let mut t = ticket(
            "SHOP-40.4",
            "Wishlist share link",
            Status::MergeConflictFixing,
        );
        t.flags.conflict_files = vec!["a.ts".into(), "b.ts".into(), "c.ts".into()];
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(
            vm.callout.unwrap().text,
            "Merge conflict · fixing · a.ts +2"
        );
    }

    #[test]
    fn keeps_conflicting() {
        let vm = m12("Dev · Keeps conflicting");
        assert_eq!(
            pill_of(&vm),
            Some((Legend::AutoMergeStopped, "Keeps conflicting"))
        );
        assert_eq!(
            Legend::AutoMergeStopped.spec().2,
            IconName::GitCompareArrows
        );
        let text = format!(
            "Keeps conflicting with {} · 3 tries",
            crewboard_core::naming::branch("SHOP-40")
        );
        assert_eq!(
            callout_of(&vm),
            (IconName::GitCompareArrows, Tone::Attention, text.as_str())
        );
        assert_eq!(vm.outline, Outline::Attention);
        assert_eq!(
            actions_of(&vm),
            [
                (CardActionKind::RetryMerge, "Retry merge", true),
                (CardActionKind::MoveToReview, "Move to Review", false),
            ]
        );
        assert_eq!(vm.actions[1].icon, None);
        // The other stops name themselves and show the Local merge's message.
        use crewboard_core::domain::AutoMergeStop;
        for (stop, label) in [
            (AutoMergeStop::BaseDiverged, "Base diverged"),
            (AutoMergeStop::CheckoutDirty, "Checkout dirty"),
        ] {
            let mut t = ticket("BILL-6", "Late fee rule", Status::AutoMergeStopped);
            t.auto_merge_stop = Some(stop);
            let mut extras = main_project();
            extras.question = Some("the message".into());
            let vm = CardViewModel::from(&t, &extras, NOW);
            assert_eq!(vm.pill.unwrap().label, label);
            assert_eq!(vm.callout.unwrap().text, "the message");
            assert_eq!(vm.actions.len(), 2);
        }
    }

    #[test]
    fn review_entries() {
        let cases = [
            (
                "Review · Validated · round 2",
                (
                    IconName::ShieldCheck,
                    Tone::Review,
                    Tone::Secondary,
                    "Validated · round 2",
                ),
            ),
            (
                "Review · Validation skipped by you",
                (
                    IconName::ShieldOff,
                    Tone::Muted,
                    Tone::Secondary,
                    "Validation skipped by you",
                ),
            ),
            (
                "Review · Submitted by you",
                (
                    IconName::UserCheck,
                    Tone::Muted,
                    Tone::Secondary,
                    "Submitted by you · not validated",
                ),
            ),
            (
                "Review · Failed 3 times · moved by you",
                (
                    IconName::ShieldX,
                    Tone::Error,
                    Tone::Error,
                    "Did not pass · failed 3 times · moved by you",
                ),
            ),
        ];
        for (name, entry) in cases {
            let vm = m12(name);
            assert_eq!(entry_of(&vm), entry, "{name}");
            assert_eq!(vm.pill, None, "{name}: Review's normal status");
            assert_eq!(vm.callout, None, "{name}: one line says how it got here");
            assert_eq!(vm.actions[0].kind, CardActionKind::Review, "{name}");
            assert_eq!(vm.meta[0].text, "+412 −38", "{name}");
        }
        // A note from Submit for review keeps its callout (d0052).
        let mut t = ticket("SHOP-18", "Checkout API", Status::ReadyForReview);
        t.flags.review_entry = Some(crewboard_core::domain::ReviewEntryFlag {
            entry: crewboard_core::domain::ReviewEntry::SubmittedByUser,
            round: 0,
        });
        t.flags.submitted_by_you = Some("Paging works, tests pending".into());
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(
            vm.callout.unwrap().text,
            "Submitted by you · \u{201c}Paging works, tests pending\u{201d}"
        );
        assert_eq!(vm.entry.unwrap().text, "Submitted by you · not validated");
    }

    #[test]
    fn merged_automatically() {
        let vm = m12("Done · Merged automatically");
        assert_eq!(pill_of(&vm), Some((Legend::Merged, "Merged")));
        assert_eq!(
            entry_of(&vm),
            (
                IconName::Bot,
                Tone::Muted,
                Tone::Muted,
                "Merged automatically"
            ),
            "wins over the Review entry"
        );
        assert_eq!(vm.faded, Some(0.55));
        assert_eq!(vm.epic.as_deref(), Some("Wishlist"));
    }

    /// *Which flag wins the pill slot* with the M12 states (*Note · M12*).
    #[test]
    fn m12_pill_rank() {
        let vm_of = |t: &Ticket| CardViewModel::from(t, &main_project(), NOW);
        // 1: the orange validation and Auto-merge stops beat Interrupted-like flags, staleness, Asked, conflicts.
        for status in [
            Status::ValidatorNeedsYou,
            Status::ValidationFailed,
            Status::ValidationNoResult,
            Status::AutoMergeStopped,
        ] {
            let mut t = ticket("SHOP-32", "Address autocomplete", status);
            t.flags.asked = Some("SHOP-40.1".parse().unwrap());
            t.flags.conflicts_with = Some("main".into());
            t.flags.last_signal_at = Some(NOW - 20 * 60_000);
            let vm = vm_of(&t);
            assert_eq!(
                vm.pill.as_ref().unwrap().legend.spec().0,
                crate::theme::Tier::NeedsYou,
                "{status}"
            );
            assert_eq!(vm.outline, Outline::Attention, "{status}");
            assert!(
                vm.meta
                    .iter()
                    .any(|m| m.text.starts_with("asked SHOP-40.1")),
                "{status}: the lower flag moves to the meta row"
            );
        }
        // 5: Validating… is calm and beats stale, Asked and conflicts like Working in background.
        let mut t = ticket("SHOP-29", "Coupon field", Status::Validating);
        t.flags.asked = Some("SHOP-40.1".parse().unwrap());
        t.flags.conflicts_with = Some("main".into());
        let vm = vm_of(&t);
        assert_eq!(vm.pill.unwrap().label, "Validating…");
        assert!(
            vm.meta
                .iter()
                .any(|m| m.text.starts_with("asked SHOP-40.1"))
        );
        // Below Waiting for limit reset (4): a validator stopped by the plan limit shows that.
        let limit = ticket("SHOP-29", "Coupon field", Status::WaitingForLimitReset);
        assert_eq!(vm_of(&limit).pill.unwrap().label, "Waiting for limit reset");
        // The round marker ranks like Sent back: the callout, never the pill; the pill stays free for a higher
        // flag such as Asked (7).
        let mut t = ticket("SHOP-30", "Order history page", Status::Working);
        t.flags.validation_round =
            Some(crewboard_core::domain::ValidationRound { round: 1, of: 3 });
        let vm = vm_of(&t);
        assert_eq!(vm.pill, None);
        assert_eq!(
            vm.callout.as_ref().unwrap().text,
            "Validation failed · round 1 of 3"
        );
        t.flags.asked = Some("SHOP-40.1".parse().unwrap());
        assert_eq!(vm_of(&t).pill.unwrap().label, "Asked SHOP-40.1");
        // Conflicts with the base still wins its callout over the marker (rank 8 above the markers).
        t.flags.asked = None;
        t.flags.conflicts_with = Some("main".into());
        assert_eq!(vm_of(&t).pill.unwrap().label, "Conflicts with main");
        // Merge conflict · fixing never shows "Conflicts with …" in the pill; Asked still may.
        let mut t = ticket(
            "SHOP-40.4",
            "Wishlist share link",
            Status::MergeConflictFixing,
        );
        t.flags.conflicts_with = Some(crewboard_core::naming::branch("SHOP-40"));
        assert_eq!(vm_of(&t).pill, None);
        t.flags.asked = Some("SHOP-40.1".parse().unwrap());
        assert_eq!(vm_of(&t).pill.unwrap().label, "Asked SHOP-40.1");
    }
}
