//! The "Epics in flight" strip (*01 · Components · Epics on the board (0080)* → Strip Header, Track Label, Track
//! Stop; *04 → Row · Epics in flight strip (0080)*; EPC-10, EPC-13, EPC-17; d0080). It sits above the columns
//! while at least one epic is in its **Track** place ([`epic_place`]), one **track** per epic:
//! - the track label: the epic's name, External ID, "1/5" merged of all, the mini bar (one segment per child), a
//!   pin for a pinned epic, the needs-you dot, and the "not queued" chips of its Backlog children;
//! - one **stop** per child that isn't in Backlog, in the **station** of its column, with its short state ("40.3
//!   waits 40.2", "40.2 asks you"), its "new · added by 40.2" flag (EPC-13) and a narrow form (icon + key; a
//!   needs-you stop keeps its words, *Note · Narrow window*). With a lead (d0083, T-0291; *03 → epic lead flows*):
//!   "40.2 asked lead" (calm), "40.3 lead's note" (violet), "40.3 failed" after the note (turquoise), and "new · by
//!   Wishlist lead" for a child the lead added ("new" in a narrow station).
//!
//! Order (d0047, d0080 changelog): pinned epics (most recently pinned first) → tracks with a child that needs the
//! user → the rest, by when the epic was first queued. Two states no frame draws reuse existing shapes: a parked
//! child of a track is a chip that says "Parked · has work", and a closed child is a stop under Done shaped like
//! `merged` that says "closed".
//!
//! The lead (d0083, T-0289; *Row · Track Label with a lead (0083)*): a track whose epic has a lead shows it on its
//! own line under the name, in the short form (the track grows from 30 to 56 px). A lead that asks you turns the
//! whole label turquoise (the needs-you dot too, and the header's count), and no child; its track sorts with the
//! tracks that need the user.

use std::cmp::Reverse;

use otaq_core::board::epic_flight::{EpicPlace, children_of, epic_place, is_parked};
use otaq_core::board::epics::progress_of;
use otaq_core::board::readiness::{ReadyState, readiness};
use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{Actor, AutoMergeStop, Column, Status, TicketKey};
use otaq_core::project_agents::lead::Lead;
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::theme::Status as Legend;
use crate::view_models::board_page::ColumnId;
use crate::view_models::lead_status::LeadStatusView;

/// The strip header's hint.
pub const HINT: &str = "hover a stop to find its card · click a name to focus";

/// A stop's or chip's state, and a mini-bar segment's. The page picks the colour from it; `needs_you` on the stop
/// makes it turquoise. `EpicStopState` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "EpicStopState")]
pub enum StopState {
    /// A never-started child in Backlog: a dashed chip on the track label.
    NotQueued,
    /// A parked child while the epic is a track: a chip saying "Parked · has work" (no frame draws it).
    Parked,
    /// Queued, nothing to wait for.
    Next,
    /// Queued, waiting for a dependency ("waits 40.2") or for a slot ("waits").
    Waits,
    /// In Dev: working, in background, validating.
    Working,
    /// Turquoise: every needs-you state (d0042, d0077); "asks you" for Asks you.
    NeedsYou,
    /// Merge conflict · fixing, or Keeps conflicting (turquoise then, `needs_you`).
    Conflict,
    /// Back to work after the lead's one note at its first stop: "lead's note" (violet, LEAD-07).
    LeadNote,
    /// Ready for Review (magenta).
    InReview,
    /// Done · Merged.
    Merged,
    /// Done · Closed · not merged: the `merged` shape in the closed tone (no frame draws it).
    Closed,
}

/// The strip. `None` from [`Strip::from`] when no epic is in flight. `EpicStrip` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EpicStrip")]
pub struct Strip {
    /// "Epics in flight · 3".
    pub title: String,
    /// "· 1 needs you"; hidden at 0.
    pub needs_you: Option<String>,
    pub hint: &'static str,
    /// The folded line: "Epics in flight 3 · 1 needs you".
    pub folded: String,
    pub tracks: Vec<Track>,
}

/// One epic in flight. `EpicTrack` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EpicTrack")]
pub struct Track {
    pub key: String,
    pub name: String,
    /// Hidden when empty.
    pub external_id: Option<String>,
    /// "1/5": merged of all children.
    pub count: String,
    /// One per child, in key order.
    pub segments: Vec<StopState>,
    pub pinned: bool,
    /// The turquoise dot: a child needs the user, or its lead asks you (then the whole label is turquoise).
    pub needs_you: bool,
    /// The lead's line, in the short form; `None` when the epic has no lead (d0083).
    pub lead: Option<LeadStatusView>,
    /// The lead asks you: the whole label is turquoise (border, fill, dot).
    pub lead_asks: bool,
    /// The Backlog children: "not queued" (or "Parked · has work") chips, in key order. How many fit ("+2") is the
    /// screen's choice.
    pub not_queued: Vec<Chip>,
    /// One per column the board shows, except Backlog, in board order.
    pub stations: Vec<Station>,
}

