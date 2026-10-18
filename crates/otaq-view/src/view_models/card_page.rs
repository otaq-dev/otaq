//! A card as the Tauri page draws it (T-0218, Ticket Card v2 since T-0322): [`CardViewModel`] and
//! [`EpicViewModel`] turned into plain, serialized shapes (`TicketCardView`, `EpicCardView`) with every text the card
//! shows already worded: the head flag, the situation line, the origin line, the footer, the compact answer
//! options, the hover's full status and the quick actions. The page only lays them out.

use otaq_core::board::tickets::Ticket;
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::view_models::card::{
    CardAction, CardActionKind, CardExtras, CardViewModel, Footer, Outline, Situation, Sleeve,
    StatusHover,
};
use crate::view_models::card_fixtures::{self, Look, NOW};
use crate::view_models::epic_card::{ChildState, EpicViewModel};
use crate::view_models::priority::{self, PriorityView};
use crate::view_models::status::PillView;
use crate::view_models::tool_avatar::ToolAvatarView;

/// The hover quick actions' tooltips and key (*Interaction states → Hover*: open terminal, open panel).
pub const OPEN_PANEL: &str = "Open panel";
pub const OPEN_PANEL_KEYS: &str = "\u{21b5}";
pub const OPEN_TERMINAL: &str = "Open terminal";
/// The epic card's last row.
pub const ADD_TO_EPIC: &str = "Add ticket to epic";
/// The recommended option's badge on a card (the panel and inbox say "Recommended").
pub const RECOMMENDED_SHORT: &str = "Rec.";
/// The last option on a card (the panel says "Other… type your own answer").
pub const OTHER_SHORT: &str = "Other\u{2026}";

/// The card's width in the frames (*Ticket Card v2*, 240 px) and at a 1280 window (*Cards at 1280*, the column
/// about 200 px wide).
pub const CARD_WIDTH: u32 = 240;
pub const CARD_WIDTH_1280: u32 = 200;

/// One answer option in the card's compact list (ATT-12): number, label, the short "Rec." badge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CardOption")]
pub struct OptionView {
    pub number: u32,
    pub label: String,
    pub recommended: Option<String>,
    pub picked: bool,
    pub other: bool,
    /// What a click sends: the option's number (Other… is the last).
    pub action: CardActionKind,
    /// The option's own note, its pros and cons (ATT-17), whole: the option's hover card.
    pub note: Option<String>,
}

/// A Ticket Card v2 for the page (SHL-07): head (avatar, key, pin, priority, flag), title, situation line, origin
/// line, footer, and the hover. `faded` is the Done cards' opacity.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "TicketCardView")]
pub struct TicketCardView {
    pub key: String,
    /// "SHOP-51 → SHOP-40.4" after Move to epic.
    pub key_label: String,
    /// Whoever has the turn; `null` while only one tool is connected.
    pub avatar: Option<ToolAvatarView>,
    pub pinned: bool,
    pub priority: Option<PriorityView>,
    /// The one short flag (≤ 12 characters); `null` when nothing takes the head.
    pub flag: Option<PillView>,
    pub title: String,
    pub situation: Option<Situation>,
    /// "Created by Checkout slowness": violet (d0068).
    pub origin: Option<String>,
    pub options: Vec<OptionView>,
    pub footer: Option<Footer>,
    /// The full status and what the card has no room for.
    pub hover: StatusHover,
    /// The violet band on top of an epic child's card (EPC-09).
    pub sleeve: Option<Sleeve>,
    pub outline: Outline,
    pub faded: Option<f32>,
    /// A Project agent just created it: the page plays the fading highlight once.
    pub fresh: bool,
    pub open_panel: String,
    pub open_panel_keys: String,
    pub open_terminal: String,
}

impl TicketCardView {
    pub fn of(ticket: &Ticket, extras: &CardExtras, now_ms: i64) -> Self {
        Self::from_vm(CardViewModel::from(ticket, extras, now_ms), ticket)
    }

