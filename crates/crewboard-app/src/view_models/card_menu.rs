//! The card's right-click menu (TKT-06, d0047; *03 · Board & columns → Pinned tickets*): Pin / Unpin for any
//! ticket or epic. The menu grows with later tickets (Delete… T-0162; panel, terminal, diff, Send back, Close
//! ticket with their milestones).

use crewboard_core::board::tickets::Ticket;
use crewboard_core::protocol::{KeyParams, Request};

use crate::icons::IconName;
use crate::view_models::menu::MenuItem;

pub const PIN: &str = "pin";
pub const UNPIN: &str = "unpin";

/// The menu for a card.
pub fn items(ticket: &Ticket) -> Vec<MenuItem> {
    let pin = if ticket.flags.is_pinned() {
        MenuItem::new(UNPIN, "Unpin").icon(IconName::PinOff)
    } else {
        MenuItem::new(PIN, "Pin to top").icon(IconName::Pin)
    };
    vec![pin]
}

/// The service request a chosen item sends.
pub fn request(choice: &str, ticket: &Ticket) -> Option<Request> {
    let key = KeyParams {
        key: ticket.key.to_string(),
    };
    match choice {
        PIN => Some(Request::TicketPin(key)),
        UNPIN => Some(Request::TicketUnpin(key)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::board::{BoardInput, build};
    use crate::view_models::card_fixtures::{NOW, ticket};
    use crewboard_core::domain::Status;

    #[test]
    fn the_label_follows_the_pin_and_the_board_resorts() {
        let mut tickets = vec![
            ticket("SHOP-24", "Update footer links", Status::Backlog),
            ticket("SHOP-31", "Add order history page", Status::Backlog),
        ];
        let menu = items(&tickets[1]);
        assert_eq!((menu[0].id, menu[0].label.as_str()), (PIN, "Pin to top"));
        assert_eq!(
            request(menu[0].id, &tickets[1]),
            Some(Request::TicketPin(KeyParams {
                key: "SHOP-31".into()
            }))
        );
        let order = |tickets: &[Ticket]| -> Vec<String> {
            build(&BoardInput {
                tickets,
                visible: None,
                project_base: None,
                now_ms: NOW,
                dev_budget: None,
                done_collapsed: false,
            })[0]
                .cards
                .iter()
                .map(|c| c.key().to_string())
                .collect()
        };
        assert_eq!(order(&tickets), ["SHOP-24", "SHOP-31"]);
        // The service pinned it; the event brings the ticket back with pinned_at set.
        tickets[1].flags.pinned_at = Some(1);
        assert_eq!(order(&tickets), ["SHOP-31", "SHOP-24"]);
        let menu = items(&tickets[1]);
        assert_eq!((menu[0].id, menu[0].label.as_str()), (UNPIN, "Unpin"));
        assert_eq!(
            request(UNPIN, &tickets[1]),
            Some(Request::TicketUnpin(KeyParams {
                key: "SHOP-31".into()
            }))
        );
        assert_eq!(request("nothing", &tickets[1]), None);
    }
}
