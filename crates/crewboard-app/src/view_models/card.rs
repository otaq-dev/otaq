//! What a ticket card shows (*crewboard.pen → 04 · Ticket card variants*, UI-03). A card answers four
//! questions at a glance: what is it (key + title), is it OK (the pill, empty when the status is the column's
//! normal one), does it need me (orange border + callout), what is it waiting on (the meta row).
//!
//! **One pill, one callout** (*Which flag wins the pill slot*): the highest flag takes the pill, lower flags
//! fall back to the meta row, so nothing is lost:
//! 1 orange (Needs you · Asks you · Idle) · 2 Interrupted · Start interrupted (· Resume failed) · 3 Parent
//! closed · 4 Waiting for limit reset · 5 Working in background · 6 stale signal (quiet 5+ min) · 8 conflicts
//! with the base · 9 PR open · 10 Parked · has work · 11 none. Rank 7 ("asked another agent", MSG-06) is
//! added by T-0154. **Pinned is not a status**: a pin icon sits next to the key and never takes the pill.
//!
//! Data that arrives in later milestones (questions M4, waits-for text M5, review flags M6, usage M8, Run M9,
//! PR M10) comes in through [`CardExtras`] and [`crewboard_core::domain::Flags`]; fixtures fill it until then.

use crewboard_core::board::tickets::{Base, Ticket};
use crewboard_core::domain::{Column, Status, TicketKey};

use crate::icons::IconName;
use crate::theme::Status as Legend;
use crate::view_models::status;

/// The last signal turns amber after this long (*Badges & pills → Last signal*).
pub const SIGNAL_AMBER_MS: i64 = 3 * 60 * 1000;
/// … and the card's pill says "Quiet N min" after this long (brief 04; SDD §26 implementation choice).
pub const SIGNAL_STALE_MS: i64 = 5 * 60 * 1000;

/// Facts about a ticket that aren't on [`Ticket`] yet or come from elsewhere on the board.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardExtras {
    /// The project's base branch; a ticket on another base says so ("on release/2.3").
    pub project_base: Option<String>,
    /// The epic's title, for the epic chip on its children.
    pub epic_title: Option<String>,
    /// Queued: what it waits for ("waits for SHOP-20 to merge", T-0077).
    pub waits_for: Option<String>,
    /// The question or prompt of an orange ticket.
    pub question: Option<String>,
    /// The prompt comes from a helper agent inside the session.
    pub helper_agent: bool,
    /// Parent closed: the parent's key.
    pub closed_parent: Option<TicketKey>,
    /// Held: why ("`develop` and `origin/develop` have diverged. Push or pull, then retry.").
    pub held_reason: Option<String>,
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
    Resume,
    RetryStart,
    Review,
    MarkMerged,
    MarkClosed,
    Reopen,
    Retry,
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
    pub bars: Vec<Bar>,
    /// The epic chip ("Wishlist").
    pub epic: Option<String>,
    pub meta: Vec<MetaItem>,
    pub outline: Outline,
    pub actions: Vec<CardAction>,
    /// Done cards fade back: (column-coloured fill, opacity).
    pub faded: Option<f32>,
}

/// "setup command failed" → "Setup command failed".
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// "182k", "1.2M", "950".
pub fn tokens_label(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.)
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
        Column::Dev => &[Status::Working, Status::Queued],
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
        let status = ticket.status;
        let flags = &ticket.flags;
        let quiet_ms = flags.last_signal_at.map(|at| now_ms - at);
        let stale = is_working(status) && quiet_ms.is_some_and(|q| q >= SIGNAL_STALE_MS);

        // The pill: the highest flag, else the status when it isn't the column's normal one.
        let status_pill = || {
            let (legend, label) = status::pill(status);
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
        } else {
            None
        };

        let callout = callout(ticket, extras);

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

        let meta = meta_row(ticket, extras, quiet_ms);
        let outline = if status.is_orange() {
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
        let key_label = match &extras.old_key {
            Some(old) => format!("{old} → {}", ticket.key),
            None => ticket.key.to_string(),
        };
        Self {
            key: ticket.key.clone(),
            key_label,
            pinned: flags.is_pinned(),
            title: ticket.title.clone(),
            pill,
            callout,
            bars,
            epic: extras.epic_title.clone(),
            meta,
            outline,
            actions: actions(status, flags.pr_number.is_some(), extras),
            faded,
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
        Status::Held => c(
            IconName::GitCompareArrows,
            Tone::Warning,
            x.held_reason.clone().unwrap_or_else(|| "Held".into()),
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
    let item = |icon: IconName, text: String, tone: Tone| MetaItem { icon, text, tone };
    let flags = &ticket.flags;
    let mut meta = Vec::new();
    if !ticket.depends_on.is_empty() {
        let keys: Vec<String> = ticket.depends_on.iter().map(ToString::to_string).collect();
        meta.push(item(IconName::Link2, deps_label(&keys, 2), Tone::Muted));
    }
    if let Some(w) = &x.waits_for {
        meta.push(item(IconName::Hourglass, w.clone(), Tone::Muted));
    }
    if ticket.status == Status::Backlog && !ticket.is_epic() {
        if ticket.done_when.is_empty() {
            meta.push(item(
                IconName::ListChecks,
                "no “Done when”".into(),
                Tone::Warning,
            ));
        } else {
            let n = ticket.done_when.len();
            meta.push(item(
                IconName::ListChecks,
                format!("{n} criteri{}", if n == 1 { "on" } else { "a" }),
                Tone::Muted,
            ));
        }
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
    if let Some(provider) = &x.pr_provider {
        meta.push(item(
            IconName::GitPullRequest,
            provider.clone(),
            Tone::Muted,
        ));
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
    }
    if let Some(port) = flags.run_port {
        meta.push(item(IconName::Play, format!(":{port}"), Tone::Running));
    }
    meta
}

/// Dependency badges: the first `shown` keys, then "+N" (T-0037).
pub fn deps_label(keys: &[String], shown: usize) -> String {
    if keys.len() <= shown {
        keys.join(" ")
    } else {
        format!("{} +{}", keys[..shown].join(" "), keys.len() - shown)
    }
}

fn actions(status: Status, has_pr: bool, x: &CardExtras) -> Vec<CardAction> {
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
        Status::StartInterrupted => vec![a(
            CardActionKind::RetryStart,
            "Retry start",
            Some(IconName::RotateCcw),
            true,
        )],
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
        Status::Held if x.held_reason.is_some() => vec![a(
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

    #[test]
    fn meta_row_details() {
        let mut t = ticket("SHOP-24", "Update footer links", Status::Backlog);
        t.done_when.clear();
        let vm = CardViewModel::from(&t, &main_project(), NOW);
        assert_eq!(vm.meta[0].text, "no “Done when”");
        assert_eq!(vm.meta[0].tone, Tone::Warning);

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
        assert_eq!(tokens_label(1_250_000), "1.2M");
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