/// A chip on the track label. `EpicTrackChip` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EpicTrackChip")]
pub struct Chip {
    pub key: String,
    /// "40.3".
    pub label: String,
    pub state: StopState,
    pub icon: IconName,
    /// "SHOP-40.3 · not queued", "SHOP-40.3 · Parked · has work".
    pub tooltip: String,
    /// The button the chip shows on hover (*04 strip row → not queued · hover: Queue*): "Queue", or "Resume" for a
    /// parked child. It queues only this child, as dragging the chip to Ready for Dev does (EPC-12, T-0270).
    pub action: String,
}

/// A column's stops on a track. `EpicStation` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EpicStation")]
pub struct Station {
    pub column: ColumnId,
    /// In key order.
    pub stops: Vec<Stop>,
    /// Done only, when a child merged: "2 merged", what the station shows while Done is its 34 px rail
    /// (*Note · Narrow window*).
    pub merged: Option<String>,
}

/// A child on its track. `EpicStop` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EpicStop")]
pub struct Stop {
    pub key: String,
    pub state: StopState,
    pub icon: IconName,
    /// "waits 40.2", "asks you", "conflict"; `None` for working and merged (icon + key).
    pub note: Option<String>,
    /// "40.3 waits 40.2".
    pub label: String,
    /// At narrow widths: "40.3" (icon + key); a needs-you stop keeps "40.2 asks you".
    pub short_label: String,
    /// "SHOP-40.3 Wishlist page · waits for SHOP-40.2 to merge".
    pub tooltip: String,
    /// Turquoise.
    pub needs_you: bool,
    /// "new · added by 40.2", "new · added by you" (EPC-13); "new · by Wishlist lead" (d0083).
    pub new: Option<String>,
    /// The flag in a narrow station: "new" (*Notes · short texts*).
    pub new_short: Option<String>,
}

impl Strip {
    /// `tickets` is the project's tickets; `columns` the board's columns, in board order (the stations follow
    /// them).
    pub fn from(tickets: &[Ticket], columns: &[Column]) -> Option<Self> {
        Self::with_leads(tickets, columns, &[])
    }

    /// The strip with the project's epic `leads` on their tracks (d0083).
    pub fn with_leads(tickets: &[Ticket], columns: &[Column], leads: &[Lead]) -> Option<Self> {
        let lead_of = |epic: &Ticket| leads.iter().find(|l| l.epic == epic.key);
        let mut epics: Vec<&Ticket> = tickets
            .iter()
            .filter(|t| t.is_epic() && epic_place(t, tickets) == EpicPlace::Track)
            .collect();
        if epics.is_empty() {
            return None;
        }
        epics.sort_by_key(|epic| order_key(epic, tickets, lead_of(epic)));
        let tracks: Vec<Track> = epics
            .iter()
            .map(|epic| track(epic, tickets, columns, lead_of(epic)))
            .collect();
        let needs_you: usize = tracks
            .iter()
            .flat_map(|t| &t.stations)
            .flat_map(|s| &s.stops)
            .filter(|s| s.needs_you)
            .count()
            + tracks.iter().filter(|t| t.lead_asks).count();
        let needs_you = (needs_you > 0).then(|| format!("· {needs_you} needs you"));
        let n = tracks.len();
        Some(Self {
            title: format!("Epics in flight · {n}"),
            folded: match &needs_you {
                Some(text) => format!("Epics in flight {n} {text}"),
                None => format!("Epics in flight {n}"),
            },
            needs_you,
            hint: HINT,
            tracks,
        })
    }
}

/// Pinned (most recently pinned first) → a child needs the user → the rest, by when the epic was first queued
/// (its earliest child's queue time; the epic keeps no time of its own on the ticket); ties go to the lower key.
fn order_key(
    epic: &Ticket,
    tickets: &[Ticket],
    lead: Option<&Lead>,
) -> (u8, Reverse<i64>, Option<i64>, bool, TicketKey) {
    let children = children_of(epic, tickets);
    let queued = children
        .iter()
        .filter_map(|c| c.queued_at.or(c.started_at))
        .min();
    let group = if epic.flags.is_pinned() {
        0
    } else if children.iter().any(|c| c.shows_needs_you())
        || lead.is_some_and(|l| l.status == otaq_core::project_agents::lead::LeadStatus::AsksYou)
    {
        1
    } else {
        2
    };
    (
        group,
        Reverse(epic.flags.pinned_at.unwrap_or(0)),
        queued,
        queued.is_none(),
        epic.key.clone(),
    )
}

