//! Search and filter (UI-02, EPC-01; d0058 ⌘K; *03 · Board & columns → Search & filter · ⌘K*): one field does
//! both. Free text matches the key (old keys from Move to epic too), the title and the epic's title; tokens
//! filter by state (`is:needs-me`, `is:working`, …), by chain (`dep:SHOP-18`) or by epic (`epic:Wishlist`).
//! Everything combines (all must match). Non-matching cards hide; columns show "shown/total".
//!
//! An `epic:` token is the same as focusing the epic (EPC-18, d0080, Q55): on the board it **dims** the other cards
//! instead of hiding them, so the board keeps its shape ([`Filter::hiding`], [`Filter::strong`]); the other words
//! still hide. Focusing an epic (its track's name, ⇧⌘G) puts its token in the filter ([`Filter::focus`]); Esc takes
//! it out ([`Filter::unfocus`]).

use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{Column, Status, TicketKey};

use crate::view_models::chain::chain;

/// A state filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Is {
    /// Needs you: the needs-you family, Interrupted, Start interrupted, Resume failed, Parent closed, and review.
    NeedsMe,
    /// The agent works: Working, Working in background, Validating…, Merge conflict · fixing.
    Working,
    /// The M12 validation states: Validating…, Needs you · validator, Validation failed N times, Validation ended
    /// without a result (d0063).
    Validating,
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
        Is::Validating,
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
            Self::Validating => "validating",
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
            Self::Validating => "validating, or its validation needs you",
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
            Self::NeedsMe => t.needs_user() || t.status == Status::ReadyForReview,
            Self::Working => matches!(
                t.status,
                Status::Working
                    | Status::WorkingInBackground
                    | Status::Validating
                    | Status::MergeConflictFixing
            ),
            Self::Validating => matches!(
                t.status,
                Status::Validating
                    | Status::ValidatorNeedsYou
                    | Status::ValidationFailed
                    | Status::ValidationNoResult
            ),
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

/// Whether an `epic:` token's `name` names `epic`: its title starts with it, or it is its key (case-insensitive).
fn names_epic(name: &str, epic: &Ticket) -> bool {
    let name = name.to_lowercase();
    epic.title.to_lowercase().starts_with(&name) || epic.key.to_string().to_lowercase() == name
}

/// Whether `t` is the epic `name` names, or one of its children.
fn in_epic(name: &str, t: &Ticket, all: &[Ticket]) -> bool {
    if t.is_epic() {
        return names_epic(name, t);
    }
    t.epic
        .as_ref()
        .and_then(|e| all.iter().find(|x| &x.key == e))
        .is_some_and(|e| names_epic(name, e))
}

