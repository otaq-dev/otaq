//! The **Lead Status** (LEAD-11, LEAD-02, LEAD-12; d0083; *01 · Components · Epics on the board → Row · Lead Status
//! (0083)*): the epic lead's icon, name and state, in two forms.
//! - **Full form** ("Wishlist lead · re-planning") on the Backlog epic card, in the epic panel, inbox items and
//!   tooltips.
//! - **Short form** (icon + state, "answering 40.2") on the lead's own line in the track label, where the epic's
//!   name is already on the line above.
//!
//! The tone: working states are violet text, idle is quiet (its icon stays violet), "asks you" is turquoise, the
//! ended states are quiet with a quiet icon. "ended by restart" and "ended" offer **Resume**; an epic with no lead
//! offers the labelled button **Open lead** (its click is the go-ahead for the lead's tokens). "planning" isn't in
//! the master row; frame 15 draws it with the same component.
//!
//! The tooltip (*03 → Interactions (0083)*) holds the full form, what woke it ("SHOP-40.1 merged") and its tokens
//! today; "ended by restart", "asks you" and "Open lead" say what a click does instead (frames 14, 26, *16 → Epic
//! lead after a restart*).

use otaq_core::board::tickets::Ticket;
use otaq_core::domain::TicketKey;
use otaq_core::project_agents::lead::{Lead, LeadStatus, lead_name};
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::view_models::card::tokens_label;

pub const OPEN_LEAD: &str = "Open lead";
pub const RESUME: &str = "Resume";
/// The hint every live lead's tooltip ends with (frame 17).
pub const CLICK_HINT: &str = "Click: open the epic panel on its Lead tab.";

/// The colour of the state. `LeadTone` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "LeadTone")]
pub enum Tone {
    /// Checking the plan, planning, re-planning, answering: violet text.
    Working,
    /// Quiet text, violet icon.
    Idle,
    /// Turquoise: its own question or a permission prompt.
    AsksYou,
    /// Ended by restart, ended, ended · epic in review: quiet text and icon.
    Ended,
    /// No lead yet: the "Open lead" button alone.
    None,
}

/// What the status's button does. `LeadActionKind` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "LeadActionKind")]
pub enum ActionKind {
    /// `lead.open`, then the epic panel.
    Open,
    /// `lead.resume`: the same conversation.
    Resume,
}

/// The button inside the status. `LeadButton` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "LeadButton")]
pub struct Button {
    pub kind: ActionKind,
    pub label: String,
    pub icon: IconName,
}

/// The status's tooltip: a title line and the lines under it. `LeadTooltip` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "LeadTooltip")]
pub struct Tooltip {
    pub title: String,
    pub lines: Vec<String>,
}

/// The Lead Status of one epic. `LeadStatusView` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "LeadStatusView")]
pub struct LeadStatusView {
    /// The epic's key: what Open lead, Resume and a click act on.
    pub epic: String,
    /// "Wishlist lead"; `None` when the epic has no lead (Open lead).
    pub name: Option<String>,
    /// The full form's state: "re-planning", "answering SHOP-40.2"; empty with no lead.
    pub state: String,
    /// The short form's state: "answering 40.2", "ended · in review"; empty with no lead.
    pub short: String,
    /// "Wishlist lead · re-planning", or "Open lead".
    pub full: String,
    pub tone: Tone,
    pub icon: IconName,
    /// Resume (ended by restart, ended) or Open lead (no lead).
    pub action: Option<Button>,
    pub tooltip: Tooltip,
    /// A live lead: a click on the status opens the epic panel on its Lead tab.
    pub opens_tab: bool,
}