fn track(epic: &Ticket, tickets: &[Ticket], columns: &[Column], lead: Option<&Lead>) -> Track {
    let mut children = children_of(epic, tickets);
    children.sort_by(|a, b| a.key.cmp(&b.key));
    let progress = progress_of(&epic.key, tickets);
    let stops: Vec<(Column, Stop)> = children
        .iter()
        .filter(|c| c.column != Column::Backlog)
        .map(|c| (c.column, stop(c, tickets)))
        .collect();
    let lead = lead.map(|l| LeadStatusView::of(epic, Some(l)));
    let lead_asks = lead.as_ref().is_some_and(LeadStatusView::asks_you);
    Track {
        key: epic.key.to_string(),
        name: epic.title.clone(),
        external_id: epic.external_id.clone().filter(|e| !e.is_empty()),
        count: format!("{}/{}", progress.merged, progress.total),
        segments: children
            .iter()
            .map(|c| match c.column {
                Column::Backlog => chip_state(c),
                _ => stop_state(c, tickets).0,
            })
            .collect(),
        pinned: epic.flags.is_pinned(),
        needs_you: lead_asks || stops.iter().any(|(_, s)| s.needs_you),
        lead,
        lead_asks,
        not_queued: children
            .iter()
            .filter(|c| c.column == Column::Backlog)
            .map(|c| chip(c))
            .collect(),
        stations: columns
            .iter()
            .filter(|c| **c != Column::Backlog)
            .map(|column| Station {
                column: ColumnId::of(*column),
                stops: stops
                    .iter()
                    .filter(|(c, _)| c == column)
                    .map(|(_, s)| s.clone())
                    .collect(),
                merged: (*column == Column::Done && progress.merged > 0)
                    .then(|| format!("{} merged", progress.merged)),
            })
            .collect(),
    }
}

fn chip_state(child: &Ticket) -> StopState {
    if is_parked(child) {
        StopState::Parked
    } else {
        StopState::NotQueued
    }
}

fn chip(child: &Ticket) -> Chip {
    let state = chip_state(child);
    let (icon, words, action) = match state {
        StopState::Parked => (Legend::Parked.spec().2, Status::Parked.label(), "Resume"),
        _ => (IconName::CircleDashed, "not queued", "Queue"),
    };
    Chip {
        key: child.key.to_string(),
        label: short_key(&child.key),
        state,
        icon,
        tooltip: format!("{} · {words}", child.key),
        action: action.to_owned(),
    }
}

/// "40.2" for an epic child; other keys stay whole ("SHOP-18").
fn short_key(key: &TicketKey) -> String {
    match key.child {
        Some(child) => format!("{}.{child}", key.number),
        None => key.to_string(),
    }
}

/// The state, its icon and its words.
fn stop_state(child: &Ticket, tickets: &[Ticket]) -> (StopState, IconName, Option<String>) {
    let s = child.status;
    let conflict = s == Status::MergeConflictFixing
        || (s == Status::AutoMergeStopped
            && child.auto_merge_stop == Some(AutoMergeStop::KeepsConflicting));
    if conflict {
        return (
            StopState::Conflict,
            IconName::GitCompareArrows,
            Some("conflict".into()),
        );
    }
    if s.is_needs_you_family() {
        let words = match s {
            Status::AsksYou => "asks you",
            // *03 → 25*: it failed again after the lead's note.
            Status::ValidationFailed => "failed",
            _ => "needs you",
        };
        let (legend, _) = crate::view_models::status::pill(s);
        return (StopState::NeedsYou, legend.spec().2, Some(words.into()));
    }
    match child.column {
        Column::ReadyForDev => match readiness(child, tickets) {
            ReadyState::Waiting { parents } => (
                StopState::Waits,
                IconName::Hourglass,
                Some(format!("waits {}", short_key(&parents[0].key))),
            ),
            ReadyState::ParentClosed { parents } => (
                StopState::Waits,
                IconName::Hourglass,
                Some(format!("waits {}", short_key(&parents[0]))),
            ),
            ReadyState::Ready if s == Status::Queued => {
                (StopState::Next, IconName::Hourglass, Some("next".into()))
            }
            // A slot, the plan limit, a diverged base, an interrupted start: the tooltip names it (Q56).
            ReadyState::Ready => (StopState::Waits, IconName::Hourglass, Some("waits".into())),
        },
        Column::ReadyForReview | Column::Approved => (
            StopState::InReview,
            Legend::ReadyForReview.spec().2,
            Some("in review".into()),
        ),
        Column::Done if s == Status::ClosedNotMerged => (
            StopState::Closed,
            Legend::ClosedNotMerged.spec().2,
            Some("closed".into()),
        ),
        Column::Done => (StopState::Merged, IconName::Check, None),
        // LEAD-06: its question waits for the lead (*03 → 21*).
        Column::Dev if child.flags.asked_lead.is_some() => {
            (StopState::Working, IconName::Bot, Some("asked lead".into()))
        }
        // LEAD-07: back at work after the lead's note (*03 → 24*).
        Column::Dev
            if child.flags.lead_note_sent.is_some()
                && matches!(s, Status::Working | Status::WorkingInBackground) =>
        {
            (
                StopState::LeadNote,
                IconName::Bot,
                Some("lead's note".into()),
            )
        }
        // Dev without a needs-you state: working, in background, validating; also Interrupted, Resume failed and
        // Waiting for limit reset, whose tooltip names them (Q56).
        _ => (StopState::Working, IconName::Activity, None),
    }
}

