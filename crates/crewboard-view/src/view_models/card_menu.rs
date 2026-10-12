//! The card's right-click menu (TKT-06, d0047; *03 · Board & columns → Pinned tickets*): Pin / Unpin for any
//! ticket or epic, then the Priority group (TKT-09, d0065: the four values, the current one checked; a pick is a
//! `ticket.update`); Move to epic… for tickets that never went to Dev and Close epic… on epics (T-0043). The
//! menu grows with later tickets (Delete… T-0162; panel, terminal, diff, Send back, Close ticket with their
//! milestones).

use crewboard_core::board::tickets::Ticket;
use crewboard_core::protocol::{KeyParams, Request};
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::view_models::epic_actions::{can_close_epic, can_move_to_epic};
use crate::view_models::menu::MenuItem;
use crate::view_models::priority;

pub const PIN: &str = "pin";
pub const UNPIN: &str = "unpin";
pub const MOVE_TO_EPIC: &str = "move-to-epic";
pub const CLOSE_EPIC: &str = "close-epic";

/// The menu for a card. `has_epics`: the project has an open epic to move into.
pub fn items(ticket: &Ticket, has_epics: bool) -> Vec<MenuItem> {
    let pin = if ticket.flags.is_pinned() {
        MenuItem::new(UNPIN, "Unpin").icon(IconName::PinOff)
    } else {
        MenuItem::new(PIN, "Pin to top").icon(IconName::Pin)
    };
    let mut items = vec![pin];
    // The menu has no submenus: the group sits inline under its label, next to Pin / Unpin.
    items.push(MenuItem::header(priority::MENU_HEADER).separated());
    items.extend(priority::menu_items(ticket.priority));
    if has_epics && can_move_to_epic(ticket) {
        items.push(
            MenuItem::new(MOVE_TO_EPIC, "Move to epic…")
                .icon(IconName::Layers)
                .separated(),
        );
    }
    items.extend(crate::view_models::close::close_item(ticket));
    items.push(crate::view_models::delete::menu_item(ticket));
    if can_close_epic(ticket) {
        items.push(
            MenuItem::new(CLOSE_EPIC, "Close epic…")
                .icon(IconName::CircleSlash)
                .danger()
                .separated(),
        );
    }
    items
}

/// What a chosen item does on the page (T-0221): most send their request at once; the rest open a dialog by
/// overlay id, so the page never compares item ids itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "CardMenuChoice")]
pub enum Choice {
    /// The item's request was sent (Pin, Unpin, a priority).
    Sent,
    /// The delete confirmation (`delete_ticket`).
    Delete,
    /// The Close ticket confirmation (`close_ticket`).
    CloseTicket,
    /// Move to epic… (the epic dialogs, T-0223).
    MoveToEpic,
    /// Close epic… (the epic dialogs, T-0223).
    CloseEpic,
    /// Not an item of this menu.
    Nothing,
}

/// What choosing `id` does; `Sent` only when it has a request for `ticket`.
pub fn choice(id: &str, ticket: &Ticket) -> Choice {
    match id {
        crate::view_models::delete::DELETE => Choice::Delete,
        crate::view_models::close::CLOSE_TICKET => Choice::CloseTicket,
        MOVE_TO_EPIC => Choice::MoveToEpic,
        CLOSE_EPIC => Choice::CloseEpic,
        _ if request(id, ticket).is_some() => Choice::Sent,
        _ => Choice::Nothing,
    }
}