/// The epic a focus on `key` focuses (⇧⌘G, EPC-18): the epic itself, or a child's epic. `None` for a standalone
/// ticket, which has nothing to focus.
pub fn epic_of<'a>(key: &TicketKey, all: &'a [Ticket]) -> Option<&'a Ticket> {
    let t = all.iter().find(|t| &t.key == key)?;
    if t.is_epic() {
        return Some(t);
    }
    let epic = t.epic.as_ref()?;
    all.iter().find(|x| &x.key == epic)
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
                Token::Epic(name) => epic_title(t).is_some_and(|e| names_epic(name, e)),
            })
    }

    /// The filter the board hides by: everything but the `epic:` tokens, which dim instead (Q55).
    pub fn hiding(&self) -> Filter {
        Filter {
            tokens: self
                .tokens
                .iter()
                .filter(|t| !matches!(t, Token::Epic(_)))
                .cloned()
                .collect(),
            text: self.text.clone(),
        }
    }

    /// With an `epic:` token, the tickets the board shows at full strength: the epic's children and the epic itself
    /// (every `epic:` token must match). `None` without one: nothing dims.
    pub fn strong(&self, all: &[Ticket]) -> Option<Vec<TicketKey>> {
        let names: Vec<&str> = self.epic_names().collect();
        (!names.is_empty()).then(|| {
            all.iter()
                .filter(|t| names.iter().all(|name| in_epic(name, t, all)))
                .map(|t| t.key.clone())
                .collect()
        })
    }

    /// The epics the `epic:` tokens focus, in project order.
    pub fn focused_epics(&self, all: &[Ticket]) -> Vec<TicketKey> {
        let names: Vec<&str> = self.epic_names().collect();
        if names.is_empty() {
            return Vec::new();
        }
        all.iter()
            .filter(|t| t.is_epic() && names.iter().all(|name| names_epic(name, t)))
            .map(|t| t.key.clone())
            .collect()
    }

    fn epic_names(&self) -> impl Iterator<Item = &str> {
        self.tokens.iter().filter_map(|t| match t {
            Token::Epic(name) => Some(name.as_str()),
            _ => None,
        })
    }

    /// Focuses `epic` (EPC-18): its `epic:` token (by title) takes the place of any other; the rest stays.
    pub fn focus(&mut self, epic: &Ticket) {
        self.unfocus();
        let name = if epic.title.trim().is_empty() {
            epic.key.to_string()
        } else {
            epic.title.clone()
        };
        self.add(Token::Epic(name));
    }

    /// Leaves the focus (Esc): takes the `epic:` tokens out. Whether there was one.
    pub fn unfocus(&mut self) -> bool {
        let before = self.tokens.len();
        self.tokens.retain(|t| !matches!(t, Token::Epic(_)));
        self.tokens.len() != before
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
    use otaq_core::domain::Kind;

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

    /// The M12 states (d0063, d0064): the needs-you ones need you, Validating… and Merge conflict · fixing are
    /// working, and `is:validating` gathers the validation states.
    #[test]
    fn m12_states_in_the_status_filter() {
        let all = vec![
            ticket("SHOP-29", "Coupon field", Status::Validating),
            ticket("SHOP-33", "Shipping estimate", Status::ValidatorNeedsYou),
            ticket("SHOP-32", "Address autocomplete", Status::ValidationFailed),
            ticket("SHOP-34", "Tax display", Status::ValidationNoResult),
            ticket(
                "SHOP-40.4",
                "Wishlist share link",
                Status::MergeConflictFixing,
            ),
            ticket("SHOP-41", "Wishlist sync", Status::AutoMergeStopped),
        ];
        let only = |token: &str| {
            let mut f = Filter::default();
            f.take_input(&format!("{token} "));
            shown(&f, &all)
        };
        assert_eq!(
            only("is:needs-me"),
            ["SHOP-33", "SHOP-32", "SHOP-34", "SHOP-41"]
        );
        assert_eq!(only("is:working"), ["SHOP-29", "SHOP-40.4"]);
        assert_eq!(
            only("is:validating"),
            ["SHOP-29", "SHOP-33", "SHOP-32", "SHOP-34"]
        );
        let f = Filter::default();
        let suggested = f.suggestions(&all, None);
        assert!(
            suggested
                .iter()
                .any(|(t, _, n)| *t == Token::Is(Is::Validating) && *n == 4)
        );
    }

    /// Q55: an `epic:` token dims the other cards instead of hiding them; the epic itself stays strong with its
    /// children; other words still hide. Focusing puts the epic's token in place of another, Esc takes it out.
    #[test]
    fn an_epic_token_dims_and_focus_sets_it() {
        let mut all = sample();
        let mut search = ticket("SHOP-45", "Search", Status::Backlog);
        search.kind = Kind::Epic;
        let mut index = ticket("SHOP-45.1", "Search index API", Status::Working);
        index.epic = Some("SHOP-45".parse().unwrap());
        all.extend([search, index]);
        let keys = |k: Option<Vec<TicketKey>>| -> Vec<String> {
            k.unwrap().iter().map(ToString::to_string).collect()
        };

        let mut f = Filter::default();
        f.take_input("epic:wish ");
        assert_eq!(f.hiding().visible(&all), None, "nothing hides");
        assert_eq!(keys(f.strong(&all)), ["SHOP-40", "SHOP-40.4"]);
        assert_eq!(
            f.focused_epics(&all),
            ["SHOP-40".parse::<TicketKey>().unwrap()]
        );
        // Other words still hide; the dim stays on top of what is shown.
        f.take_input("share");
        assert_eq!(keys(f.hiding().visible(&all)), ["SHOP-40.4"]);
        assert_eq!(keys(f.strong(&all)), ["SHOP-40", "SHOP-40.4"]);
        assert_eq!(Filter::default().strong(&all), None);

        // ⇧⌘G on SHOP-45.1: its epic by title replaces Wishlist; the typed text stays.
        let epic = epic_of(&"SHOP-45.1".parse().unwrap(), &all).unwrap();
        assert_eq!(epic.key.to_string(), "SHOP-45");
        f.focus(epic);
        assert_eq!(f.tokens, vec![Token::Epic("Search".into())]);
        assert_eq!(f.text, "share");
        f.text.clear();
        assert_eq!(keys(f.strong(&all)), ["SHOP-45", "SHOP-45.1"]);
        assert!(
            epic_of(&"SHOP-22".parse().unwrap(), &all).is_none(),
            "a standalone ticket"
        );
        assert_eq!(
            epic_of(&"SHOP-40".parse().unwrap(), &all).map(|e| e.key.to_string()),
            Some("SHOP-40".into())
        );

        // Esc: only the epic token goes.
        f.add(Token::Is(Is::Working));
        assert!(f.unfocus());
        assert_eq!(f.tokens, vec![Token::Is(Is::Working)]);
        assert!(!f.unfocus());
    }
}