fn stop(child: &Ticket, tickets: &[Ticket]) -> Stop {
    let (state, icon, note) = stop_state(child, tickets);
    let key = short_key(&child.key);
    let label = match &note {
        Some(note) => format!("{key} {note}"),
        None => key.clone(),
    };
    let needs_you = child.shows_needs_you();
    let detail = match readiness(child, tickets).reason() {
        Some(reason) if child.column == Column::ReadyForDev => reason,
        _ => child.status.label().to_owned(),
    };
    let by_lead = child.flags.by_lead.as_ref().filter(|b| b.added);
    let new = child.joined_queue_by.as_ref().map(|by| {
        if let (Actor::ProjectAgent(_), Some(lead)) = (by, by_lead) {
            return format!("new · by {}", lead.lead);
        }
        let by = match by {
            Actor::User(_) => "you".to_owned(),
            Actor::Agent(key) => short_key(key),
            Actor::ProjectAgent(_) => Legend::ProjectAgent
                .label()
                .trim_start_matches("by ")
                .to_owned(),
        };
        format!("new · added by {by}")
    });
    let mut tooltip = format!("{} {} · {detail}", child.key, child.title);
    if let Some(new) = &new {
        tooltip.push_str(&format!(" · {new}"));
    }
    Stop {
        key: child.key.to_string(),
        state,
        icon,
        short_label: if needs_you { label.clone() } else { key },
        label,
        note,
        tooltip,
        needs_you,
        new_short: new.as_ref().map(|_| "new".to_owned()),
        new,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use otaq_core::domain::Kind;

    fn epic(key: &str, title: &str, external: &str) -> Ticket {
        let mut t = ticket(key, title, Status::Backlog);
        t.kind = Kind::Epic;
        t.external_id = Some(external.into());
        t
    }

    fn child(key: &str, status: Status) -> Ticket {
        let mut t = ticket(key, &format!("{key} title"), status);
        t.epic = Some(key.parse::<TicketKey>().unwrap().epic().unwrap());
        t.base = otaq_core::board::tickets::Base::Epic(t.epic.clone().unwrap());
        if status.column() != Column::Backlog {
            t.queued_at = Some(1_000);
        }
        t
    }

    fn deps(mut t: Ticket, on: &[&str]) -> Ticket {
        t.depends_on = on.iter().map(|k| k.parse().unwrap()).collect();
        t
    }

    fn at(mut t: Ticket, queued: i64) -> Ticket {
        t.queued_at = Some(queued);
        t
    }

    /// The 04 strip row: Search (pinned, 45.1 next, 45.2 waits), Wishlist (40.2 asks you, 40.4 conflict, 40.5
    /// waits 40.2 and new, 40.1 merged, 40.3 not queued), Gift cards (60.2 waits 60.1, 60.1 working, 60.3 not
    /// queued).
    fn sample() -> Vec<Ticket> {
        let mut search = epic("SHOP-45", "Search", "PAY-130");
        search.flags.pinned_at = Some(5);
        // The frame's 40.5 reads "waits 40.1" next to a merged 40.1, which readiness can't produce: here it
        // waits for 40.2.
        let mut new = deps(
            at(child("SHOP-40.5", Status::Queued), 2_000),
            &["SHOP-40.2"],
        );
        new.joined_queue_by = Some(Actor::Agent("SHOP-40.2".parse().unwrap()));
        vec![
            search,
            at(child("SHOP-45.1", Status::Queued), 3_000),
            at(child("SHOP-45.2", Status::WaitingForSlot), 3_000),
            epic("SHOP-40", "Wishlist", "PAY-123"),
            child("SHOP-40.1", Status::Merged),
            at(child("SHOP-40.2", Status::AsksYou), 500),
            child("SHOP-40.3", Status::Backlog),
            child("SHOP-40.4", Status::MergeConflictFixing),
            new,
            epic("SHOP-60", "Gift cards", "PAY-150"),
            at(child("SHOP-60.1", Status::Working), 100),
            deps(child("SHOP-60.2", Status::Queued), &["SHOP-60.1"]),
            child("SHOP-60.3", Status::Backlog),
        ]
    }

    fn stops(track: &Track) -> Vec<(ColumnId, String)> {
        track
            .stations
            .iter()
            .flat_map(|s| s.stops.iter().map(|stop| (s.column, stop.label.clone())))
            .collect()
    }

    fn find<'a>(strip: &'a Strip, key: &str) -> &'a Stop {
        strip
            .tracks
            .iter()
            .flat_map(|t| &t.stations)
            .flat_map(|s| &s.stops)
            .find(|s| s.key == key)
            .unwrap()
    }

    #[test]
    fn the_04_strip_row() {
        let strip = Strip::from(&sample(), Column::ALL).unwrap();
        assert_eq!(strip.title, "Epics in flight · 3");
        assert_eq!(strip.needs_you.as_deref(), Some("· 1 needs you"));
        assert_eq!(strip.folded, "Epics in flight 3 · 1 needs you");
        assert_eq!(strip.hint, HINT);
        let names: Vec<_> = strip.tracks.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Search", "Wishlist", "Gift cards"]);

        let [search, wishlist, gift] = &strip.tracks[..] else {
            panic!()
        };
        assert!(search.pinned && !search.needs_you);
        assert_eq!(search.count, "0/2");
        assert_eq!(search.external_id.as_deref(), Some("PAY-130"));
        assert_eq!(
            stops(search),
            [
                (ColumnId::ReadyForDev, "45.1 next".to_owned()),
                (ColumnId::ReadyForDev, "45.2 waits".to_owned()),
            ]
        );

        assert!(wishlist.needs_you && !wishlist.pinned);
        assert_eq!(wishlist.count, "1/5");
        assert_eq!(
            wishlist.segments,
            [
                StopState::Merged,
                StopState::NeedsYou,
                StopState::NotQueued,
                StopState::Conflict,
                StopState::Waits
            ]
        );
        assert_eq!(
            stops(wishlist),
            [
                (ColumnId::ReadyForDev, "40.5 waits 40.2".to_owned()),
                (ColumnId::Dev, "40.2 asks you".to_owned()),
                (ColumnId::Dev, "40.4 conflict".to_owned()),
                (ColumnId::Done, "40.1".to_owned()),
            ]
        );
        assert_eq!(wishlist.not_queued.len(), 1);
        assert_eq!(wishlist.not_queued[0].label, "40.3");
        assert_eq!(wishlist.not_queued[0].tooltip, "SHOP-40.3 · not queued");
        assert_eq!(wishlist.not_queued[0].icon, IconName::CircleDashed);

        assert_eq!(gift.count, "0/3");
        assert_eq!(
            stops(gift),
            [
                (ColumnId::ReadyForDev, "60.2 waits 60.1".to_owned()),
                (ColumnId::Dev, "60.1".to_owned()),
            ]
        );
        assert_eq!(gift.not_queued[0].label, "60.3");
        // A station per column but Backlog, empty ones too.
        let columns: Vec<_> = gift.stations.iter().map(|s| s.column).collect();
        assert_eq!(
            columns,
            [
                ColumnId::ReadyForDev,
                ColumnId::Dev,
                ColumnId::ReadyForReview,
                ColumnId::Approved,
                ColumnId::Done
            ]
        );
        // The Done station's words for the Done rail (T-0269).
        let merged = |t: &Track| {
            t.stations
                .iter()
                .map(|s| s.merged.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            merged(wishlist),
            [None, None, None, None, Some("1 merged".into())]
        );
        assert_eq!(merged(gift), [None, None, None, None, None]);
    }

    #[test]
    fn every_stop_state() {
        let strip = Strip::from(&sample(), Column::ALL).unwrap();
        let next = find(&strip, "SHOP-45.1");
        assert_eq!(
            (next.state, next.icon, next.note.as_deref()),
            (StopState::Next, IconName::Hourglass, Some("next"))
        );
        assert_eq!(next.short_label, "45.1");

        let waits = find(&strip, "SHOP-60.2");
        assert_eq!(waits.state, StopState::Waits);
        assert_eq!(waits.short_label, "60.2");
        assert_eq!(
            waits.tooltip,
            "SHOP-60.2 SHOP-60.2 title · waits for SHOP-60.1 to merge"
        );

        let working = find(&strip, "SHOP-60.1");
        assert_eq!(
            (working.state, working.icon, working.note.as_deref()),
            (StopState::Working, IconName::Activity, None)
        );
        assert_eq!(working.tooltip, "SHOP-60.1 SHOP-60.1 title · Working");

        let asks = find(&strip, "SHOP-40.2");
        assert_eq!(asks.state, StopState::NeedsYou);
        assert_eq!(asks.icon, IconName::MessageCircleQuestionMark);
        assert!(asks.needs_you);
        assert_eq!(
            asks.short_label, "40.2 asks you",
            "needs-you stops keep their words"
        );

        let conflict = find(&strip, "SHOP-40.4");
        assert_eq!(
            (conflict.state, conflict.icon, conflict.needs_you),
            (StopState::Conflict, IconName::GitCompareArrows, false)
        );
        assert_eq!(conflict.short_label, "40.4");

        let merged = find(&strip, "SHOP-40.1");
        assert_eq!(
            (merged.state, merged.icon, merged.note.as_deref()),
            (StopState::Merged, IconName::Check, None)
        );

        let new = find(&strip, "SHOP-40.5");
        assert_eq!(new.new.as_deref(), Some("new · added by 40.2"));
        assert!(new.tooltip.ends_with(" · new · added by 40.2"));
        assert_eq!(find(&strip, "SHOP-45.1").new, None);
    }

    /// *03 → epic lead flows 20, 21, 24, 25* and the 1280 check (T-0291, d0083): the lead's stops.
    #[test]
    fn the_leads_stops() {
        use crate::view_models::card_fixtures::{WISHLIST_LEAD, lead_note};
        let mut new = at(child("SHOP-40.5", Status::Queued), 2_000);
        let lead = Actor::ProjectAgent("lead-1".into());
        new.created_by = lead.clone();
        new.joined_queue_by = Some(lead);
        new.flags.by_lead = Some(otaq_core::domain::ByLead {
            lead: WISHLIST_LEAD.into(),
            added: true,
            ..Default::default()
        });
        let mut asked = at(child("SHOP-40.2", Status::Working), 500);
        asked.flags.asked_lead = Some(otaq_core::domain::AskedLead {
            lead: WISHLIST_LEAD.into(),
            question: "Should the button show a filled heart?".into(),
        });
        let mut noted = at(child("SHOP-40.3", Status::Working), 600);
        noted.flags.lead_note_sent = Some(lead_note());
        let mut failed = at(child("SHOP-40.4", Status::ValidationFailed), 700);
        failed.flags.lead_note_sent = Some(lead_note());
        let tickets = vec![
            epic("SHOP-40", "Wishlist", "PAY-123"),
            asked,
            noted,
            failed,
            new,
        ];
        let strip = Strip::from(&tickets, Column::ALL).unwrap();
        let stop = |key| {
            let s = find(&strip, key);
            (s.state, s.icon, s.label.clone(), s.needs_you)
        };
        assert_eq!(
            stop("SHOP-40.2"),
            (
                StopState::Working,
                IconName::Bot,
                "40.2 asked lead".into(),
                false
            )
        );
        assert_eq!(
            stop("SHOP-40.3"),
            (
                StopState::LeadNote,
                IconName::Bot,
                "40.3 lead's note".into(),
                false
            )
        );
        assert_eq!(
            stop("SHOP-40.4"),
            (
                StopState::NeedsYou,
                IconName::ShieldX,
                "40.4 failed".into(),
                true
            )
        );
        let new = find(&strip, "SHOP-40.5");
        assert_eq!(new.new.as_deref(), Some("new · by Wishlist lead"));
        assert_eq!(new.new_short.as_deref(), Some("new"), "1280 px");
        assert!(new.tooltip.ends_with(" · new · by Wishlist lead"));
    }

    #[test]
    fn needs_you_review_and_conflict_variants() {
        let mut tickets = sample();
        let set = |tickets: &mut Vec<Ticket>, key: &str, status: Status| {
            let t = tickets
                .iter_mut()
                .find(|t| t.key.to_string() == key)
                .unwrap();
            t.status = status;
            t.column = status.column();
        };
        set(&mut tickets, "SHOP-40.2", Status::NeedsYou);
        set(&mut tickets, "SHOP-60.1", Status::ReadyForReview);
        set(&mut tickets, "SHOP-40.4", Status::AutoMergeStopped);
        tickets
            .iter_mut()
            .find(|t| t.key.to_string() == "SHOP-40.4")
            .unwrap()
            .auto_merge_stop = Some(AutoMergeStop::KeepsConflicting);
        tickets
            .iter_mut()
            .find(|t| t.key.to_string() == "SHOP-40.5")
            .unwrap()
            .joined_queue_by = Some(Actor::User("me".into()));
        let strip = Strip::from(&tickets, Column::ALL).unwrap();

        let needs = find(&strip, "SHOP-40.2");
        assert_eq!(needs.label, "40.2 needs you");
        assert_eq!(needs.icon, IconName::Hand);

        let review = find(&strip, "SHOP-60.1");
        assert_eq!(review.state, StopState::InReview);
        assert_eq!(review.label, "60.1 in review");
        assert_eq!(stops(&strip.tracks[2])[1].0, ColumnId::ReadyForReview);

        let keeps = find(&strip, "SHOP-40.4");
        assert_eq!(keeps.state, StopState::Conflict);
        assert!(keeps.needs_you, "Keeps conflicting is turquoise");
        assert_eq!(keeps.short_label, "40.4 conflict");
        assert_eq!(strip.needs_you.as_deref(), Some("· 2 needs you"));

        assert_eq!(
            find(&strip, "SHOP-40.5").new.as_deref(),
            Some("new · added by you")
        );
    }

    #[test]
    fn a_pinned_epic_comes_first_even_above_a_needs_you_track() {
        let mut tickets = sample();
        // Unpin Search; pin Gift cards, which needs nobody, below Wishlist, which needs you.
        tickets[0].flags.pinned_at = None;
        tickets[9].flags.pinned_at = Some(9);
        let strip = Strip::from(&tickets, Column::ALL).unwrap();
        let names: Vec<_> = strip.tracks.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Gift cards", "Wishlist", "Search"]);

        // Two pins: the most recent first.
        tickets[0].flags.pinned_at = Some(10);
        let strip = Strip::from(&tickets, Column::ALL).unwrap();
        let names: Vec<_> = strip.tracks.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Search", "Gift cards", "Wishlist"]);
    }

    #[test]
    fn the_rest_go_by_when_the_epic_was_queued() {
        let mut tickets = sample();
        tickets[0].flags.pinned_at = None;
        set_status(&mut tickets, "SHOP-40.2", Status::Working);
        let strip = Strip::from(&tickets, Column::ALL).unwrap();
        // Gift cards first queued at 100, Wishlist at 500, Search at 3000.
        let names: Vec<_> = strip.tracks.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Gift cards", "Wishlist", "Search"]);
        assert_eq!(strip.needs_you, None);
        assert_eq!(strip.folded, "Epics in flight 3");
    }

    fn set_status(tickets: &mut [Ticket], key: &str, status: Status) {
        let t = tickets
            .iter_mut()
            .find(|t| t.key.to_string() == key)
            .unwrap();
        t.status = status;
        t.column = status.column();
    }

    #[test]
    fn a_single_mode_track_with_a_not_queued_chip() {
        let mut search = epic("SHOP-45", "Search", "PAY-130");
        search.epic_queue_mode = otaq_core::board::epic_flight::EpicQueueMode::Single;
        let tickets = vec![
            search,
            child("SHOP-45.1", Status::Queued),
            child("SHOP-45.2", Status::Backlog),
        ];
        let strip = Strip::from(&tickets, Column::ALL).unwrap();
        assert_eq!(strip.title, "Epics in flight · 1");
        let track = &strip.tracks[0];
        assert_eq!(track.count, "0/2");
        assert_eq!(
            stops(track),
            [(ColumnId::ReadyForDev, "45.1 next".to_owned())]
        );
        assert_eq!(track.not_queued.len(), 1);
        assert_eq!(track.not_queued[0].state, StopState::NotQueued);
        assert_eq!(track.not_queued[0].tooltip, "SHOP-45.2 · not queued");
        assert_eq!(track.not_queued[0].action, "Queue", "hover: Queue (T-0270)");
        assert_eq!(track.segments, [StopState::Next, StopState::NotQueued]);
    }

    #[test]
    fn no_strip_without_a_track() {
        // Nothing started: the epic is a card in Backlog.
        let tickets = vec![
            epic("SHOP-45", "Search", "PAY-130"),
            child("SHOP-45.1", Status::Backlog),
            child("SHOP-45.2", Status::Backlog),
        ];
        assert_eq!(Strip::from(&tickets, Column::ALL), None);
        // The epic in Review: a delivery card.
        let mut tickets = tickets;
        tickets[0].status = Status::ReadyForReview;
        tickets[0].column = Column::ReadyForReview;
        set_status(&mut tickets, "SHOP-45.1", Status::Merged);
        set_status(&mut tickets, "SHOP-45.2", Status::Merged);
        assert_eq!(Strip::from(&tickets, Column::ALL), None);
        // A standalone ticket only.
        assert_eq!(
            Strip::from(&[ticket("SHOP-51", "Fix", Status::Working)], Column::ALL),
            None
        );
    }

    #[test]
    fn gap_a_parked_child_while_the_epic_is_a_track() {
        let tickets = vec![
            epic("SHOP-40", "Wishlist", "PAY-123"),
            child("SHOP-40.1", Status::Working),
            child("SHOP-40.2", Status::Parked),
            child("SHOP-40.3", Status::Backlog),
        ];
        let strip = Strip::from(&tickets, Column::ALL).unwrap();
        let track = &strip.tracks[0];
        let chips: Vec<_> = track
            .not_queued
            .iter()
            .map(|c| (c.label.as_str(), c.state, c.tooltip.as_str()))
            .collect();
        assert_eq!(
            chips,
            [
                ("40.2", StopState::Parked, "SHOP-40.2 · Parked · has work"),
                ("40.3", StopState::NotQueued, "SHOP-40.3 · not queued"),
            ]
        );
        assert_eq!(track.not_queued[0].icon, Legend::Parked.spec().2);
        assert_eq!(track.not_queued[0].action, "Resume");
    }

    #[test]
    fn gap_a_closed_child_is_a_closed_stop_under_done() {
        let tickets = vec![
            epic("SHOP-40", "Wishlist", "PAY-123"),
            child("SHOP-40.1", Status::Merged),
            child("SHOP-40.2", Status::ClosedNotMerged),
            child("SHOP-40.3", Status::Working),
        ];
        let strip = Strip::from(&tickets, Column::ALL).unwrap();
        let closed = find(&strip, "SHOP-40.2");
        assert_eq!(
            (closed.state, closed.icon, closed.label.as_str()),
            (StopState::Closed, IconName::CircleSlash, "40.2 closed")
        );
        assert_eq!(
            stops(&strip.tracks[0]),
            [
                (ColumnId::Dev, "40.3".to_owned()),
                (ColumnId::Done, "40.1".to_owned()),
                (ColumnId::Done, "40.2 closed".to_owned()),
            ]
        );
        assert_eq!(strip.tracks[0].count, "1/3");
    }

    #[test]
    fn a_stop_waits_for_the_first_blocker_even_outside_the_epic() {
        let tickets = vec![
            epic("SHOP-40", "Wishlist", "PAY-123"),
            deps(
                child("SHOP-40.1", Status::Queued),
                &["SHOP-40.2", "SHOP-18"],
            ),
            child("SHOP-40.2", Status::Working),
            ticket("SHOP-18", "Login", Status::Working),
        ];
        let strip = Strip::from(&tickets, Column::ALL).unwrap();
        assert_eq!(find(&strip, "SHOP-40.1").label, "40.1 waits SHOP-18");
    }

    #[test]
    fn states_no_frame_draws_reuse_working_and_waits() {
        // Q56: an interrupted child keeps the working shape, an interrupted start the "waits" one; the tooltip
        // names the status.
        let tickets = vec![
            epic("SHOP-40", "Wishlist", "PAY-123"),
            child("SHOP-40.1", Status::Interrupted),
            child("SHOP-40.2", Status::StartInterrupted),
        ];
        let strip = Strip::from(&tickets, Column::ALL).unwrap();
        let interrupted = find(&strip, "SHOP-40.1");
        assert_eq!(interrupted.state, StopState::Working);
        assert!(!interrupted.needs_you);
        assert_eq!(
            interrupted.tooltip,
            "SHOP-40.1 SHOP-40.1 title · Interrupted"
        );
        let start = find(&strip, "SHOP-40.2");
        assert_eq!(
            (start.state, start.label.as_str()),
            (StopState::Waits, "40.2 waits")
        );
        assert_eq!(
            start.tooltip,
            "SHOP-40.2 SHOP-40.2 title · Start interrupted"
        );
        assert_eq!(strip.needs_you, None);
    }

    /// *Row · Track Label with a lead (0083)*, frames 19 and 26: the lead's short form on its own line; a lead that
    /// asks you turns the whole label turquoise (the dot and the header count) and moves its track up, no child.
    #[test]
    fn the_lead_line_and_the_turquoise_rule() {
        use crate::view_models::lead_status::sample as lead;
        use otaq_core::project_agents::lead::LeadStatus;

        let tickets = vec![
            epic("SHOP-60", "Gift cards", "PAY-150"),
            at(child("SHOP-60.1", Status::Working), 100),
            epic("SHOP-40", "Wishlist", "PAY-123"),
            at(child("SHOP-40.2", Status::Working), 500),
            child("SHOP-40.3", Status::Backlog),
        ];
        let wishlist = &tickets[2];
        let replanning = lead(wishlist, LeadStatus::Replanning, 0);
        let strip = Strip::with_leads(&tickets, Column::ALL, &[replanning]).unwrap();
        let keys: Vec<&str> = strip.tracks.iter().map(|t| t.key.as_str()).collect();
        assert_eq!(keys, ["SHOP-60", "SHOP-40"], "first queued first");
        assert_eq!(strip.tracks[0].lead, None, "no lead: no second line");
        let line = strip.tracks[1].lead.as_ref().unwrap();
        assert_eq!(line.short, "re-planning");
        assert!(!strip.tracks[1].needs_you);
        assert_eq!(strip.needs_you, None);

        let answering = lead(
            wishlist,
            LeadStatus::Answering {
                ticket: "SHOP-40.2".parse().unwrap(),
            },
            0,
        );
        let strip = Strip::with_leads(&tickets, Column::ALL, &[answering]).unwrap();
        assert_eq!(
            strip.tracks[1].lead.as_ref().unwrap().short,
            "answering 40.2"
        );

        let asks = lead(wishlist, LeadStatus::AsksYou, 0);
        let strip = Strip::with_leads(&tickets, Column::ALL, &[asks]).unwrap();
        let keys: Vec<&str> = strip.tracks.iter().map(|t| t.key.as_str()).collect();
        assert_eq!(
            keys,
            ["SHOP-40", "SHOP-60"],
            "a lead that asks you needs you"
        );
        assert!(strip.tracks[0].needs_you);
        assert!(strip.tracks[0].lead_asks);
        assert!(!strip.tracks[1].lead_asks);
        assert_eq!(strip.tracks[0].lead.as_ref().unwrap().short, "asks you");
        assert_eq!(strip.needs_you.as_deref(), Some("· 1 needs you"));
        assert!(
            strip.tracks[0]
                .stations
                .iter()
                .flat_map(|s| &s.stops)
                .all(|s| !s.needs_you),
            "no child turns turquoise for it"
        );

        let ended = lead(wishlist, LeadStatus::EndedByRestart, 0);
        let strip = Strip::with_leads(&tickets, Column::ALL, &[ended]).unwrap();
        let line = strip.tracks[1].lead.as_ref().unwrap();
        assert_eq!(line.short, "ended by restart");
        assert_eq!(line.action.as_ref().unwrap().label, "Resume");
    }
}
