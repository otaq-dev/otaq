//! Sample cards from the `shop-web` project (design-briefs/00-README.md), one per variant of *04 · Ticket card
//! variants*. The gallery draws them; tests build on them. States that the service can't produce yet (orange
//! family M4, Queued reasons M5, review flags M6, usage M8, Run M9, PR M10) exist here only.

use crewboard_core::board::tickets::{Base, DoneWhenItem, Ticket};
use crewboard_core::domain::{Actor, Flags, Kind, SentBack, Status};

use crate::view_models::card::CardExtras;

/// "Now" for fixtures, in Unix milliseconds.
pub const NOW: i64 = 1_791_600_000_000;

pub fn ticket(key: &str, title: &str, status: Status) -> Ticket {
    Ticket {
        id: key.into(),
        key: key.parse().expect("fixture key"),
        kind: Kind::Ticket,
        title: title.into(),
        body: String::new(),
        done_when: vec![DoneWhenItem {
            text: "Works".into(),
            added_by: Actor::User("me".into()),
        }],
        epic: None,
        column: status.column(),
        status,
        base: Base::Branch("main".into()),
        base_locked: false,
        external_id: None,
        depends_on: Vec::new(),
        aliases: Vec::new(),
        next_child_number: None,
        flags: Flags::default(),
        created_by: Actor::User("me".into()),
        updated_by: None,
        started_at: None,
        created_at: 0,
        updated_at: 0,
        status_since: None,
        work: None,
        start_problem: None,
        signal: None,
        question: None,
    }
}

/// How the gallery draws a variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Look {
    #[default]
    Plain,
    Selected,
    Hovered,
    Dragging,
    Chain,
    Dimmed,
}

pub struct Variant {
    pub name: &'static str,
    pub ticket: Ticket,
    pub extras: CardExtras,
    pub look: Look,
}

fn v(name: &'static str, ticket: Ticket, extras: CardExtras) -> Variant {
    Variant {
        name,
        ticket,
        extras,
        look: Look::Plain,
    }
}

fn base() -> CardExtras {
    CardExtras {
        project_base: Some("main".into()),
        ..CardExtras::default()
    }
}

fn keys(list: &[&str]) -> Vec<crewboard_core::domain::TicketKey> {
    list.iter()
        .map(|k| k.parse().expect("fixture key"))
        .collect()
}

