//! Tokens in the header, on cards, in the panel and in History (USG-01; d0020, d0086; *02 · App shell → Header
//! states*, *15 · Settings → Global · Usage*). The counts come from the service's `usage.summary`; the shown
//! number is [`Tokens::shown`].

use std::collections::HashMap;

use otaq_core::board::tickets::Ticket;
use otaq_core::usage::{SessionUsage, Tokens, UsageSummary};
use serde::Serialize;
use ts_rs::TS;

use crate::view_models::card::tokens_label;

/// The header's usage item: "1.84M today".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct HeaderUsage {
    /// "1.84M" (mono, primary).
    pub number: String,
    /// "today" (muted).
    pub label: String,
    /// The hover text's title and detail.
    pub tooltip: (String, String),
}

/// The header item from today's summary.
pub fn header(summary: &UsageSummary) -> HeaderUsage {
    let today = &summary.today;
    let reasoning = today
        .reasoning
        .map(|r| format!(" · {} reasoning", tokens_label(r)))
        .unwrap_or_default();
    let detail = format!(
        "{} in · {} out{reasoning}, all projects (cached input not counted)",
        tokens_label(today.input),
        tokens_label(today.output),
    );
    HeaderUsage {
        number: tokens_label(summary.today.shown()),
        label: "today".to_owned(),
        tooltip: ("Tokens today".to_owned(), detail),
    }
}

/// Fills every ticket's token count from the summary: a known count, "unknown" ("—"), or nothing when the
/// ticket never ran a conversation.
pub fn apply(tickets: &mut [Ticket], summary: &UsageSummary) {
    let by_id: HashMap<&str, Option<&Tokens>> = summary
        .tickets
        .iter()
        .map(|t| (t.id.as_str(), t.tokens.as_ref()))
        .collect();
    for ticket in tickets {
        apply_one(ticket, by_id.get(ticket.id.as_str()).copied());
    }
}

/// Fills one ticket (`None`: it ran no conversation; `Some(None)`: unknown).
pub fn apply_one(ticket: &mut Ticket, usage: Option<Option<&Tokens>>) {
    ticket.flags.tokens = usage.flatten().map(Tokens::shown);
    ticket.flags.tokens_unknown = matches!(usage, Some(None));
}

/// One ticket's entry in the summary.
pub fn of_ticket<'a>(summary: &'a UsageSummary, id: &str) -> Option<Option<&'a Tokens>> {
    summary
        .tickets
        .iter()
        .find(|t| t.id == id)
        .map(|t| t.tokens.as_ref())
}

/// A History row's tokens: "226k", or "—" when unknown or not an agent session.
pub fn session_label(usage: &[SessionUsage], session: &str) -> String {
    usage
        .iter()
        .find(|u| u.session == session)
        .and_then(|u| u.tokens)
        .map_or_else(|| "—".to_owned(), |t| tokens_label(t.shown()))
}

#[cfg(test)]
mod tests {
    use otaq_core::domain::Status;
    use otaq_core::usage::TicketUsage;

    use super::*;
    use crate::view_models::card_fixtures;

    fn summary() -> UsageSummary {
        UsageSummary {
            day: "2026-10-10".into(),
            today: Tokens {
                input: 40_000 + 1_200_000,
                cached: 30_000_000,
                output: 600_000,
                reasoning: None,
            },
            tickets: vec![
                TicketUsage {
                    id: "known".into(),
                    tokens: Some(Tokens {
                        input: 400 + 180_000,
                        cached: 4_000_000,
                        output: 2_000,
                        reasoning: None,
                    }),
                    ..TicketUsage::default()
                },
                TicketUsage {
                    id: "unknown".into(),
                    tokens: None,
                    ..TicketUsage::default()
                },
            ],
            plan: None,
            limit_waiting: 0,
        }
    }

    #[test]
    fn the_header_shows_todays_tokens() {
        let header = header(&summary());
        assert_eq!(header.number, "1.84M");
        assert_eq!(header.label, "today");
        assert_eq!(
            header.tooltip,
            (
                "Tokens today".to_owned(),
                "1.24M in · 600k out, all projects (cached input not counted)".to_owned()
            )
        );
    }

    #[test]
    fn tickets_get_a_count_an_unknown_or_nothing() {
        let mut tickets = vec![
            card_fixtures::ticket("SHOP-18", "Known", Status::Working),
            card_fixtures::ticket("SHOP-19", "Unknown", Status::Working),
            card_fixtures::ticket("SHOP-20", "Never ran", Status::Backlog),
        ];
        tickets[0].id = "known".into();
        tickets[1].id = "unknown".into();
        tickets[2].flags.tokens = Some(5);
        apply(&mut tickets, &summary());
        assert_eq!(tickets[0].flags.tokens, Some(182_400));
        assert!(!tickets[0].flags.tokens_unknown);
        assert_eq!(tickets[1].flags.tokens, None);
        assert!(tickets[1].flags.tokens_unknown);
        assert_eq!(tickets[2].flags.tokens, None);
        assert!(!tickets[2].flags.tokens_unknown);
    }

    #[test]
    fn history_rows_show_each_sessions_tokens() {
        let usage = vec![
            SessionUsage {
                session: "a".into(),
                tokens: Some(Tokens {
                    output: 226_000,
                    ..Default::default()
                }),
            },
            SessionUsage {
                session: "b".into(),
                tokens: None,
            },
        ];
        assert_eq!(session_label(&usage, "a"), "226k");
        assert_eq!(session_label(&usage, "b"), "—");
        assert_eq!(session_label(&usage, "shell"), "—");
    }
}