impl LeadStatusView {
    /// The status of `epic`, whose lead is `lead` (`None` → Open lead).
    pub fn of(epic: &Ticket, lead: Option<&Lead>) -> Self {
        let Some(lead) = lead.filter(|l| l.status != LeadStatus::None) else {
            return Self::open_lead(epic);
        };
        let (state, short) = words(&lead.status);
        let tone = tone(&lead.status);
        let action =
            matches!(lead.status, LeadStatus::EndedByRestart | LeadStatus::Ended).then(|| Button {
                kind: ActionKind::Resume,
                label: RESUME.to_owned(),
                icon: IconName::RotateCcw,
            });
        let full = format!("{} \u{b7} {state}", lead.name);
        Self {
            epic: epic.key.to_string(),
            name: Some(lead.name.clone()),
            tooltip: tooltip(epic, lead, &full),
            state,
            short,
            full,
            tone,
            icon: IconName::Bot,
            action,
            opens_tab: true,
        }
    }

    fn open_lead(epic: &Ticket) -> Self {
        Self {
            epic: epic.key.to_string(),
            name: None,
            state: String::new(),
            short: String::new(),
            full: OPEN_LEAD.to_owned(),
            tone: Tone::None,
            icon: IconName::Bot,
            action: Some(Button {
                kind: ActionKind::Open,
                label: OPEN_LEAD.to_owned(),
                icon: IconName::Bot,
            }),
            tooltip: Tooltip {
                title: OPEN_LEAD.to_owned(),
                lines: vec![format!(
                    "Starts the {}: a read-only agent that plans with you. Its tokens count on the epic.",
                    lead_name(&epic.title)
                )],
            },
            opens_tab: false,
        }
    }

    /// The form on a review card (frame 27, *Notes · short texts*): "Wishlist lead · ended · in review".
    pub fn on_review_card(mut self) -> Self {
        if let Some(name) = &self.name {
            self.full = format!("{name} \u{b7} {}", self.short);
        }
        self
    }

    /// The lead waits for the user: the card's border and the whole track label turn turquoise.
    pub fn asks_you(&self) -> bool {
        self.tone == Tone::AsksYou
    }
}

/// The full and the short form's words for a status.
pub fn words(status: &LeadStatus) -> (String, String) {
    let both = |s: &str| (s.to_owned(), s.to_owned());
    match status {
        LeadStatus::None => (String::new(), String::new()),
        LeadStatus::Idle => both("idle"),
        LeadStatus::CheckingPlan => both("checking the plan"),
        LeadStatus::Planning => both("planning"),
        LeadStatus::Replanning => both("re-planning"),
        LeadStatus::Answering { ticket } => (
            format!("answering {ticket}"),
            format!("answering {}", short_key(ticket)),
        ),
        LeadStatus::AsksYou => both("asks you"),
        LeadStatus::EndedByRestart => both("ended by restart"),
        LeadStatus::EndedInReview => (
            "ended \u{b7} epic in review".to_owned(),
            "ended \u{b7} in review".to_owned(),
        ),
        LeadStatus::Ended => both("ended"),
    }
}

pub fn tone(status: &LeadStatus) -> Tone {
    match status {
        LeadStatus::None => Tone::None,
        LeadStatus::Idle => Tone::Idle,
        LeadStatus::CheckingPlan
        | LeadStatus::Planning
        | LeadStatus::Replanning
        | LeadStatus::Answering { .. } => Tone::Working,
        LeadStatus::AsksYou => Tone::AsksYou,
        LeadStatus::EndedByRestart | LeadStatus::EndedInReview | LeadStatus::Ended => Tone::Ended,
    }
}