/// The *Variant grid* (ticket rows; epics are the Epic Card's, T-0032).
pub fn variants() -> Vec<Variant> {
    let mut out = Vec::new();
    let mut t = ticket("SHOP-31", "Add order history page", Status::Backlog);
    t.done_when = (0..3)
        .map(|i| DoneWhenItem {
            text: format!("criterion {i}"),
            added_by: Actor::User("me".into()),
        })
        .collect();
    out.push(v("Backlog · complete", t, base()));
    let mut t = ticket("SHOP-24", "Update footer links", Status::Backlog);
    t.done_when.clear();
    out.push(v("Backlog · no “Done when”", t, base()));
    out.push(v(
        "Queued · waiting on deps",
        ticket("SHOP-21", "Order confirmation email", Status::Queued),
        CardExtras {
            waits_for: Some("waits for SHOP-20 to merge".into()),
            ..base()
        },
    ));
    let mut t = ticket("SHOP-20", "Checkout page", Status::Queued);
    t.depends_on = keys(&["SHOP-18", "SHOP-19"]);
    out.push(v(
        "Queued · waits for merge",
        t,
        CardExtras {
            waits_for: Some("waits for SHOP-18, SHOP-19 to merge · SHOP-18 in Review".into()),
            ..base()
        },
    ));
    let mut t = ticket("SHOP-25", "Upgrade to Next.js 15", Status::Parked);
    t.flags.tokens = Some(241_000);
    out.push(v(
        "Backlog · Parked · has work",
        t,
        CardExtras {
            branch: Some(crewboard_core::naming::branch("SHOP-25")),
            ..base()
        },
    ));
    let mut t = ticket("SHOP-31", "Hotfix: checkout crash", Status::Backlog);
    t.base = Base::Branch("release/2.3".into());
    t.done_when.clear();
    t.done_when.push(DoneWhenItem {
        text: "No crash".into(),
        added_by: Actor::User("me".into()),
    });
    out.push(v("Backlog · other base branch", t, base()));
    out.push(v(
        "Queued · start interrupted",
        ticket("SHOP-28", "Cart badge count", Status::StartInterrupted),
        base(),
    ));
    let mut t = ticket("SHOP-19", "Price table", Status::Working);
    t.flags.last_signal_at = Some(NOW - 40_000);
    t.flags.tokens = Some(182_000);
    t.flags.run_port = Some(4020);
    out.push(v("Working · normal + running", t.clone(), base()));
    let mut stale = ticket("BILL-4", "Invoice PDF export", Status::Working);
    stale.flags.last_signal_at = Some(NOW - 6 * 60_000);
    stale.flags.tokens = Some(205_000);
    out.push(v("Working · stale signal", stale, base()));
    let mut ny = ticket("SHOP-22", "Fix flaky login redirect", Status::NeedsYou);
    ny.flags.last_signal_at = Some(NOW - 12_000);
    ny.flags.tokens = Some(64_000);
    out.push(v(
        "Working · needs you",
        ny,
        CardExtras {
            question: Some("Allow rm -rf .next?".into()),
            ..base()
        },
    ));
    let mut mw = t.clone();
    mw.flags.messages_waiting = 1;
    mw.flags.run_port = None;
    out.push(v("Dev · message waiting", mw, base()));
    let mut ans = ticket("SHOP-40.1", "Wishlist API", Status::Merged);
    ans.epic = Some("SHOP-40".parse().expect("key"));
    ans.flags.answering = Some("SHOP-40.2".parse().expect("key"));
    ans.flags.last_signal_at = Some(NOW - 5_000);
    out.push(v(
        "Done · answering",
        ans,
        CardExtras {
            epic_title: Some("Wishlist".into()),
            ..base()
        },
    ));
    let mut int = ticket("SHOP-25", "Upgrade to Next.js 15", Status::Interrupted);
    int.flags.tokens = Some(311_000);
    out.push(v("Working · interrupted", int, base()));
    let mut asks = ticket("SHOP-23", "Split user full name", Status::AsksYou);
    asks.flags.last_signal_at = Some(NOW - 3 * 60_000);
    asks.flags.tokens = Some(41_000);
    out.push(v(
        "Dev · Asks you",
        asks,
        CardExtras {
            question: Some("Split fullName everywhere, or only in the profile form?".into()),
            ..base()
        },
    ));
    out.push(v(
        "Ready for Review",
        ticket("SHOP-18", "Checkout API", Status::ReadyForReview),
        CardExtras {
            diff: Some((412, 38, 9)),
            ..base()
        },
    ));
    let mut sb = ticket("SHOP-27", "Search results paging", Status::Working);
    sb.flags.sent_back = Some(SentBack {
        comments: 3,
        round: 2,
    });
    sb.flags.last_signal_at = Some(NOW - 15_000);
    sb.flags.tokens = Some(133_000);
    out.push(v("Agent Working · sent back", sb, base()));
    let mut pr = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
    pr.flags.pr_number = Some(481);
    pr.flags.tokens = Some(52_000);
    out.push(v(
        "Approved · PR open",
        pr,
        CardExtras {
            pr_provider: Some("GitHub".into()),
            ..base()
        },
    ));
    out.push(v(
        "Done · merged",
        ticket("SHOP-17", "Add Stripe client", Status::Merged),
        base(),
    ));
    let mut combined = ticket("SHOP-22", "Fix flaky login redirect", Status::NeedsYou);
    combined.depends_on = keys(&["SHOP-17", "SHOP-18"]);
    combined.flags.run_port = Some(4024);
    out.push(v(
        "Combined · needs you + running + 2 deps",
        combined,
        CardExtras {
            question: Some("Allow rm -rf .next?".into()),
            ..base()
        },
    ));
    for (name, look) in [
        ("Selected · focused", Look::Selected),
        ("Hovered · quick actions", Look::Hovered),
        ("Dragging · lifted", Look::Dragging),
    ] {
        out.push(Variant {
            look,
            ..v(name, t.clone(), base())
        });
    }
    out.push(Variant {
        look: Look::Chain,
        ..v(
            "Chain-highlighted",
            ticket("SHOP-18", "Checkout API", Status::ReadyForReview),
            CardExtras {
                diff: Some((412, 38, 9)),
                ..base()
            },
        )
    });
    let mut dim = ticket("SHOP-27", "Search results paging", Status::Working);
    dim.flags.sent_back = Some(SentBack {
        comments: 3,
        round: 1,
    });
    out.push(Variant {
        look: Look::Dimmed,
        ..v("Dimmed (not in chain)", dim, base())
    });
    let mut closed = ticket("SHOP-26", "Add sitemap", Status::ClosedNotMerged);
    closed.flags.pr_number = Some(481);
    out.push(v("Done · closed, not merged", closed, base()));
    let mut pc = ticket("SHOP-30", "Sitemap for blog posts", Status::ParentClosed);
    pc.depends_on = keys(&["SHOP-26"]);
    out.push(v(
        "Child · parent closed",
        pc,
        CardExtras {
            closed_parent: Some("SHOP-26".parse().expect("key")),
            ..base()
        },
    ));
    let mut idle = ticket("SHOP-19", "Price table", Status::Idle);
    idle.flags.last_signal_at = Some(NOW - 6 * 60_000);
    idle.flags.tokens = Some(182_000);
    out.push(v(
        "Dev · Idle",
        idle,
        CardExtras {
            question: Some("Should I also update the tests?".into()),
            ..base()
        },
    ));
    let mut pinned = t.clone();
    pinned.flags.pinned_at = Some(1);
    out.push(v("Pinned", pinned, base()));
    let mut limit = ticket("SHOP-19", "Price table", Status::WaitingForLimitReset);
    limit.flags.tokens = Some(182_000);
    out.push(v("Dev · waiting for limit reset", limit, base()));
    let mut conflicts = ticket("SHOP-18", "Checkout API", Status::ReadyForReview);
    conflicts.flags.conflicts_with = Some("main".into());
    out.push(v(
        "Review · conflicts with main",
        conflicts,
        CardExtras {
            diff: Some((412, 38, 9)),
            ..base()
        },
    ));
    let mut child = ticket("SHOP-40.2", "Wishlist button", Status::Working);
    child.epic = Some("SHOP-40".parse().expect("key"));
    child.depends_on = keys(&["SHOP-40.1"]);
    child.flags.last_signal_at = Some(NOW - 60_000);
    child.flags.tokens = Some(36_000);
    out.push(v(
        "Epic child · working",
        child,
        CardExtras {
            epic_title: Some("Wishlist".into()),
            ..base()
        },
    ));
    let mut moved = ticket("SHOP-40.4", "Wishlist share link", Status::Backlog);
    moved.epic = Some("SHOP-40".parse().expect("key"));
    out.push(v(
        "Ticket · Move to epic…",
        moved,
        CardExtras {
            old_key: Some("SHOP-51".parse().expect("key")),
            epic_title: Some("Wishlist".into()),
            ..base()
        },
    ));
    let mut bg = t.clone();
    bg.status = Status::WorkingInBackground;
    bg.flags.run_port = None;
    bg.flags.last_signal_at = None;
    out.push(v(
        "Dev · working in background",
        bg.clone(),
        CardExtras {
            background: Some("test run, 1 helper agent · 6 min".into()),
            ..base()
        },
    ));
    out.push(v(
        "Dev · background · scheduled wakeup",
        bg,
        CardExtras {
            background: Some("wakes up at 4:10pm".into()),
            background_wakeup: true,
            ..base()
        },
    ));
    let mut helper = t.clone();
    helper.status = Status::NeedsYou;
    helper.flags.run_port = None;
    helper.flags.last_signal_at = None;
    out.push(v(
        "Dev · needs you · helper agent",
        helper,
        CardExtras {
            question: Some("Allow pnpm test --update-snapshots?".into()),
            helper_agent: true,
            ..base()
        },
    ));
    out.push(v(
        "Queued · held: base diverged",
        ticket("BILL-6", "Late fee rule", Status::Held),
        CardExtras {
            held_reason: Some(
                "develop and origin/develop have diverged. Push or pull, then retry.".into(),
            ),
            ..base()
        },
    ));
    out
}

