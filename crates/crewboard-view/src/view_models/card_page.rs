//! A card as the Tauri page draws it (T-0218): [`CardViewModel`] and [`EpicViewModel`] turned into plain, serialized
//! shapes (`TicketCardView`, `EpicCardView`) with every text the card shows already worded, including the parts the
//! GPUI app wrote in its drawing code (the message bar, the quick actions, the compact answer options, "Add ticket to
//! epic") and the **compact density** line (*04 · Ticket card variants → Compact density*: the pill shrinks to its
//! icon, one value sits at the end of the key row, the title takes one line). The page only lays them out.

use crewboard_core::board::tickets::Ticket;
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::theme::Palette;
use crate::view_models::card::{
    Bar, Callout, CardAction, CardActionKind, CardExtras, CardViewModel, Entry, MetaItem, Outline,
    Tone,
};
use crate::view_models::card_fixtures::{self, Look, NOW};
use crate::view_models::epic_card::{ChildState, EpicViewModel};
use crate::view_models::priority::{self, PriorityView};
use crate::view_models::status::PillView;

/// The hover quick actions' tooltip and key (*Hovered · quick actions*).
pub const OPEN_PANEL: &str = "Open panel";
pub const OPEN_PANEL_KEYS: &str = "↵";
/// The epic card's last row.
pub const ADD_TO_EPIC: &str = "Add ticket to epic";
/// The recommended option's badge on a card (the panel and inbox say "Recommended").
pub const RECOMMENDED_SHORT: &str = "Rec.";
/// The last option on a card (the panel says "Other… type your own answer").
pub const OTHER_SHORT: &str = "Other…";

/// How a bar under the title looks: the accent "messages waiting" bar with its action, or a calm grey one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "CardBarLook")]
pub enum BarLook {
    Messages,
    Quiet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CardBar")]
pub struct BarView {
    pub look: BarLook,
    pub icon: IconName,
    pub text: String,
    /// "Deliver now" on the messages bar.
    pub action: Option<CardAction>,
}

impl BarView {
    fn of(bar: &Bar) -> Self {
        match bar {
            Bar::MessagesWaiting(n) => Self {
                look: BarLook::Messages,
                icon: IconName::MessageSquareDot,
                text: format!("{n} message{} waiting", if *n == 1 { "" } else { "s" }),
                action: Some(CardAction {
                    kind: CardActionKind::DeliverNow,
                    label: "Deliver now",
                    icon: None,
                    primary: false,
                }),
            },
            Bar::Background { text, wakeup } => Self {
                look: BarLook::Quiet,
                icon: if *wakeup {
                    IconName::AlarmClock
                } else {
                    IconName::Cpu
                },
                text: text.clone(),
                action: None,
            },
            // *Dev · Validating…*: the validator at work, in the background bar's look.
            Bar::Validator(text) => Self {
                look: BarLook::Quiet,
                icon: IconName::ShieldEllipsis,
                text: text.clone(),
                action: None,
            },
        }
    }
}

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
}

/// The value at the end of a compact card's key row (*Compact density*): the port, else the last signal, else the
/// parent (one) or the number of parents (several, with the link icon), else the tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CardCompactValue")]
pub struct CompactValue {
    pub icon: Option<IconName>,
    pub text: String,
    pub tone: Tone,
}

/// A compact card's key row: the pill as its icon alone (the label as its tooltip), and one value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CardCompact")]
pub struct CompactView {
    pub flag: Option<PillView>,
    pub value: Option<CompactValue>,
}

/// A Ticket Card for the page. `faded` is the Done cards' opacity.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "TicketCardView")]
pub struct TicketCardView {
    pub key: String,
    /// "SHOP-51 → SHOP-40.4" after Move to epic.
    pub key_label: String,
    pub pinned: bool,
    pub priority: Option<PriorityView>,
    pub title: String,
    pub pill: Option<PillView>,
    pub callout: Option<Callout>,
    pub entry: Option<Entry>,
    pub options: Vec<OptionView>,
    pub bars: Vec<BarView>,
    pub epic: Option<String>,
    pub meta: Vec<MetaItem>,
    pub outline: Outline,
    pub actions: Vec<CardAction>,
    pub faded: Option<f32>,
    pub open_panel: String,
    pub open_panel_keys: String,
    pub compact: CompactView,
}

impl TicketCardView {
    pub fn of(ticket: &Ticket, extras: &CardExtras, now_ms: i64) -> Self {
        Self::from_vm(CardViewModel::from(ticket, extras, now_ms), ticket)
    }