/// The service request a chosen item sends.
pub fn request(choice: &str, ticket: &Ticket) -> Option<Request> {
    let key = KeyParams {
        key: ticket.key.to_string(),
    };
    match choice {
        PIN => Some(Request::TicketPin(key)),
        UNPIN => Some(Request::TicketUnpin(key)),
        _ => priority::from_menu_id(choice).map(|p| priority::request(&ticket.key, p)),
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
        let menu = items(&tickets[1], false);
        assert_eq!(
            menu.len(),
            8,
            "pin, the priority group, close and delete; no epic to move into"
        );
        assert_eq!(menu[6].id.as_str(), crate::view_models::close::CLOSE_TICKET);
        assert_eq!(menu[7].id.as_str(), crate::view_models::delete::DELETE);
        assert_eq!(
            (menu[0].id.as_str(), menu[0].label.as_str()),
            (PIN, "Pin to top")
        );
        assert_eq!(items(&tickets[1], true)[6].id.as_str(), MOVE_TO_EPIC);
        assert_eq!(
            request(&menu[0].id, &tickets[1]),
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
        let menu = items(&tickets[1], false);
        assert_eq!(
            (menu[0].id.as_str(), menu[0].label.as_str()),
            (UNPIN, "Unpin")
        );
        assert_eq!(
            request(UNPIN, &tickets[1]),
            Some(Request::TicketUnpin(KeyParams {
                key: "SHOP-31".into()
            }))
        );
        assert_eq!(request("nothing", &tickets[1]), None);
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = crewboard_core::domain::Kind::Epic;
        let epic_menu = items(&epic, true);
        assert_eq!(
            epic_menu.len(),
            8,
            "pin, the priority group, delete and close; no move for an epic"
        );
        assert!(epic_menu[7].danger && epic_menu[7].id.as_str() == CLOSE_EPIC);
    }

    /// Every item says what it does on the page; dialogs open by overlay id, the rest send their request.
    #[test]
    fn each_item_has_its_choice() {
        let t = ticket("SHOP-21", "Order confirmation email", Status::Backlog);
        let choices: Vec<Choice> = items(&t, true)
            .iter()
            .filter(|i| !i.header)
            .map(|i| choice(&i.id, &t))
            .collect();
        assert_eq!(
            choices,
            [
                Choice::Sent,
                Choice::Sent,
                Choice::Sent,
                Choice::Sent,
                Choice::Sent,
                Choice::MoveToEpic,
                Choice::CloseTicket,
                Choice::Delete,
            ]
        );
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = crewboard_core::domain::Kind::Epic;
        assert_eq!(choice(CLOSE_EPIC, &epic), Choice::CloseEpic);
        assert_eq!(choice("nothing", &t), Choice::Nothing);
        assert_eq!(
            serde_json::to_value(Choice::CloseTicket).unwrap()["kind"],
            "close_ticket"
        );
    }

    /// The Priority group next to Pin / Unpin: its label, the four values with the current one checked, and a
    /// pick sends `ticket.update` with the priority only.
    #[test]
    fn the_priority_group_sets_the_priority() {
        use crewboard_core::board::tickets::TicketUpdate;
        use crewboard_core::domain::Priority;
        use crewboard_core::protocol::TicketUpdateParams;

        let mut t = ticket("SHOP-35", "Slow checkout query", Status::Queued);
        t.priority = Priority::High;
        let menu = items(&t, false);
        assert!(menu[1].header && menu[1].label == priority::MENU_HEADER);
        let group: Vec<_> = menu[2..6]
            .iter()
            .map(|i| (i.label.as_str(), i.checked))
            .collect();
        assert_eq!(
            group,
            [
                ("P1 · Urgent", false),
                ("P2 · High", true),
                ("P3 · Normal", false),
                ("P4 · Low", false)
            ]
        );
        assert_eq!(
            request(&menu[2].id, &t),
            Some(Request::TicketUpdate(TicketUpdateParams {
                key: "SHOP-35".into(),
                update: TicketUpdate {
                    priority: Some(Priority::Urgent),
                    ..TicketUpdate::default()
                },
            }))
        );
        // An epic child following its epic's P2 has P2 checked; picking it gives the child its own.
        let mut child = ticket("SHOP-40.2", "Wishlist button", Status::Backlog);
        child.priority = Priority::High;
        child.priority_inherited = true;
        assert!(items(&child, false)[3].checked);
        assert!(matches!(
            request(&priority::menu_id(Priority::High), &child),
            Some(Request::TicketUpdate(p)) if p.update.priority == Some(Priority::High)
        ));
    }
}
