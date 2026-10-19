//! The header's search and filter field as the Tauri page draws it (T-0221; UI-02, EPC-01, d0058 ⌘K; *03 · Board &
//! columns → Search & filter · ⌘K*). The page keeps what the user typed and the chips per project ([`FilterState`]);
//! [`view`] turns it into the field: finished tokens become chips, the rest stays in the field, the "4 results" count,
//! and the suggestions under it with their counts. The board takes the same state with `board.view` and hides the
//! cards that don't match ([`FilterState::filter`]).

use otaq_core::board::tickets::Ticket;
use otaq_core::domain::TicketKey;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::keymap::{KeyAction, Platform};
use crate::view_models::filter::{Filter, Token};

/// The field's placeholder, in the header and in the field.
pub const PLACEHOLDER: &str = "Search  ·  is:needs-you";

/// The suggestion list's label.
pub const SUGGESTIONS_HEADER: &str = "FILTERS";

/// What the field typed in ↵ opens Recovery and Cleanup (*16 → Where Cleanup lives*).
pub const CLEANUP_WORD: &str = "cleanup";

/// The filter as the page keeps it: the chips (`"is:needs-you"`, `"dep:SHOP-18"`) and what is typed in the field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "BoardFilter")]
pub struct FilterState {
    pub tokens: Vec<String>,
    pub text: String,
}

impl FilterState {
    /// The filter it stands for. A finished token still in the text counts as a chip, so the board never shows a
    /// moment where `is:needs-you ` is searched for as words.
    pub fn filter(&self) -> Filter {
        let mut filter = Filter {
            tokens: Vec::new(),
            text: String::new(),
        };
        for token in self.tokens.iter().filter_map(|t| Token::parse(t)) {
            filter.add(token);
        }
        filter.take_input(&self.text);
        filter
    }
}

/// One row under the field: `is:working` · "agent running now" · `3`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SearchSuggestion")]
pub struct Suggestion {
    /// The chip it adds.
    pub token: String,
    pub description: String,
    pub count: String,
}

/// The field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SearchView")]
pub struct SearchView {
    /// The filter with finished tokens moved into the chips; `text` is what the field shows.
    pub filter: FilterState,
    pub placeholder: String,
    /// ⌘K, shown while no filter is active.
    pub keys: Option<String>,
    /// "4 results" while a filter is active.
    pub results: Option<String>,
    pub suggestions_header: String,
    pub suggestions: Vec<Suggestion>,
    /// What ↵ adds: the first suggestion while a token is half typed (`is:wor`).
    pub enter_adds: Option<String>,
    /// ↵ opens Recovery and Cleanup: the field says "cleanup".
    pub enter_opens_cleanup: bool,
}