fn tooltip(epic: &Ticket, lead: &Lead, full: &str) -> Tooltip {
    match &lead.status {
        // Frame 26.
        LeadStatus::AsksYou => {
            let mut lines = Vec::new();
            if let Some(q) = &lead.agent.question {
                let options: Vec<String> = q
                    .options
                    .iter()
                    .enumerate()
                    .map(|(i, o)| format!("{} {o}", i + 1))
                    .collect();
                let mut line = format!("\u{201c}{}\u{201d}", q.text.trim());
                if !options.is_empty() {
                    line.push_str(&format!(" \u{b7} {}", options.join(" \u{b7} ")));
                }
                lines.push(line);
            } else if let Some(prompt) = &lead.agent.needs_you {
                lines.push(prompt.clone());
            }
            lines.push("Click to answer in the Lead tab.".to_owned());
            Tooltip {
                title: format!("{} asks you", lead.name),
                lines,
            }
        }
        // *16 → Epic lead after a restart*.
        LeadStatus::EndedByRestart => Tooltip {
            title: full.to_owned(),
            lines: vec![format!(
                "Resume continues the same conversation. Until then, {}'s children ask you directly.",
                epic.key
            )],
        },
        LeadStatus::Ended => Tooltip {
            title: full.to_owned(),
            lines: vec!["Resume continues the same conversation.".to_owned()],
        },
        _ => {
            let mut lines = Vec::new();
            if let Some(woke) = lead.woke_by.as_deref().map(woke_words) {
                lines.push(woke);
            }
            lines.push(format!("{} today", tokens_label(lead.tokens_today.shown())));
            lines.push(CLICK_HINT.to_owned());
            Tooltip {
                title: full.to_owned(),
                lines,
            }
        }
    }
}

/// What woke it, from the wake-up's text: "SHOP-40.1 merged: Wishlist API" → "SHOP-40.1 merged".
pub fn woke_words(text: &str) -> String {
    text.split_once(": ")
        .map_or(text, |(head, _)| head)
        .trim()
        .to_owned()
}

/// "40.2" for an epic child; other keys stay whole.
fn short_key(key: &TicketKey) -> String {
    match key.child {
        Some(child) => format!("{}.{child}", key.number),
        None => key.to_string(),
    }
}

/// A lead as the service would report it, for tests and the gallery: `epic`'s lead in `status`, with `tokens`
/// today.
pub fn sample(epic: &Ticket, status: LeadStatus, tokens: u64) -> Lead {
    use otaq_core::project_agents::{AgentState, ProjectAgent};
    let state = match status {
        LeadStatus::EndedByRestart => AgentState::EndedByRestart,
        LeadStatus::EndedInReview | LeadStatus::Ended => AgentState::Ended,
        _ => AgentState::Running,
    };
    let name = lead_name(&epic.title);
    Lead {
        epic: epic.key.clone(),
        epic_id: epic.id.clone(),
        name: name.clone(),
        status,
        agent: ProjectAgent {
            id: format!("lead-{}", epic.key),
            project: epic.key.project.to_string(),
            name,
            name_source: None,
            state,
            claude_session_id: "s".into(),
            got_request: true,
            mid_turn: false,
            limit_stopped: false,
            created_at: 0,
            ended_at: None,
            closed: false,
            question: None,
            needs_you: None,
            focused: false,
            tokens: Default::default(),
            session: None,
            epic: Some(epic.key.clone()),
        },
        woke_by: None,
        tokens_today: otaq_core::usage::Tokens {
            input: tokens,
            ..Default::default()
        },
    }
}

/// One Lead Status in the gallery. `LeadGalleryStatus` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "LeadGalleryStatus")]
pub struct GalleryStatus {
    pub name: String,
    pub view: LeadStatusView,
}

/// One track label in the gallery. `LeadGalleryTrack` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "LeadGalleryTrack")]
pub struct GalleryTrack {
    pub name: String,
    pub track: crate::view_models::epic_strip::Track,
}

/// The gallery's lead rows (T-0289): *Row · Lead Status (0083)* (the full and short forms), *Row · Track Label with
/// a lead (0083)* and *Row · Epic Card with a lead (0083)*, from SHOP-40 Wishlist. `LeadGallery` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "LeadGallery")]
pub struct Gallery {
    pub statuses: Vec<GalleryStatus>,
    pub tracks: Vec<GalleryTrack>,
    pub epics: Vec<crate::view_models::card_page::EpicCell>,
}

