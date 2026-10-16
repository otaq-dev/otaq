//! The next-item hotkey (ATT-07; d0016, d0058; *10 · Needs-you inbox → 1 · ⌘J from anywhere, 2 · Answered, 3 · Next
//! item, 4 · All clear*): ⌘J opens the next inbox item with its terminal focused, from anywhere. A run starts at the
//! first ⌘J and shows "Inbox · 1 of 7" on top of the panel. It **never advances by itself**: once the open item
//! is answered the bar says "Done · 6 left" and the ticket stays open; the next ⌘J moves on. When nothing is left,
//! "All clear" shows for a moment and the run ends.

use std::time::{Duration, Instant};

use otaq_core::board::needs_you::NeedsYouItem;
use otaq_core::domain::TicketKey;

/// How long "All clear" stays.
pub const ALL_CLEAR_FOR: Duration = Duration::from_secs(3);

/// A ⌘J run through the inbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxRun {
    started: Instant,
    /// Items handled since the run started (they left the inbox).
    handled: Vec<TicketKey>,
    /// The item open now.
    current: Option<TicketKey>,
    /// Shown "All clear" since then.
    clear_since: Option<Instant>,
}

/// What the bar on top of the panel shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunBar {
    /// "Inbox · 2 of 7", with one segment per item (`done` of them filled).
    Step { n: usize, of: usize, done: usize },
    /// The open item was answered: "Done · 6 left" and "next: ⌘J".
    Done { left: usize, of: usize },
    /// Nothing left: "All clear · 7 items handled in 14 min."
    AllClear { handled: usize, minutes: u64 },
}

/// What ⌘J does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextStep {
    /// Open this item's panel with its terminal focused.
    Open(Box<NeedsYouItem>),
    /// Nothing left: show "All clear".
    AllClear,
}

impl InboxRun {
    pub fn new(now: Instant) -> Self {
        Self {
            started: now,
            handled: Vec::new(),
            current: None,
            clear_since: None,
        }
    }

    pub fn current(&self) -> Option<&TicketKey> {
        self.current.as_ref()
    }

    /// ⌘J: the item after the open one in inbox order (the first one if the open one is done).
    pub fn next(&mut self, items: &[NeedsYouItem], now: Instant) -> NextStep {
        self.note_handled(items);
        let at = self
            .current
            .as_ref()
            .and_then(|k| items.iter().position(|i| &i.key == k));
        let next = match at {
            Some(i) => items.get(i + 1).or_else(|| items.first()),
            None => items.first(),
        };
        match next {
            Some(item) => {
                self.current = Some(item.key.clone());
                self.clear_since = None;
                NextStep::Open(Box::new(item.clone()))
            }
            None => {
                self.current = None;
                self.clear_since = Some(now);
                NextStep::AllClear
            }
        }
    }

    /// The bar for the inbox as it is now.
    pub fn bar(&mut self, items: &[NeedsYouItem], now: Instant) -> RunBar {
        self.note_handled(items);
        let handled = self.handled.len();
        let of = handled + items.len();
        if self.clear_since.is_some() || (items.is_empty() && self.current.is_none()) {
            let minutes = now.duration_since(self.started).as_secs() / 60;
            return RunBar::AllClear { handled, minutes };
        }
        match self
            .current
            .as_ref()
            .and_then(|k| items.iter().position(|i| &i.key == k))
        {
            Some(i) => RunBar::Step {
                n: handled + i + 1,
                of,
                done: handled,
            },
            None => RunBar::Done {
                left: items.len(),
                of,
            },
        }
    }

    /// The run is over: "All clear" has been shown long enough.
    pub fn finished(&self, now: Instant) -> bool {
        self.clear_since
            .is_some_and(|at| now.duration_since(at) >= ALL_CLEAR_FOR)
    }

    /// The open item left the inbox: it counts as handled.
    fn note_handled(&mut self, items: &[NeedsYouItem]) {
        if let Some(current) = &self.current
            && !items.iter().any(|i| &i.key == current)
            && !self.handled.contains(current)
        {
            self.handled.push(current.clone());
        }
    }
}