/// The field for `state` over the project's `tickets`; `selected` is the open ticket (its chain is suggested).
pub fn view(
    state: &FilterState,
    tickets: &[Ticket],
    selected: Option<&TicketKey>,
    platform: Platform,
) -> SearchView {
    let mut filter = Filter {
        tokens: Vec::new(),
        text: String::new(),
    };
    for token in state.tokens.iter().filter_map(|t| Token::parse(t)) {
        filter.add(token);
    }
    let shown = filter.take_input(&state.text);
    let results = filter.visible(tickets).map(|keys| {
        let n = keys.len();
        format!("{n} result{}", if n == 1 { "" } else { "s" })
    });
    let suggestions: Vec<Suggestion> = filter
        .suggestions(tickets, selected)
        .into_iter()
        .map(|(token, description, count)| Suggestion {
            token: token.label(),
            description,
            count: count.to_string(),
        })
        .collect();
    let enter_adds = filter
        .text
        .contains(':')
        .then(|| suggestions.first().map(|s| s.token.clone()))
        .flatten();
    SearchView {
        enter_opens_cleanup: filter.tokens.is_empty()
            && filter.text.trim().eq_ignore_ascii_case(CLEANUP_WORD),
        filter: FilterState {
            tokens: filter.tokens.iter().map(Token::label).collect(),
            text: shown,
        },
        placeholder: PLACEHOLDER.to_owned(),
        keys: results
            .is_none()
            .then(|| KeyAction::FilterBoard.keys(platform))
            .flatten(),
        results,
        suggestions_header: SUGGESTIONS_HEADER.to_owned(),
        suggestions,
        enter_adds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use otaq_core::domain::Status;

    fn sample() -> Vec<Ticket> {
        vec![
            ticket("SHOP-18", "Checkout API", Status::ReadyForReview),
            ticket("SHOP-20", "Checkout page", Status::Queued),
            ticket("SHOP-22", "Fix flaky login redirect", Status::NeedsYou),
            ticket("SHOP-19", "Price table", Status::Working),
        ]
    }

    fn state(tokens: &[&str], text: &str) -> FilterState {
        FilterState {
            tokens: tokens.iter().map(|t| (*t).to_owned()).collect(),
            text: text.to_owned(),
        }
    }

    #[test]
    fn a_finished_token_becomes_a_chip_and_the_rest_stays_typed() {
        let all = sample();
        let v = view(&state(&[], "is:needs-you "), &all, None, Platform::Mac);
        assert_eq!(v.filter, state(&["is:needs-you"], ""));
        assert_eq!(v.results.as_deref(), Some("2 results"));
        assert_eq!(v.keys, None, "the count takes ⌘K's place");
        let v = view(
            &state(&["is:needs-you"], "checkout"),
            &all,
            None,
            Platform::Mac,
        );
        assert_eq!(v.filter, state(&["is:needs-you"], "checkout"));
        assert_eq!(v.results.as_deref(), Some("1 result"));
        let v = view(&state(&[], ""), &all, None, Platform::Mac);
        assert_eq!(v.results, None);
        assert_eq!(v.keys.as_deref(), Some("/"));
        assert_eq!(v.placeholder, PLACEHOLDER);
    }

    #[test]
    fn suggestions_carry_their_counts_and_enter_takes_the_first() {
        let all = sample();
        let key: TicketKey = "SHOP-18".parse().unwrap();
        let v = view(&state(&[], ""), &all, Some(&key), Platform::Mac);
        assert_eq!(v.suggestions_header, "FILTERS");
        assert_eq!(
            v.suggestions[0],
            Suggestion {
                token: "is:needs-you".into(),
                description: "needs you, asks, interrupted, review".into(),
                count: "2".into(),
            }
        );
        assert!(v.suggestions.iter().any(|s| s.token == "dep:SHOP-18"));
        assert_eq!(v.enter_adds, None, "nothing half typed");
        let v = view(&state(&[], "is:wor"), &all, None, Platform::Mac);
        assert_eq!(v.enter_adds.as_deref(), Some("is:working"));
        let v = view(&state(&["is:working"], ""), &all, None, Platform::Mac);
        assert!(
            v.suggestions.iter().all(|s| s.token != "is:working"),
            "a chip already there isn't suggested again"
        );
    }

    #[test]
    fn cleanup_opens_recovery_and_cleanup() {
        let all = sample();
        assert!(view(&state(&[], "Cleanup"), &all, None, Platform::Mac).enter_opens_cleanup);
        assert!(
            !view(
                &state(&["is:working"], "cleanup"),
                &all,
                None,
                Platform::Mac
            )
            .enter_opens_cleanup
        );
    }

    #[test]
    fn the_board_filter_reads_chips_and_unfinished_text() {
        let all = sample();
        let shown = |s: FilterState| -> Vec<String> {
            s.filter()
                .visible(&all)
                .unwrap_or_default()
                .iter()
                .map(ToString::to_string)
                .collect()
        };
        assert_eq!(shown(state(&["is:needs-you"], "")), ["SHOP-18", "SHOP-22"]);
        assert_eq!(shown(state(&[], "is:needs-you checkout")), ["SHOP-18"]);
        assert!(
            !state(&["not-a-token"], "").filter().is_active(),
            "an unknown chip is dropped, leaving no filter"
        );
        assert!(!state(&[], "  ").filter().is_active());
    }
}