    /// From a view model already built (the board's, with its queue facts and signal).
    pub fn from_vm(vm: CardViewModel, _ticket: &Ticket) -> Self {
        let options = vm.options.as_ref().map_or_else(Vec::new, |o| {
            o.rows()
                .into_iter()
                .map(|row| OptionView {
                    number: row.number,
                    label: if row.other {
                        OTHER_SHORT.to_owned()
                    } else {
                        row.label
                    },
                    recommended: row.recommended.then(|| RECOMMENDED_SHORT.to_owned()),
                    picked: row.picked,
                    other: row.other,
                    action: CardActionKind::PickOption(row.number),
                    note: row.note,
                })
                .collect()
        });
        Self {
            key: vm.key.to_string(),
            key_label: vm.key_label,
            avatar: vm.avatar,
            pinned: vm.pinned,
            priority: vm.priority.map(priority::view),
            flag: vm.flag.as_ref().map(|f| f.pill()),
            title: vm.title,
            situation: vm.situation,
            origin: vm.origin,
            options,
            footer: vm.footer,
            hover: vm.hover,
            sleeve: vm.sleeve,
            outline: vm.outline,
            faded: vm.faded,
            fresh: vm.fresh,
            open_panel: OPEN_PANEL.to_owned(),
            open_panel_keys: OPEN_PANEL_KEYS.to_owned(),
            open_terminal: OPEN_TERMINAL.to_owned(),
        }
    }
}

/// A child row of the Epic Card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EpicChildRow")]
pub struct ChildView {
    pub key: String,
    pub title: String,
    pub state: ChildState,
    pub icon: IconName,
    pub status_label: String,
    /// The row can be dragged out to queue only this child (d0080).
    pub draggable: bool,
    /// "added by Wishlist lead" (d0083).
    pub origin: Option<String>,
}

/// An Epic Card for the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "EpicCardView")]
pub struct EpicCardView {
    pub key: String,
    pub pinned: bool,
    pub external_id: Option<String>,
    pub priority: Option<PriorityView>,
    pub pill: PillView,
    pub title: String,
    pub segments: Vec<ChildState>,
    pub progress: String,
    pub branch: String,
    pub children: Vec<ChildView>,
    /// "Add ticket to epic", when it can still take a child.
    pub add: Option<String>,
    pub actions: Vec<CardAction>,
    pub faded: Option<f32>,
    /// The Lead row under the branch line, or Open lead (d0083, T-0289).
    pub lead: Option<crate::view_models::lead_status::LeadStatusView>,
    /// Its lead asks you: the border is turquoise.
    pub lead_asks: bool,
}

impl EpicCardView {
    pub fn of(epic: &Ticket, tickets: &[Ticket], branch: Option<&str>) -> Self {
        Self::from_vm(EpicViewModel::from(epic, tickets, branch))
    }

    pub fn from_vm(vm: EpicViewModel) -> Self {
        let lead_asks = vm.lead_asks();
        Self {
            key: vm.key.to_string(),
            pinned: vm.pinned,
            external_id: vm.external_id,
            priority: vm.priority.map(priority::view),
            pill: PillView::legend(vm.pill.legend, Some(&vm.pill.label)),
            title: vm.title,
            segments: vm.segments,
            progress: vm.progress,
            branch: vm.branch,
            children: vm
                .children
                .into_iter()
                .map(|c| ChildView {
                    key: c.key.to_string(),
                    title: c.title,
                    state: c.state,
                    icon: c.state.icon(),
                    status_label: c.status_label.to_owned(),
                    draggable: c.draggable,
                    origin: c.origin,
                })
                .collect(),
            add: vm.can_add.then(|| ADD_TO_EPIC.to_owned()),
            actions: vm.actions,
            faded: vm.faded,
            lead: vm.lead,
            lead_asks,
        }
    }
}

/// One cell of the gallery's variant grid: the frame's variant name and the look it is drawn in.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "CardGalleryCell")]
pub struct GalleryCell {
    pub name: String,
    pub look: Look,
    pub card: TicketCardView,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "EpicGalleryCell")]