/// "Inbox · 1 of 7", "Done · 6 left", "All clear".
pub fn bar_title(bar: &RunBar) -> String {
    match bar {
        RunBar::Step { n, of, .. } => format!("Inbox · {n} of {of}"),
        RunBar::Done { left, .. } => format!("Done · {left} left"),
        RunBar::AllClear { .. } => "All clear".to_owned(),
    }
}

/// "7 items handled in 14 min. 3 agents running; nothing needs you right now."
pub fn all_clear_text(handled: usize, minutes: u64, running: u32) -> String {
    let items = if handled == 1 { "item" } else { "items" };
    let agents = if running == 1 { "agent" } else { "agents" };
    format!(
        "{handled} {items} handled in {minutes} min. {running} {agents} running; nothing needs you right now."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use otaq_core::board::needs_you::NeedsYouGroup;
    use otaq_core::domain::Status;

    fn items(keys: &[&str]) -> Vec<NeedsYouItem> {
        keys.iter()
            .map(|k| NeedsYouItem {
                group: NeedsYouGroup::Family,
                project: "SHOP".into(),
                project_name: "shop-web".into(),
                id: (*k).into(),
                key: k.parse().unwrap(),
                title: String::new(),
                status: Status::NeedsYou,
                interrupted_from: None,
                question: None,
                reason: String::new(),
                since: 0,
                age_ms: 0,
                blocked: 0,
                message: None,
                suggestion: None,
            })
            .collect()
    }

    fn opened(step: NextStep) -> String {
        match step {
            NextStep::Open(item) => item.key.to_string(),
            NextStep::AllClear => "all clear".into(),
        }
    }

    #[test]
    fn steps_through_without_advancing_on_its_own() {
        let t0 = Instant::now();
        let all = items(&[
            "SHOP-22", "SHOP-23", "SHOP-12", "SHOP-25", "SHOP-28", "SHOP-18", "BILL-5",
        ]);
        let mut run = InboxRun::new(t0);
        assert_eq!(opened(run.next(&all, t0)), "SHOP-22");
        assert_eq!(
            run.bar(&all, t0),
            RunBar::Step {
                n: 1,
                of: 7,
                done: 0
            }
        );
        assert_eq!(bar_title(&run.bar(&all, t0)), "Inbox · 1 of 7");

        // SHOP-22 answered: the ticket stays open, the bar says so.
        let rest = &all[1..];
        assert_eq!(run.bar(rest, t0), RunBar::Done { left: 6, of: 7 });
        assert_eq!(
            run.current().unwrap().to_string(),
            "SHOP-22",
            "no auto-advance"
        );
        assert_eq!(bar_title(&run.bar(rest, t0)), "Done · 6 left");

        assert_eq!(opened(run.next(rest, t0)), "SHOP-23");
        assert_eq!(
            run.bar(rest, t0),
            RunBar::Step {
                n: 2,
                of: 7,
                done: 1
            }
        );
        // Not answered: ⌘J moves past it.
        assert_eq!(opened(run.next(rest, t0)), "SHOP-12");
        assert_eq!(
            run.bar(rest, t0),
            RunBar::Step {
                n: 3,
                of: 7,
                done: 1
            }
        );
    }

    #[test]
    fn all_clear_then_the_run_ends() {
        let t0 = Instant::now();
        let one = items(&["SHOP-22"]);
        let mut run = InboxRun::new(t0);
        run.next(&one, t0);
        let later = t0 + Duration::from_secs(14 * 60);
        assert_eq!(run.next(&[], later), NextStep::AllClear);
        assert_eq!(
            run.bar(&[], later),
            RunBar::AllClear {
                handled: 1,
                minutes: 14
            }
        );
        assert!(!run.finished(later + Duration::from_secs(1)));
        assert!(run.finished(later + ALL_CLEAR_FOR));
        assert_eq!(
            all_clear_text(7, 14, 3),
            "7 items handled in 14 min. 3 agents running; nothing needs you right now."
        );
    }

    #[test]
    fn the_last_unanswered_item_wraps_to_the_first() {
        let t0 = Instant::now();
        let two = items(&["SHOP-22", "SHOP-23"]);
        let mut run = InboxRun::new(t0);
        run.next(&two, t0);
        run.next(&two, t0);
        assert_eq!(opened(run.next(&two, t0)), "SHOP-22");
    }
}