    /// From a view model already built (the board's, with its queue facts and signal).
    pub fn from_vm(vm: CardViewModel, ticket: &Ticket) -> Self {
        let compact = compact(&vm, ticket);
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
                })
                .collect()
        });
        Self {
            key: vm.key.to_string(),
            key_label: vm.key_label,
            pinned: vm.pinned,
            priority: vm.priority.map(priority::view),
            title: vm.title,
            pill: vm.pill.map(|p| PillView::legend(p.legend, Some(&p.label))),
            callout: vm.callout,
            entry: vm.entry,
            options,
            bars: vm.bars.iter().map(BarView::of).collect(),
            epic: vm.epic,
            meta: vm.meta,
            outline: vm.outline,
            actions: vm.actions,
            faded: vm.faded,
            open_panel: OPEN_PANEL.to_owned(),
            open_panel_keys: OPEN_PANEL_KEYS.to_owned(),
            compact,
        }
    }
}

/// A callout's tone as a pill palette, for a compact card whose only flag is its callout.
fn palette(tone: Tone) -> Palette {
    match tone {
        Tone::Attention => Palette::Attn,
        Tone::Error => Palette::Error,
        Tone::Warning => Palette::Paused,
        Tone::Working => Palette::Working,
        Tone::Review => Palette::Review,
        Tone::Running => Palette::Approved,
        Tone::Epic => Palette::Epic,
        Tone::Accent => Palette::Pinned,
        Tone::Muted | Tone::Secondary => Palette::Queued,
    }
}

fn compact(vm: &CardViewModel, ticket: &Ticket) -> CompactView {
    let flag = match (&vm.pill, &vm.callout) {
        (Some(p), _) => Some(PillView::legend(p.legend, Some(&p.label))),
        // A marker on a working card ("Validation failed · round 2 of 3") keeps its icon.
        (None, Some(c)) => Some(PillView {
            palette: palette(c.tone),
            icon: c.icon,
            label: c.text.clone(),
        }),
        (None, None) => None,
    };
    let meta = |icon: IconName| vm.meta.iter().find(|m| m.icon == icon);
    let plain = |m: &MetaItem| CompactValue {
        icon: None,
        text: m.text.clone(),
        tone: m.tone,
    };
    let parents = match ticket.depends_on.as_slice() {
        [] => None,
        [one] => Some(CompactValue {
            icon: None,
            text: one.to_string(),
            tone: Tone::Muted,
        }),
        many => Some(CompactValue {
            icon: Some(IconName::Link2),
            text: many.len().to_string(),
            tone: Tone::Muted,
        }),
    };
    let value = meta(IconName::Play)
        .or_else(|| meta(IconName::Activity))
        .map(plain)
        .or(parents)
        .or_else(|| meta(IconName::Coins).map(plain));
    CompactView { flag, value }
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
}

impl EpicCardView {
    pub fn of(epic: &Ticket, tickets: &[Ticket], branch: Option<&str>) -> Self {
        Self::from_vm(EpicViewModel::from(epic, tickets, branch))
    }

    pub fn from_vm(vm: EpicViewModel) -> Self {
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
                })
                .collect(),
            add: vm.can_add.then(|| ADD_TO_EPIC.to_owned()),
            actions: vm.actions,
            faded: vm.faded,
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

/// The gallery's cards (T-0218): *04 → Variant grid* in the frames' order, the *Compact density* comparison, and
/// the epic rows, from [`card_fixtures`].
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "CardGallery")]
pub struct CardGallery {
    pub variants: Vec<GalleryCell>,
    /// The *Compact density → Compare* column: the same tickets, comfortable and compact.
    pub compact: Vec<TicketCardView>,
    pub epics: Vec<EpicCell>,
}

/// The variants the *Compact density* frame compares, in its order.
pub const COMPACT_SAMPLES: &[&str] = &[
    "Working · needs you",
    "Working · normal + running",
    "Queued · waits for merge",
    "Working · interrupted",
    "Queued · waiting on deps",
];