pub fn gallery() -> Gallery {
    use crate::view_models::card_fixtures::ticket;
    use crate::view_models::card_page::{EpicCardView, EpicCell};
    use crate::view_models::epic_card::EpicViewModel;
    use crate::view_models::epic_strip::Strip;
    use otaq_core::domain::{Actor, Column, Kind, Status};

    let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
    epic.kind = Kind::Epic;
    epic.external_id = Some("PAY-123".into());
    let answering = LeadStatus::Answering {
        ticket: "SHOP-40.2".parse().expect("a key"),
    };
    let mut asks = sample(&epic, LeadStatus::AsksYou, 18_000);
    asks.agent.question = Some(otaq_core::board::questions::HumanQuestion {
        id: "q".into(),
        text: "Should the share link (40.4) expire? PAY-123 doesn't say.".into(),
        options: vec!["Never".into(), "After 30 days".into()],
        recommended: None,
        asked_at: 0,
        via_lead: None,
    });
    let mut replanning = sample(&epic, LeadStatus::Replanning, 212_000);
    replanning.woke_by = Some("SHOP-40.1 merged: Wishlist API".into());
    let lead = |status: LeadStatus| sample(&epic, status, 212_000);
    let statuses = vec![
        ("Idle", lead(LeadStatus::Idle)),
        (
            "Working · checking the plan",
            lead(LeadStatus::CheckingPlan),
        ),
        ("Working · planning (frame 15)", lead(LeadStatus::Planning)),
        ("Working · re-planning", replanning.clone()),
        ("Working · answering", lead(answering.clone())),
        ("Waits for the user", asks.clone()),
        ("Ended by restart", lead(LeadStatus::EndedByRestart)),
        ("Ended · epic in review", lead(LeadStatus::EndedInReview)),
        ("Ended", lead(LeadStatus::Ended)),
    ];
    let mut statuses: Vec<GalleryStatus> = statuses
        .into_iter()
        .map(|(name, l)| GalleryStatus {
            name: name.to_owned(),
            view: LeadStatusView::of(&epic, Some(&l)),
        })
        .collect();
    statuses.push(GalleryStatus {
        name: "None · no lead yet".to_owned(),
        view: LeadStatusView::of(&epic, None),
    });

    // The children: 40.1 merged, 40.2 working, 40.3 queued, 40.4 not queued (a track), or all in Backlog (a card).
    let child = |key: &str, title: &str, status: Status| {
        let mut t = ticket(key, title, status);
        t.epic = Some(epic.key.clone());
        t.queued_at = (status.column() != Column::Backlog).then_some(1_000);
        t
    };
    let in_flight = vec![
        epic.clone(),
        child("SHOP-40.1", "Wishlist API", Status::Merged),
        child("SHOP-40.2", "Wishlist button", Status::Working),
        child("SHOP-40.3", "Wishlist page", Status::Queued),
        child("SHOP-40.4", "Wishlist share link", Status::Backlog),
    ];
    let tracks = [
        ("Idle", lead(LeadStatus::Idle)),
        ("Checking the plan", lead(LeadStatus::CheckingPlan)),
        ("Re-planning", replanning),
        ("Answering 40.2", lead(answering)),
        ("Asks you · whole label", asks.clone()),
        ("Ended by restart", lead(LeadStatus::EndedByRestart)),
    ]
    .into_iter()
    .filter_map(|(name, l)| {
        let strip = Strip::with_leads(&in_flight, Column::ALL, &[l])?;
        Some(GalleryTrack {
            name: name.to_owned(),
            track: strip.tracks.into_iter().next()?,
        })
    })
    .collect();

    let idle = lead(LeadStatus::Idle);
    let mut backlog = vec![epic.clone()];
    for (key, title) in [
        ("SHOP-40.1", "Wishlist API"),
        ("SHOP-40.2", "Wishlist button"),
        ("SHOP-40.3", "Wishlist page"),
        ("SHOP-40.4", "Wishlist share link"),
    ] {
        let mut t = child(key, title, Status::Backlog);
        t.created_by = Actor::ProjectAgent(idle.agent.id.clone());
        backlog.push(t);
    }
    let by_user: Vec<Ticket> = backlog
        .iter()
        .cloned()
        .map(|mut t| {
            t.created_by = Actor::User("you".into());
            t
        })
        .collect();
    let card = |name: &str, tickets: &[Ticket], l: Option<&Lead>| EpicCell {
        name: name.to_owned(),
        card: EpicCardView::from_vm(
            EpicViewModel::from(&epic, tickets, None).with_lead(&epic, tickets, l),
        ),
    };
    let epics = vec![
        card("None · Open lead", &by_user, None),
        card("Idle · rows added by the lead", &backlog, Some(&idle)),
        card(
            "Working · checking the plan",
            &backlog,
            Some(&lead(LeadStatus::CheckingPlan)),
        ),
        card("Asks you", &backlog, Some(&asks)),
    ];
    Gallery {
        statuses,
        tracks,
        epics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use otaq_core::board::questions::HumanQuestion;
    use otaq_core::domain::{Kind, Status};

    fn wishlist() -> Ticket {
        let mut t = ticket("SHOP-40", "Wishlist", Status::Backlog);
        t.kind = Kind::Epic;
        t
    }

    fn view(status: LeadStatus) -> LeadStatusView {
        let epic = wishlist();
        LeadStatusView::of(&epic, Some(&sample(&epic, status, 212_000)))
    }

    /// Every status in both forms and its tone (*Row · Lead Status (0083)*, frame 15 for planning).
    #[test]
    fn every_status_in_both_forms_and_tones() {
        let rows = [
            (LeadStatus::Idle, "Wishlist lead · idle", "idle", Tone::Idle),
            (
                LeadStatus::CheckingPlan,
                "Wishlist lead · checking the plan",
                "checking the plan",
                Tone::Working,
            ),
            (
                LeadStatus::Planning,
                "Wishlist lead · planning",
                "planning",
                Tone::Working,
            ),
            (
                LeadStatus::Replanning,
                "Wishlist lead · re-planning",
                "re-planning",
                Tone::Working,
            ),
            (
                LeadStatus::Answering {
                    ticket: "SHOP-40.2".parse().unwrap(),
                },
                "Wishlist lead · answering SHOP-40.2",
                "answering 40.2",
                Tone::Working,
            ),
            (
                LeadStatus::AsksYou,
                "Wishlist lead · asks you",
                "asks you",
                Tone::AsksYou,
            ),
            (
                LeadStatus::EndedByRestart,
                "Wishlist lead · ended by restart",
                "ended by restart",
                Tone::Ended,
            ),
            (
                LeadStatus::EndedInReview,
                "Wishlist lead · ended · epic in review",
                "ended · in review",
                Tone::Ended,
            ),
            (
                LeadStatus::Ended,
                "Wishlist lead · ended",
                "ended",
                Tone::Ended,
            ),
        ];
        for (status, full, short, tone) in rows {
            let v = view(status.clone());
            assert_eq!(v.full, full, "{status:?}");
            assert_eq!(v.short, short, "{status:?}");
            assert_eq!(v.tone, tone, "{status:?}");
            assert_eq!(v.name.as_deref(), Some("Wishlist lead"));
            assert_eq!(v.icon, IconName::Bot);
            assert!(v.opens_tab);
            let resume = matches!(status, LeadStatus::EndedByRestart | LeadStatus::Ended);
            assert_eq!(
                v.action.as_ref().map(|a| (a.kind, a.label.as_str())),
                resume.then_some((ActionKind::Resume, "Resume")),
                "{status:?}"
            );
            assert_eq!(v.asks_you(), tone == Tone::AsksYou);
        }
    }

    #[test]
    fn no_lead_is_the_open_lead_button() {
        let epic = wishlist();
        for v in [
            LeadStatusView::of(&epic, None),
            LeadStatusView::of(&epic, Some(&sample(&epic, LeadStatus::None, 0))),
        ] {
            assert_eq!(v.name, None);
            assert_eq!(v.full, "Open lead");
            assert_eq!(v.tone, Tone::None);
            assert!(!v.opens_tab);
            let action = v.action.unwrap();
            assert_eq!(
                (action.kind, action.label.as_str()),
                (ActionKind::Open, "Open lead")
            );
            assert_eq!(
                v.tooltip.lines,
                [
                    "Starts the Wishlist lead: a read-only agent that plans with you. Its tokens count on the epic."
                ]
            );
        }
    }

    /// *Interactions (0083)*: the full form, what woke it and its tokens today.
    #[test]
    fn the_tooltip_says_what_woke_it_and_its_tokens_today() {
        let epic = wishlist();
        let mut lead = sample(&epic, LeadStatus::Replanning, 212_000);
        lead.woke_by = Some("SHOP-40.1 merged: Wishlist API".into());
        let v = LeadStatusView::of(&epic, Some(&lead));
        assert_eq!(v.tooltip.title, "Wishlist lead · re-planning");
        assert_eq!(
            v.tooltip.lines,
            ["SHOP-40.1 merged", "212k today", CLICK_HINT]
        );
        // Idle: nothing woke it.
        let v = view(LeadStatus::Idle);
        assert_eq!(v.tooltip.lines, ["212k today", CLICK_HINT]);
        assert_eq!(
            woke_words("SHOP-40.2 asks (question 7): Can guests save?"),
            "SHOP-40.2 asks (question 7)"
        );
    }

    #[test]
    fn the_tooltips_of_asks_you_and_ended_by_restart() {
        let epic = wishlist();
        let mut lead = sample(&epic, LeadStatus::AsksYou, 0);
        lead.agent.question = Some(HumanQuestion {
            id: "q".into(),
            text: "Should the share link (40.4) expire? PAY-123 doesn't say.".into(),
            options: vec!["Never".into(), "After 30 days".into()],
            recommended: None,
            asked_at: 0,
            via_lead: None,
        });
        let v = LeadStatusView::of(&epic, Some(&lead));
        assert_eq!(v.tooltip.title, "Wishlist lead asks you");
        assert_eq!(
            v.tooltip.lines,
            [
                "\u{201c}Should the share link (40.4) expire? PAY-123 doesn't say.\u{201d} · 1 Never · 2 After 30 days",
                "Click to answer in the Lead tab."
            ]
        );
        let v = view(LeadStatus::EndedByRestart);
        assert_eq!(v.tooltip.title, "Wishlist lead · ended by restart");
        assert_eq!(
            v.tooltip.lines,
            [
                "Resume continues the same conversation. Until then, SHOP-40's children ask you directly."
            ]
        );
    }

    #[test]
    fn the_gallery_has_every_form_and_the_frames_rows() {
        let g = gallery();
        assert_eq!(g.statuses.len(), 10);
        assert_eq!(g.statuses.last().unwrap().view.full, "Open lead");
        assert_eq!(g.tracks.len(), 6);
        assert!(g.tracks[4].track.lead_asks);
        assert_eq!(
            g.tracks[3].track.lead.as_ref().unwrap().short,
            "answering 40.2"
        );
        assert_eq!(g.epics.len(), 4);
        assert!(g.epics[0].card.children.iter().all(|c| c.origin.is_none()));
        assert!(
            g.epics[1]
                .card
                .children
                .iter()
                .all(|c| c.origin.as_deref() == Some("added by Wishlist lead"))
        );
        assert!(g.epics[3].card.lead_asks);
    }

    #[test]
    fn the_review_card_uses_the_short_ended_words() {
        let v = view(LeadStatus::EndedInReview).on_review_card();
        assert_eq!(v.full, "Wishlist lead · ended · in review");
    }
}