pub struct EpicCell {
    pub name: String,
    pub card: EpicCardView,
}

/// The gallery's cards: *04 → Ticket Card v2 · all variants* in the frame's order (the M21 ones), drawn at
/// [`CARD_WIDTH`] and at [`CARD_WIDTH_1280`]; the epic rows; the sleeved children (*Epic Sleeve v2*); and the
/// lead's rows. From [`card_fixtures`].
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "CardGallery")]
pub struct CardGallery {
    pub variants: Vec<GalleryCell>,
    /// The normal card width and the width at 1280 (*Cards at 1280*): the page draws the variants at both.
    pub width: u32,
    pub width_1280: u32,
    pub epics: Vec<EpicCell>,
    /// *Epic Sleeve v2*: sleeved epic children (EPC-09).
    pub sleeves: Vec<GalleryCell>,
    /// *04 → Row · Epic children & the lead (0083)* and *Row · Unread notes & same file (0083 · T)* (T-0291).
    pub leads: Vec<GalleryCell>,
    /// *04 → Variants · one tool (M22)* and the picked-by cards (T-0345): the footer's agent part, Start failed and
    /// No result on a refused model.
    pub agents: Vec<GalleryCell>,
    /// *04 → Variants · several tools (M23)* and the fallback and limit cards (T-0362): avatars on, Unavailable,
    /// waiting on a tool, moved by a fallback.
    pub tools: Vec<GalleryCell>,
}