pub fn gallery() -> CardGallery {
    let variants = card_fixtures::variants();
    let compact = COMPACT_SAMPLES
        .iter()
        .filter_map(|name| variants.iter().find(|v| v.name == *name))
        .map(|v| TicketCardView::of(&v.ticket, &v.extras, NOW))
        .collect();
    CardGallery {
        variants: variants
            .iter()
            .map(|v| GalleryCell {
                name: v.name.to_owned(),
                look: v.look,
                card: TicketCardView::of(&v.ticket, &v.extras, NOW),
            })
            .collect(),
        compact,
        epics: card_fixtures::epic_variants()
            .iter()
            .map(|v| EpicCell {
                name: v.name.to_owned(),
                card: EpicCardView::of(&v.epic, &v.tickets, v.branch),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell<'a>(g: &'a CardGallery, name: &str) -> &'a TicketCardView {
        &g.variants
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no variant {name}"))
            .card
    }

    #[test]
    fn the_page_gets_the_view_models_pill_for_the_rank_cards() {
        let g = gallery();
        for (name, label) in [
            ("Combined · needs you + running + 2 deps", "Needs you"),
            ("Working · needs you", "Needs you"),
        ] {
            let card = cell(&g, name);
            let variant = card_fixtures::variants()
                .into_iter()
                .find(|v| v.name == name)
                .unwrap();
            let vm = CardViewModel::from(&variant.ticket, &variant.extras, NOW);
            let pill = vm.pill.unwrap();
            assert_eq!(card.pill.as_ref().map(|p| p.label.as_str()), Some(label));
            assert_eq!(card.pill, Some(PillView::legend(pill.legend, Some(label))));
            assert_eq!(card.outline, Outline::Attention, "{name}");
        }
    }

    #[test]
    fn every_fixture_reaches_the_page() {
        let g = gallery();
        assert_eq!(g.variants.len(), card_fixtures::variants().len());
        assert_eq!(g.epics.len(), card_fixtures::epic_variants().len());
        assert_eq!(g.compact.len(), COMPACT_SAMPLES.len());
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
    }

    #[test]
    fn compact_rows_match_the_frame() {
        let g = gallery();
        let rows: Vec<(Option<IconName>, Option<String>)> = g
            .compact
            .iter()
            .map(|c| {
                (
                    c.compact.flag.as_ref().map(|f| f.icon),
                    c.compact.value.as_ref().map(|v| v.text.clone()),
                )
            })
            .collect();
        assert_eq!(rows[0].1.as_deref(), Some("12s"), "SHOP-22: the signal");
        assert_eq!(rows[1], (None, Some(":4020".into())), "SHOP-19: the port");
        let shop_20 = g.compact[2].compact.value.as_ref().unwrap();
        assert_eq!(
            (shop_20.icon, shop_20.text.as_str()),
            (Some(IconName::Link2), "2"),
            "SHOP-20: two parents"
        );
        assert_eq!(rows[3].1.as_deref(), Some("311k"), "SHOP-25: the tokens");
        assert_eq!(rows[4].1.as_deref(), Some("SHOP-20"), "SHOP-21: its parent");
        let flag = g.compact[0].compact.flag.as_ref().unwrap();
        assert_eq!(flag.palette, Palette::Attn);
        assert_eq!(flag.label, "Needs you");
    }

    #[test]
    fn a_marker_callout_is_the_compact_flag_when_there_is_no_pill() {
        let g = gallery();
        let card = cell(&g, "Working · validation failed · round 2 of 3");
        assert!(card.pill.is_none());
        let callout = card.callout.as_ref().unwrap();
        let flag = card.compact.flag.as_ref().unwrap();
        assert_eq!((flag.icon, &flag.label), (callout.icon, &callout.text));
    }

    #[test]
    fn bars_options_and_epic_rows_are_worded_here() {
        let g = gallery();
        let bar = &cell(&g, "Dev · message waiting").bars[0];
        assert_eq!(bar.look, BarLook::Messages);
        assert_eq!(bar.text, "1 message waiting");
        assert_eq!(bar.action.as_ref().unwrap().label, "Deliver now");
        assert_eq!(
            BarView::of(&Bar::MessagesWaiting(2)).text,
            "2 messages waiting"
        );
        assert_eq!(
            BarView::of(&Bar::Background {
                text: "wakes up at 4:10pm".into(),
                wakeup: true
            })
            .icon,
            IconName::AlarmClock
        );
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
        use crewboard_core::board::questions::HumanQuestion;
        let mut t = card_fixtures::ticket(
            "SHOP-21",
            "Order email",
            crewboard_core::domain::Status::AsksYou,
        );
        t.question = Some(HumanQuestion {
            options: vec!["Only the profile form".into(), "Everywhere".into()],
            recommended: Some(1),
            id: "q1".into(),
            text: "Split fullName everywhere?".into(),
            asked_at: NOW,
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
    }
}
