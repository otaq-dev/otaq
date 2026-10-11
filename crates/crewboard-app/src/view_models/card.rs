//! What a ticket card shows (*crewboard.pen → 04 · Ticket card variants*, UI-03). A card answers four
//! questions at a glance: what is it (key + title), is it OK (the pill, empty when the status is the column's
//! normal one), does it need me (orange border + callout), what is it waiting on (the meta row).
//!
//! **One pill, one callout** (*Which flag wins the pill slot*): the highest flag takes the pill, lower flags
//! fall back to the meta row, so nothing is lost:
//! 1 orange (Needs you · Asks you · Idle) · 2 Interrupted · Start interrupted (· Resume failed) · 3 Parent
//! closed · 4 Waiting for limit reset · 5 Working in background · 6 stale signal (quiet 5+ min) · 8 conflicts
//! with the base · 9 PR open · 10 Parked · has work · 11 none. Rank 7 is "asked another agent" (MSG-06): "Asked
//! SHOP-40.1" in the pill, else "asked SHOP-40.1 · waiting" in the meta row. **Pinned is not a status**: a pin icon
//! sits next to the key and never takes the pill.
//!
//! Data that arrives in later milestones (questions M4, waits-for text M5, review flags M6, usage M8, Run M9,
//! PR M10) comes in through [`CardExtras`] and [`crewboard_core::domain::Flags`]; fixtures fill it until then.

use crewboard_core::board::tickets::{Base, Ticket};
use crewboard_core::domain::{Column, Status, TicketKey};
use crewboard_core::signals::background::{minutes, wakeup_text};
use crewboard_core::signals::{HELPER_PROMPT_PREFIX, is_stale};

use crate::icons::IconName;
use crate::theme::Status as Legend;
use crate::view_models::status;

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

/// The tone of a callout or a meta item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pill {
    pub legend: Legend,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callout {
    pub icon: IconName,
    pub tone: Tone,
    pub text: String,
    /// Mono text (a command or prompt).
    pub mono: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaItem {
    pub icon: IconName,
    pub text: String,
    pub tone: Tone,
    /// Shown on hover ("—" tokens say why).
    pub tooltip: Option<&'static str>,
}

/// A quiet line under the title, the frame's "Entry": an icon and a short text ("Ready to start").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub icon: IconName,
    pub text: String,
    pub tone: Tone,
}

/// A bar under the title: messages waiting to be typed, or background work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bar {
    MessagesWaiting(u32),
    Background { text: String, wakeup: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardAction {
    pub kind: CardActionKind,
    pub label: &'static str,
    pub icon: Option<IconName>,
    pub primary: bool,
}

/// The card's outline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
                label: label.to_owned(),
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
                    | Status::WorkingInBackground
            ) {
            Some(status_pill())
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
        // An unblocked Backlog ticket: a hint only, it never starts by itself (d0066).
        let entry = flags.ready_to_start.then(|| Entry {
            icon: IconName::CirclePlay,
            text: READY_TO_START.to_owned(),
            tone: Tone::Muted,
        });
        Self {
            key: ticket.key.clone(),
            key_label,
            pinned: flags.is_pinned(),
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
        }
    }
}

fn callout(ticket: &Ticket, x: &CardExtras) -> Option<Callout> {
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
            } else if let Some(note) = &flags.submitted_by_you {
                c(
                    IconName::UserCheck,
                    Tone::Review,
                    crate::view_models::close::submitted_line(note),
                )
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
            Status::Merged | Status::ClosedNotMerged | Status::WaitingForLimitReset
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
        assert!(crate::board::ticks(&t));
        let done = ticket("SHOP-17", "Add Stripe client", Status::Merged);
        assert!(!crate::board::ticks(&done));
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
}