pub fn gallery() -> CardGallery {
    let cell = |v: &card_fixtures::Variant| GalleryCell {
        name: v.name.to_owned(),
        look: v.look,
        card: TicketCardView::of(&v.ticket, &v.extras, NOW),
    };
    CardGallery {
        variants: card_fixtures::variants().iter().map(cell).collect(),
        width: CARD_WIDTH,
        width_1280: CARD_WIDTH_1280,
        epics: card_fixtures::epic_variants()
            .iter()
            .map(|v| EpicCell {
                name: v.name.to_owned(),
                card: EpicCardView::of(&v.epic, &v.tickets, v.branch),
            })
            .collect(),
        sleeves: card_fixtures::sleeve_variants().iter().map(cell).collect(),
        leads: card_fixtures::lead_variants().iter().map(cell).collect(),
        agents: card_fixtures::agent_variants().iter().map(cell).collect(),
        tools: card_fixtures::tool_variants().iter().map(cell).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Palette;

    fn cell<'a>(g: &'a CardGallery, name: &str) -> &'a TicketCardView {
        &g.variants
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no variant {name}"))
            .card
    }

    #[test]
    fn the_page_gets_the_view_models_flag() {
        let g = gallery();
        let card = cell(&g, "Needs you + running + 2 dependencies");
        let flag = card.flag.as_ref().unwrap();
        assert_eq!(
            (flag.label.as_str(), flag.palette),
            ("Needs you", Palette::Attn)
        );
        assert_eq!(card.outline, Outline::Attention);
        assert_eq!(card.hover.status, "Needs you \u{b7} permission");
        assert!(card.avatar.is_none(), "one tool connected: no avatar");
    }

    #[test]
    fn every_fixture_reaches_the_page() {
        let g = gallery();
        assert_eq!(g.variants.len(), card_fixtures::variants().len());
        assert_eq!(g.epics.len(), card_fixtures::epic_variants().len());
        assert_eq!(g.sleeves.len(), card_fixtures::sleeve_variants().len());
        assert_eq!(g.leads.len(), card_fixtures::lead_variants().len());
        assert_eq!(g.agents.len(), card_fixtures::agent_variants().len());
        assert_eq!(g.tools.len(), card_fixtures::tool_variants().len());
        assert!(
            g.tools.iter().all(|c| c.card.avatar.is_some()),
            "several tools: avatars on"
        );
        assert_eq!((g.width, g.width_1280), (240, 200));
        let looks: Vec<Look> = g.variants.iter().map(|c| c.look).collect();
        for look in [
            Look::Plain,
            Look::Selected,
            Look::Hovered,
            Look::Dragging,
            Look::Chain,
            Look::Dimmed,
        ] {
            assert!(looks.contains(&look), "{look:?}");
        }
        let json = serde_json::to_value(&g).unwrap();
        assert_eq!(json["variants"][0]["look"], "plain");
        assert_eq!(json["variants"][0]["card"]["outline"], "normal");
        // No field of the v1 card is left (SHL-07).
        let card = json["variants"][0]["card"].as_object().unwrap();
        for old in [
            "pill",
            "pill_short",
            "callout",
            "entry",
            "meta",
            "bars",
            "actions",
            "compact",
        ] {
            assert!(!card.contains_key(old), "{old}");
        }
    }

    #[test]
    fn options_footer_and_epic_rows_are_worded_here() {
        let g = gallery();
        let footer = cell(&g, "Working").footer.as_ref().unwrap();
        let json = serde_json::to_value(footer).unwrap();
        assert_eq!(json["tokens"]["text"], "182k");
        assert_eq!(json["signal"]["text"], "40s");
        assert_eq!(json["action"], serde_json::Value::Null);
        let epics = &g.epics;
        assert_eq!(epics[0].card.add.as_deref(), Some(ADD_TO_EPIC));
        assert_eq!(epics[0].card.children[0].status_label, "Merged");
        let done = epics.iter().find(|e| e.name == "Epic · Done").unwrap();
        assert_eq!(done.card.add, None);
        let json = serde_json::to_value(&epics[2].card).unwrap();
        assert_eq!(json["actions"][0]["kind"], "mark_merged");
        assert_eq!(json["segments"][0], "merged");
    }

    #[test]
    fn answer_options_use_the_short_wording() {
        use otaq_core::board::questions::HumanQuestion;
        let mut t =
            card_fixtures::ticket("SHOP-21", "Order email", otaq_core::domain::Status::AsksYou);
        t.question = Some(HumanQuestion {
            options: vec!["Only the profile form".into(), "Everywhere".into()],
            recommended: Some(1),
            notes: Vec::new(),
            id: "q1".into(),
            text: "Split fullName everywhere?".into(),
            asked_at: NOW,
            via_lead: None,
        });
        let card = TicketCardView::of(&t, &CardExtras::default(), NOW);
        assert_eq!(
            card.options[0].recommended.as_deref(),
            Some(RECOMMENDED_SHORT)
        );
        let last = card.options.last().unwrap();
        assert_eq!((last.label.as_str(), last.other), (OTHER_SHORT, true));
        assert_eq!(
            serde_json::to_value(last.action).unwrap(),
            serde_json::json!({ "pick_option": 3 })
        );
        assert_eq!(
            card.footer.unwrap().action.unwrap().kind,
            CardActionKind::SendAnswer
        );
    }

    /// ATT-17: the compact list keeps its labels; each option carries its note whole for the hover card.
    #[test]
    fn each_option_carries_its_note_for_the_hover_card() {
        use otaq_core::board::questions::HumanQuestion;
        let long = "y".repeat(600);
        let mut t =
            card_fixtures::ticket("NOTE-1", "Add export", otaq_core::domain::Status::AsksYou);
        t.question = Some(HumanQuestion {
            options: vec!["Markdown files".into(), "One ZIP".into()],
            recommended: Some(1),
            notes: vec![None, Some(long.clone())],
            id: "q1".into(),
            text: "Question 1 of 2: Which export format?".into(),
            asked_at: NOW,
            via_lead: None,
        });
        let card = TicketCardView::of(&t, &CardExtras::default(), NOW);
        let shown: Vec<(&str, Option<&str>)> = card
            .options
            .iter()
            .map(|o| (o.label.as_str(), o.note.as_deref()))
            .collect();
        assert_eq!(
            shown,
            [
                ("Markdown files", None),
                ("One ZIP", Some(long.as_str())),
                (OTHER_SHORT, None)
            ]
        );
    }
}