/// An epic and its project's tickets, for the Epic Card rows.
pub struct EpicVariant {
    pub name: &'static str,
    pub epic: Ticket,
    pub tickets: Vec<Ticket>,
    pub branch: Option<&'static str>,
}

/// *Row · epics* and *Row · epics as delivery units (0059)*.
pub fn epic_variants() -> Vec<EpicVariant> {
    let epic = |key: &str, title: &str, status: Status, external: Option<&str>| {
        let mut t = ticket(key, title, status);
        t.kind = Kind::Epic;
        t.external_id = external.map(str::to_owned);
        t
    };
    let child = |epic: &str, key: &str, title: &str, status: Status| {
        let mut t = ticket(key, title, status);
        t.epic = Some(epic.parse().expect("key"));
        t
    };
    let wishlist = |status: Status, children: [Status; 3]| {
        let e = epic("SHOP-40", "Wishlist", status, Some("PAY-123"));
        let mut tickets = vec![e.clone()];
        for (i, (title, s)) in ["Wishlist API", "Wishlist button", "Wishlist page"]
            .into_iter()
            .zip(children)
            .enumerate()
        {
            tickets.push(child("SHOP-40", &format!("SHOP-40.{}", i + 1), title, s));
        }
        (e, tickets)
    };
    let mut out = Vec::new();
    let (e, t) = wishlist(
        Status::Backlog,
        [Status::Merged, Status::Working, Status::Backlog],
    );
    out.push(EpicVariant {
        name: "Epic · Backlog",
        epic: e,
        tickets: t,
        branch: None,
    });
    let (e, t) = wishlist(Status::ReadyForReview, [Status::Merged; 3]);
    out.push(EpicVariant {
        name: "Epic · Ready for Review",
        epic: e,
        tickets: t,
        branch: None,
    });
    let (mut e, mut t) = wishlist(Status::PrOpen, [Status::Merged; 3]);
    e.flags.pr_number = Some(490);
    t[0] = e.clone();
    out.push(EpicVariant {
        name: "Epic · Approved · PR open",
        epic: e,
        tickets: t,
        branch: Some("feature/PAY-123-wishlist"),
    });
    let (e, t) = wishlist(Status::Merged, [Status::Merged; 3]);
    out.push(EpicVariant {
        name: "Epic · Done",
        epic: e,
        tickets: t,
        branch: Some("feature/PAY-123-wishlist"),
    });
    let gift = epic("SHOP-50", "Gift cards", Status::ClosedNotMerged, None);
    out.push(EpicVariant {
        name: "Epic · closed, not merged",
        tickets: vec![
            gift.clone(),
            child(
                "SHOP-50",
                "SHOP-50.1",
                "Gift card model",
                Status::ClosedNotMerged,
            ),
            child(
                "SHOP-50",
                "SHOP-50.2",
                "Redeem gift card",
                Status::ClosedNotMerged,
            ),
        ],
        epic: gift,
        branch: None,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card::CardViewModel;

    #[test]
    fn every_fixture_builds_a_card() {
        let all = variants();
        assert!(all.len() >= 30);
        for variant in &all {
            let vm = CardViewModel::from(&variant.ticket, &variant.extras, NOW);
            assert!(!vm.title.is_empty(), "{}", variant.name);
        }
    }
}
