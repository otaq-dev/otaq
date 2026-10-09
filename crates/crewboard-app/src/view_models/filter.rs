//! Search and filter (UI-02, EPC-01; d0058 ⌘K; *03 · Board & columns → Search & filter · ⌘K*): one field does
//! both. Free text matches the key (old keys from Move to epic too), the title and the epic's title; tokens
//! filter by state (`is:needs-me`, `is:working`, …), by chain (`dep:SHOP-18`) or by epic (`epic:Wishlist`).
//! Everything combines (all must match). Non-matching cards hide; columns show "shown/total".

use crewboard_core::board::order::needs_user;
use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::{Column, Status, TicketKey};

use crate::view_models::chain::chain;

/// A state filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Is {
    /// Needs you: the orange family, Interrupted, Start interrupted, Resume failed, Parent closed, and review.
    NeedsMe,
    Working,
    Queued,
    Parked,
    /// A Run process is running.
    Running,
    Backlog,
    Review,
    Approved,
    Done,
}

impl Is {
    pub const ALL: &[Is] = &[
        Is::NeedsMe,
        Is::Working,
        Is::Queued,
        Is::Parked,
        Is::Running,
        Is::Backlog,
        Is::Review,
        Is::Approved,
        Is::Done,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::NeedsMe => "needs-me",
            Self::Working => "working",
            Self::Queued => "queued",
            Self::Parked => "parked",
            Self::Running => "running",
            Self::Backlog => "backlog",
            Self::Review => "review",
            Self::Approved => "approved",
            Self::Done => "done",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::NeedsMe => "needs you, asks, interrupted, review",
            Self::Working => "agent running now",
            Self::Queued => "waiting to start",
            Self::Parked => "parked in Backlog, has work",
            Self::Running => "a run process is running",
            Self::Backlog => "not started",
            Self::Review => "ready for your review",
            Self::Approved => "approved, PR open",
            Self::Done => "merged or closed",
        }
    }

    fn matches(self, t: &Ticket) -> bool {
        match self {
            Self::NeedsMe => needs_user(t.status) || t.status == Status::ReadyForReview,
            Self::Working => matches!(t.status, Status::Working | Status::WorkingInBackground),
            Self::Queued => t.status.is_queued(),
            Self::Parked => t.status == Status::Parked,
            Self::Running => t.flags.run_port.is_some(),
            Self::Backlog => t.column == Column::Backlog,
            Self::Review => t.column == Column::ReadyForReview,
            Self::Approved => t.column == Column::Approved,
            Self::Done => t.column == Column::Done,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Token {
    Is(Is),
    /// In this ticket's chain (parents and children, transitively).
    Dep(TicketKey),
    /// Children of the epic whose title or key starts with this (case-insensitive).
    Epic(String),
}

impl Token {
    /// Reads `is:needs-me`, `dep:SHOP-18`, `epic:Wishlist`.
    pub fn parse(word: &str) -> Option<Self> {
        let (kind, value) = word.split_once(':')?;
        match kind.to_lowercase().as_str() {
            "is" => Is::ALL
                .iter()
                .find(|i| i.name() == value.to_lowercase())
                .map(|i| Self::Is(*i)),
            "dep" => value.to_uppercase().parse().ok().map(Self::Dep),
            "epic" if !value.is_empty() => Some(Self::Epic(value.to_owned())),
            _ => None,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Is(i) => format!("is:{}", i.name()),
            Self::Dep(k) => format!("dep:{k}"),
            Self::Epic(e) => format!("epic:{e}"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    pub tokens: Vec<Token>,
    pub text: String,
}

impl Filter {
    pub fn is_active(&self) -> bool {
        !self.tokens.is_empty() || !self.text.trim().is_empty()
    }

    /// Takes the field's text: complete tokens (followed by a space) become chips, the rest stays text.
    pub fn take_input(&mut self, input: &str) -> String {
        let ends_with_space = input.ends_with(' ');
        let words: Vec<&str> = input.split_whitespace().collect();
        let mut rest = Vec::new();
        for (i, word) in words.iter().enumerate() {
            let complete = i + 1 < words.len() || ends_with_space;
            match Token::parse(word) {
                Some(token) if complete => self.add(token),
                _ => rest.push(*word),
            }
        }
        let mut text = rest.join(" ");
        if ends_with_space && !text.is_empty() {
            text.push(' ');
        }
        self.text = text.trim().to_owned();
        text
    }

    pub fn add(&mut self, token: Token) {
        if !self.tokens.contains(&token) {
            self.tokens.push(token);
        }
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.tokens.len() {
            self.tokens.remove(index);
        }
    }

    pub fn clear(&mut self) {
        self.tokens.clear();
        self.text.clear();
    }

    /// Whether a ticket stays visible. `all` is the project (for chains and epic titles).
    pub fn matches(&self, t: &Ticket, all: &[Ticket]) -> bool {
        let epic_title = |t: &Ticket| {
            t.epic
                .as_ref()
                .and_then(|e| all.iter().find(|x| &x.key == e))
        };
        let text = self.text.trim().to_lowercase();
        let text_ok = text.is_empty()
            || text.split_whitespace().all(|word| {
                t.key.to_string().to_lowercase().contains(word)
                    || t.aliases
                        .iter()
                        .any(|a| a.to_string().to_lowercase().contains(word))
                    || t.title.to_lowercase().contains(word)
                    || epic_title(t).is_some_and(|e| e.title.to_lowercase().contains(word))
            });
        text_ok
            && self.tokens.iter().all(|token| match token {
                Token::Is(i) => i.matches(t),
                Token::Dep(k) => chain(k, all).is_some_and(|c| c.members.contains(&t.key)),
                Token::Epic(name) => {
                    let name = name.to_lowercase();
                    epic_title(t).is_some_and(|e| {
                        e.title.to_lowercase().starts_with(&name)
                            || e.key.to_string().to_lowercase() == name
                    })
                }
            })
    }

    /// The keys that stay visible, or `None` when no filter is active.
    pub fn visible(&self, all: &[Ticket]) -> Option<Vec<TicketKey>> {
        self.is_active().then(|| {
            all.iter()
                .filter(|t| self.matches(t, all))
                .map(|t| t.key.clone())
                .collect()
        })
    }

    /// The suggestions under the field: every state filter, the selected ticket's chain, and each epic,
    /// with how many cards each would show.
    pub fn suggestions(
        &self,
        all: &[Ticket],
        selected: Option<&TicketKey>,
    ) -> Vec<(Token, String, usize)> {
        let count = |token: &Token| {
            let f = Filter {
                tokens: vec![token.clone()],
                text: String::new(),
            };
            all.iter().filter(|t| f.matches(t, all)).count()
        };
        let mut out: Vec<(Token, String)> = Is::ALL
            .iter()
            .map(|i| (Token::Is(*i), i.description().to_owned()))
            .collect();
        if let Some(k) = selected {
            out.push((Token::Dep(k.clone()), format!("in {k}’s chain")));
        }
        for epic in all.iter().filter(|t| t.is_epic()) {
            out.push((
                Token::Epic(epic.title.clone()),
                format!("tickets in epic {}", epic.key),
            ));
        }
        let typed = self.text.trim().to_lowercase();
        out.into_iter()
            .filter(|(t, _)| !self.tokens.contains(t))
            .filter(|(t, _)| typed.is_empty() || t.label().to_lowercase().starts_with(&typed))
            .map(|(t, d)| {
                let n = count(&t);
                (t, d, n)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use crewboard_core::domain::Kind;

    fn sample() -> Vec<Ticket> {
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = Kind::Epic;
        let mut moved = ticket("SHOP-40.4", "Wishlist share link", Status::Backlog);
        moved.epic = Some("SHOP-40".parse().unwrap());
        moved.aliases = vec!["SHOP-51".parse().unwrap()];
        let mut checkout = ticket("SHOP-20", "Checkout page", Status::Queued);
        checkout.depends_on = vec!["SHOP-18".parse().unwrap()];
        let mut running = ticket("SHOP-19", "Price table", Status::Working);
        running.flags.run_port = Some(4020);
        vec![
            epic,
            moved,
            ticket("SHOP-18", "Checkout API", Status::ReadyForReview),
            checkout,
            ticket("SHOP-22", "Fix flaky login redirect", Status::NeedsYou),
            running,
            ticket("SHOP-25", "Upgrade to Next.js 15", Status::Parked),
        ]
    }

    fn shown(filter: &Filter, all: &[Ticket]) -> Vec<String> {
        filter
            .visible(all)
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn text_matches_key_title_and_aliases() {
        let all = sample();
        let mut f = Filter::default();
        f.take_input("checkout");
        assert_eq!(shown(&f, &all), ["SHOP-18", "SHOP-20"]);
        f.take_input("SHOP-51");
        assert_eq!(
            shown(&f, &all),
            ["SHOP-40.4"],
            "the old key finds the moved ticket"
        );
        f.take_input("wishlist");
        assert_eq!(shown(&f, &all), ["SHOP-40", "SHOP-40.4"]);
        f.clear();
        assert_eq!(f.visible(&all), None);
    }

    #[test]
    fn tokens_and_combinations() {
        let all = sample();
        let mut f = Filter::default();
        let rest = f.take_input("is:needs-me ");
        assert_eq!(rest, "");
        assert_eq!(f.tokens, vec![Token::Is(Is::NeedsMe)]);
        assert_eq!(shown(&f, &all), ["SHOP-18", "SHOP-22"]);
        f.take_input("checkout");
        assert_eq!(shown(&f, &all), ["SHOP-18"], "token and text combine");
        let mut f = Filter::default();
        f.take_input("epic:wish ");
        assert_eq!(shown(&f, &all), ["SHOP-40.4"]);
        let mut f = Filter::default();
        f.take_input("dep:shop-20 ");
        assert_eq!(shown(&f, &all), ["SHOP-18", "SHOP-20"]);
        let mut f = Filter::default();
        f.take_input("is:running is:working ");
        assert_eq!(shown(&f, &all), ["SHOP-19"]);
        f.remove(0);
        assert_eq!(f.tokens, vec![Token::Is(Is::Working)]);
        // An unfinished token stays text until a space follows.
        let mut f = Filter::default();
        assert_eq!(f.take_input("is:park"), "is:park");
        assert!(f.tokens.is_empty());
    }

    #[test]
    fn suggestions_count_and_follow_typing() {
        let all = sample();
        let mut f = Filter::default();
        let key: TicketKey = "SHOP-18".parse().unwrap();
        let s = f.suggestions(&all, Some(&key));
        let needs = s
            .iter()
            .find(|(t, _, _)| *t == Token::Is(Is::NeedsMe))
            .unwrap();
        assert_eq!(needs.2, 2);
        assert!(s.iter().any(|(t, d, n)| *t == Token::Dep(key.clone())
            && d == "in SHOP-18’s chain"
            && *n == 2));
        assert!(
            s.iter()
                .any(|(t, _, n)| *t == Token::Epic("Wishlist".into()) && *n == 1)
        );
        f.take_input("is:p");
        let labels: Vec<String> = f
            .suggestions(&all, None)
            .iter()
            .map(|(t, _, _)| t.label())
            .collect();
        assert_eq!(labels, ["is:parked"]);
    }
}
