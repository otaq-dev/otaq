//! Sample cards from the `shop-web` project (design-briefs/00-README.md), one per variant of *04 · Ticket card
//! variants → Ticket Card v2 · all variants* (the M21 ones). The gallery draws them; tests build on them.

use otaq_core::board::tickets::{Base, Ticket};
use otaq_core::domain::{Actor, Flags, Kind, Priority, Status};
use otaq_core::domain::{AskedLead, ByLead, LeadNoteSent, SameFile, StuckStop, UnreadNotes};

use crate::view_models::card::{AgentFacts, CardExtras, Sleeve};

/// "Now" for fixtures, in Unix milliseconds.
pub const NOW: i64 = 1_791_600_000_000;

pub fn ticket(key: &str, title: &str, status: Status) -> Ticket {
    Ticket {
        id: key.into(),
        key: key.parse().expect("fixture key"),
        kind: Kind::Ticket,
        title: title.into(),
        body: String::new(),
        priority: Default::default(),
        priority_inherited: false,
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
        queued_at: None,
        ready_since: None,
        validation_round: 0,
        auto_merge_conflicts: 0,
        auto_merge_stop: None,
        work: None,
        start_problem: None,
        interrupted_from: None,
        signal: None,
        question: None,
        pushed_differs: None,
        epic_queue_mode: Default::default(),
        joined_queue_by: None,
    }
}

/// How the gallery draws a variant. `CardLook` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "CardLook")]
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

/// The sleeve of SHOP-40 Wishlist's child `n` (four children, *04 → Row · epic children & Done (0080)*).
pub fn wishlist(n: u32, pinned: bool) -> Option<Sleeve> {
    Some(Sleeve {
        epic_key: "SHOP-40".into(),
        name: "Wishlist".into(),
        position: format!("{n} of 4"),
        pinned,
    })
}

fn keys(list: &[&str]) -> Vec<otaq_core::domain::TicketKey> {
    list.iter()
        .map(|k| k.parse().expect("fixture key"))
        .collect()
}

/// *04 → Update U + V · Ticket Card v2 · all variants (0086 · V)* in the frame's order, without the variants that
/// need other tools or named agents (M22, M23); then *Interaction states*. Names are the frame's; "(0066)" and
/// "(0083)" drop the "agent picked" footer M21 leaves out.
pub fn variants() -> Vec<Variant> {
    use otaq_core::board::tickets::AgentSignal;
    use otaq_core::domain::{AutoMergeStop, ReviewEntry, ReviewEntryFlag, ValidationRound};
    let signal = |text: &str| AgentSignal {
        text: Some(text.into()),
        since: Some(NOW - 60_000),
        wakeup_at: None,
    };
    let live = |t: &mut Ticket, ago_s: i64, tokens: u64| {
        t.flags.last_signal_at = Some(NOW - ago_s * 1000);
        t.flags.tokens = Some(tokens);
    };
    let asks = |question: &str| CardExtras {
        question: Some(question.into()),
        ..base()
    };
    let child = |key: &str, title: &str, status| {
        let mut t = ticket(key, title, status);
        t.epic = Some("SHOP-40".parse().expect("key"));
        t
    };
    let sleeved = |n: u32| CardExtras {
        sleeve: wishlist(n, false),
        ..base()
    };
    let mut out = Vec::new();

    let mut t = ticket("SHOP-31", "Hotfix: checkout crash", Status::Backlog);
    t.base = Base::Branch("release/2.3".into());
    t.priority = Priority::Urgent;
    out.push(v("Backlog · calm", t, base()));

    let mut t = ticket("SHOP-24", "Update footer links", Status::Backlog);
    t.priority = Priority::Low;
    t.flags.ready_to_start = true;
    out.push(v("Ready to start (0066)", t, base()));

    let mut t = ticket("SHOP-36", "Cache product prices", Status::Backlog);
    t.created_by = Actor::ProjectAgent("pa-1".into());
    out.push(v(
        "Created by a Project agent (0068)",
        t,
        CardExtras {
            agent_name: Some("Checkout slowness".into()),
            ..base()
        },
    ));

    let mut t = ticket("SHOP-20", "Checkout page", Status::Queued);
    t.depends_on = keys(&["SHOP-18", "SHOP-19"]);
    out.push(v(
        "Waits for merge (0049)",
        t,
        CardExtras {
            waits_for: Some("waits for SHOP-18, SHOP-19 to merge".into()),
            waits_for_merge: true,
            ..base()
        },
    ));

    out.push(v(
        "Start failed (0018)",
        ticket("SHOP-28", "Cart badge count", Status::StartInterrupted),
        CardExtras {
            start_failure: Some("setup command failed".into()),
            ..base()
        },
    ));

    let mut t = ticket("SHOP-35", "Slow checkout query", Status::WaitingForSlot);
    t.priority = Priority::High;
    out.push(v("Waiting for a free slot (0016)", t, base()));

    let mut working = ticket("SHOP-19", "Price table", Status::Working);
    live(&mut working, 40, 182_000);
    out.push(v("Working", working.clone(), base()));

    // Quiet past the core's 5 minutes (ATT-05).
    let mut t = ticket("BILL-4", "Invoice PDF export", Status::Working);
    live(&mut t, 6 * 60, 64_000);
    out.push(v("Working · stale signal (0027)", t, base()));

    let mut t = working.clone();
    t.status = Status::WorkingInBackground;
    t.flags.last_signal_at = None;
    out.push(v(
        "Working in background (0042)",
        t,
        CardExtras {
            background: Some("test run, 1 helper agent · 6 min".into()),
            ..base()
        },
    ));

    let mut t = working.clone();
    t.flags.run_port = Some(4020);
    t.flags.pinned_at = Some(1);
    out.push(v("Running (Run, 0035) · pinned (0047)", t, base()));

    let mut t = ticket("SHOP-29", "Coupon field", Status::Working);
    t.flags.last_signal_at = Some(NOW - 12_000);
    t.flags.tokens_unknown = true;
    out.push(v("Tokens not reported", t, base()));

    let mut t = ticket("SHOP-22", "Fix flaky login redirect", Status::NeedsYou);
    live(&mut t, 12, 64_000);
    out.push(v(
        "Needs you · permission (0042)",
        t,
        asks("Allow rm -rf .next?"),
    ));

    let mut t = ticket("SHOP-12", "Add dark mode toggle", Status::AsksYou);
    live(&mut t, 12, 64_000);
    out.push(v(
        "Asks you (0037)",
        t,
        asks("Should dark mode follow the system setting?"),
    ));

    let mut t = ticket("SHOP-19", "Price table", Status::Idle);
    live(&mut t, 12, 64_000);
    out.push(v("Idle (0042)", t, asks("Should I also update the tests?")));

    let mut t = ticket("SHOP-25", "Upgrade to Next.js 15", Status::Interrupted);
    t.flags.tokens = Some(311_000);
    out.push(v("Interrupted (0018)", t, base()));

    let mut t = ticket("SHOP-33", "Shipping estimate", Status::ValidatorNeedsYou);
    t.flags.validating_round = Some(ValidationRound { round: 1, of: 3 });
    live(&mut t, 60, 64_000);
    t.signal = Some(signal("Allow Bash(pnpm test)?"));
    out.push(v(
        "Needs you · validator (0063)",
        t.clone(),
        base().with_signal(&t, NOW),
    ));

    let mut t = ticket("SHOP-32", "Address autocomplete", Status::ValidationFailed);
    t.validation_round = 3;
    t.flags.findings = vec!["Arrow keys do nothing in the list".into()];
    t.flags.tokens = Some(64_000);
    out.push(v("Validation failed 3 times (0063)", t, base()));

    let mut t = ticket("SHOP-34", "Tax display", Status::ValidationNoResult);
    t.flags.tokens = Some(64_000);
    out.push(v("Validation ended without a result", t, base()));

    let mut t = ticket("SHOP-20", "Checkout page", Status::ParentClosed);
    t.depends_on = keys(&["SHOP-18"]);
    out.push(v(
        "Parent closed (0033)",
        t,
        CardExtras {
            closed_parent: Some("SHOP-18".parse().expect("key")),
            ..base()
        },
    ));

    let mut t = ticket("SHOP-29", "Coupon field", Status::Validating);
    t.status_since = Some(NOW - 2 * 60_000);
    t.flags.validating_round = Some(ValidationRound { round: 1, of: 3 });
    live(&mut t, 3 * 60, 23_000);
    out.push(v("Validating… (0063)", t, base()));

    let mut t = ticket("SHOP-30", "Order history page", Status::Working);
    t.validation_round = 2;
    t.flags.validation_round = Some(ValidationRound { round: 2, of: 3 });
    t.flags.findings = vec![
        "src/orders/page.tsx:41 the list isn't paginated".into(),
        "no test for the empty state".into(),
    ];
    live(&mut t, 20, 141_000);
    out.push(v("Validation failed · round 2 of 3", t, base()));

    let mut t = child(
        "SHOP-40.4",
        "Wishlist share link",
        Status::MergeConflictFixing,
    );
    t.flags.conflicts_with = Some(otaq_core::naming::branch("SHOP-40"));
    t.flags.conflict_files = vec!["lib/wishlist.ts".into()];
    live(&mut t, 10, 77_000);
    out.push(v("Merge conflict · fixing (0064)", t, sleeved(4)));

    let mut t = child("SHOP-40.4", "Wishlist share link", Status::AutoMergeStopped);
    t.auto_merge_stop = Some(AutoMergeStop::KeepsConflicting);
    t.auto_merge_conflicts = 3;
    t.flags.conflicts_with = Some(otaq_core::naming::branch("SHOP-40"));
    t.flags.tokens = Some(64_000);
    out.push(v("Keeps conflicting (0064)", t, sleeved(4)));

    let mut t = child("SHOP-40.3", "Wishlist page", Status::Queued);
    t.depends_on = keys(&["SHOP-40.2"]);
    out.push(v(
        "Picked by the lead (0083)",
        t,
        CardExtras {
            waits_for: Some("waits for SHOP-40.2 to merge".into()),
            ..sleeved(3)
        },
    ));

    let mut t = child("SHOP-40.2", "Wishlist button", Status::Working);
    live(&mut t, 12, 58_000);
    t.flags.unread_notes = Some(UnreadNotes {
        count: 1,
        from: WISHLIST_LEAD.into(),
        latest_text: "40.1 returns items newest first. Use the same order on the shared list."
            .into(),
        latest_at: NOW - 2 * 60_000,
    });
    t.flags.same_file = vec![SameFile {
        other: "SHOP-40.4".parse().expect("key"),
        files: vec!["lib/wishlist.ts".into()],
    }];
    out.push(v("Unread note · same file (0083 · T)", t, sleeved(2)));

    let mut t = child("SHOP-40.5", "Wishlist empty state", Status::AsksYou);
    live(&mut t, 12, 64_000);
    t.flags.via_lead = Some(WISHLIST_LEAD.into());
    out.push(v(
        "Asks you · via Wishlist lead (0083)",
        t,
        CardExtras {
            question: Some("Show the empty state on mobile too?".into()),
            ..sleeved(4)
        },
    ));

    let review = |name, entry, round, diff: bool| {
        let mut t = ticket("SHOP-18", "Checkout API", Status::ReadyForReview);
        t.validation_round = round;
        t.flags.review_entry = Some(ReviewEntryFlag { entry, round });
        v(
            name,
            t,
            CardExtras {
                diff: diff.then_some((412, 38, 9)),
                ..base()
            },
        )
    };
    out.push(review(
        "Validated · round 2",
        ReviewEntry::Validated,
        2,
        true,
    ));
    out.push(review(
        "Validation skipped by you",
        ReviewEntry::Skipped,
        1,
        true,
    ));
    out.push(review(
        "Failed 3 times · moved by you",
        ReviewEntry::FailedMoved,
        3,
        false,
    ));

    let mut t = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
    t.flags.pr_number = Some(481);
    t.flags.tokens = Some(64_000);
    out.push(v(
        "Approved · PR open (0017)",
        t,
        CardExtras {
            pr_provider: Some("GitHub".into()),
            ..base()
        },
    ));

    out.push(v(
        "Merged",
        ticket("SHOP-17", "Add Stripe client", Status::Merged),
        base(),
    ));

    let mut t = child("SHOP-40.1", "Wishlist API", Status::Merged);
    t.validation_round = 1;
    t.flags.review_entry = Some(ReviewEntryFlag {
        entry: ReviewEntry::Validated,
        round: 1,
    });
    t.flags.merged_automatically = true;
    out.push(v("Merged automatically (0064)", t, sleeved(1)));

    let mut t = ticket("SHOP-26", "Add sitemap", Status::ClosedNotMerged);
    t.flags.pr_number = Some(481);
    out.push(v("Closed · not merged (0033)", t, base()));

    out.push(v("Only one tool connected (0086)", working.clone(), base()));

    let mut t = ticket("SHOP-22", "Fix flaky login redirect", Status::NeedsYou);
    t.depends_on = keys(&["SHOP-18", "SHOP-19"]);
    t.flags.run_port = Some(4022);
    live(&mut t, 12, 64_000);
    out.push(v(
        "Needs you + running + 2 dependencies",
        t,
        asks("Allow rm -rf .next?"),
    ));

    for (name, look) in [
        ("Hover", Look::Hovered),
        ("Selected (panel open)", Look::Selected),
        ("Dragging", Look::Dragging),
        ("Chain-highlighted", Look::Chain),
        ("Dimmed", Look::Dimmed),
    ] {
        out.push(Variant {
            look,
            ..v(name, working.clone(), base())
        });
    }
    out
}

/// *04 → Row · epic children & Done (0080)*: sleeved children of SHOP-40 Wishlist (EPC-09, EPC-13, EPC-17).
pub fn sleeve_variants() -> Vec<Variant> {
    let child = |key: &str, title: &str, status| {
        let mut t = ticket(key, title, status);
        t.epic = Some("SHOP-40".parse().expect("key"));
        t
    };
    let mut out = Vec::new();
    let mut t = child("SHOP-40.2", "Wishlist button", Status::Working);
    t.depends_on = keys(&["SHOP-40.1"]);
    t.flags.last_signal_at = Some(NOW - 60_000);
    t.flags.tokens = Some(96_000);
    out.push(v(
        "Dev · epic child · working",
        t,
        CardExtras {
            sleeve: wishlist(2, false),
            ..base()
        },
    ));
    let mut t = child("SHOP-40.2", "Wishlist button", Status::AsksYou);
    t.flags.last_signal_at = Some(NOW - 2 * 60_000);
    t.flags.tokens = Some(98_000);
    out.push(v(
        "Dev · epic child · asks you",
        t,
        CardExtras {
            sleeve: wishlist(2, false),
            question: Some("Show the heart on product cards too?".into()),
            ..base()
        },
    ));
    // The epic is pinned: the pin is in the sleeve, not on the child (EPC-17).
    let mut t = child("SHOP-40.3", "Wishlist page", Status::Queued);
    t.depends_on = keys(&["SHOP-40.2"]);
    out.push(v(
        "Pinned epic · child (no own pin)",
        t,
        CardExtras {
            sleeve: wishlist(3, true),
            waits_for: Some("waits for SHOP-40.2 to merge".into()),
            waits_for_merge: true,
            ..base()
        },
    ));
    // *03 → epic flows 3 → 10*: SHOP-40.2's agent added it while the whole epic was queued (EPC-13).
    let mut t = child("SHOP-40.4", "Wishlist share link", Status::Queued);
    t.depends_on = keys(&["SHOP-40.2"]);
    t.created_by = Actor::Agent("SHOP-40.2".parse().expect("key"));
    t.joined_queue_by = Some(Actor::Agent("SHOP-40.2".parse().expect("key")));
    out.push(v(
        "Ready for Dev · epic child · new",
        t,
        CardExtras {
            sleeve: wishlist(4, false),
            ..base()
        },
    ));
    out
}

/// The lead's name in the fixtures.
pub const WISHLIST_LEAD: &str = "Wishlist lead";

/// The lead's note in the fixtures (*03 → 24, 25*).
pub fn lead_note() -> LeadNoteSent {
    LeadNoteSent {
        lead: WISHLIST_LEAD.into(),
        text: "The API returns items[] (SHOP-40.1). Fix the test, don't change the API.".into(),
        view: Some("The agent keeps changing the API instead of the test.".into()),
        stop: StuckStop::ValidationFailed,
        after_rounds: Some(3),
        at: NOW - 30 * 60_000,
    }
}

/// The refused model's message, as Claude Code words it, straight apostrophe and all (the models report; T-0335's
/// fixture).
pub const MODEL_ISSUE_MESSAGE: &str = "There's an issue with the selected model (claude-fable-5-1). It may not \
                                       exist or you may not have access to it.";

/// shop-web's named agents in the M22 cards (update Y, "a list you made"): Claude Code, Strong, Light, Sonnet and
/// Fable test; the project's Developer default is Strong.
pub fn named(developer: &str) -> AgentFacts {
    AgentFacts {
        count: 5,
        developer: Some(developer.into()),
        validator: Some("Claude Code".into()),
        ..AgentFacts::default()
    }
}

/// *04 → Variants · one tool (M22)* (`uUqnz`) and the picked-by cards of *Ticket Card v2 · all variants* (`SPy9r`),
/// then the running footer with two or more named agents and with one (Q-A). The gallery's "M22 · Named agents".
pub fn agent_variants() -> Vec<Variant> {
    use otaq_core::board::tickets::{AgentSignal, StartProblem};
    use otaq_core::domain::StartStep;
    use otaq_core::named_agents::RowSetBy;
    let picked = || RowSetBy::Agent {
        picked_by: "Checkout slowness".into(),
        reason: "docs-only change".into(),
    };
    let with = |agents: AgentFacts| CardExtras { agents, ..base() };
    let mut out = Vec::new();

    let mut t = ticket("SHOP-48", "Add gift wrap option", Status::Backlog);
    t.flags.ready_to_start = true;
    out.push(v(
        "Set by you \u{b7} before the drag",
        t,
        with(AgentFacts {
            off_default: Some(RowSetBy::User),
            ..named("Fable test")
        }),
    ));

    let mut t = ticket("SHOP-48", "Add gift wrap option", Status::StartInterrupted);
    t.start_problem = Some(StartProblem {
        step: StartStep::Model,
        message: MODEL_ISSUE_MESSAGE.into(),
    });
    out.push(v(
        "Start failed on the model",
        t.clone(),
        with(AgentFacts {
            off_default: Some(RowSetBy::User),
            ..named("Fable test")
        }),
    ));

    let mut t = ticket("SHOP-29", "Coupon field", Status::ValidationNoResult);
    t.signal = Some(AgentSignal {
        text: Some(otaq_core::validation::couldnt_run_text(
            "claude-fable-5-1",
            MODEL_ISSUE_MESSAGE,
        )),
        since: Some(NOW - 60_000),
        wakeup_at: None,
    });
    t.started_at = Some(NOW - 3_600_000);
    out.push(v(
        "No result \u{b7} the validator\u{2019}s model",
        t.clone(),
        CardExtras {
            agents: AgentFacts {
                validator: Some("Fable test".into()),
                ..named("Claude Code")
            },
            ..base()
        }
        .with_signal(&t, NOW),
    ));

    let mut t = ticket("SHOP-24", "Update footer links", Status::Backlog);
    t.flags.ready_to_start = true;
    out.push(v(
        "Project default \u{b7} no footer",
        t,
        with(named("Strong")),
    ));

    let mut t = ticket("SHOP-41", "Fix README typo", Status::Backlog);
    t.priority = Priority::Low;
    t.flags.ready_to_start = true;
    out.push(v(
        "Ready to start \u{b7} agent picked (0066, 0086)",
        t,
        with(AgentFacts {
            off_default: Some(picked()),
            ..named("Light")
        }),
    ));

    let mut t = ticket("SHOP-41", "Fix README typo", Status::Backlog);
    t.created_by = Actor::ProjectAgent("pa-1".into());
    out.push(v(
        "Created by a Project agent \u{b7} agent picked",
        t,
        CardExtras {
            agent_name: Some("Checkout slowness".into()),
            ..with(AgentFacts {
                off_default: Some(picked()),
                ..named("Light")
            })
        },
    ));

    let mut working = ticket("SHOP-19", "Price table", Status::Working);
    working.started_at = Some(NOW - 3_600_000);
    working.flags.last_signal_at = Some(NOW - 40_000);
    working.flags.tokens = Some(182_000);
    out.push(v(
        "Working \u{b7} two or more named agents",
        working.clone(),
        with(AgentFacts {
            off_default: Some(RowSetBy::User),
            running_developer: Some("Strong".into()),
            ..named("Strong")
        }),
    ));
    out.push(v(
        "Working \u{b7} one named agent",
        working,
        with(AgentFacts {
            count: 1,
            running_developer: Some("Claude Code".into()),
            ..named("Claude Code")
        }),
    ));
    out
}

/// A role held before it starts (T-0357's `agent_holds`, as `board.agents` names it).
#[allow(clippy::too_many_arguments)]
pub fn hold(
    role: otaq_core::named_agents::Role,
    reason: otaq_core::named_agents::HoldReason,
    tool: otaq_core::agent_tools::Tool,
    model: Option<&str>,
    agent: &str,
    fallback: Option<&str>,
    can_use_fallback: bool,
    fallback_held: bool,
) -> otaq_core::named_agents::rows::CardHold {
    otaq_core::named_agents::rows::CardHold {
        role,
        reason,
        tool,
        model: model.map(str::to_owned),
        agent: Some(agent.to_owned()),
        fallback: fallback.map(str::to_owned),
        can_use_fallback,
        fallback_held,
        resets_at: None,
    }
}

/// A row a fallback moved (or the user's Move waiting to…).
pub fn moved(
    from: &str,
    reason: otaq_core::named_agents::MoveReason,
    tool: otaq_core::agent_tools::Tool,
) -> otaq_core::named_agents::RowMove {
    otaq_core::named_agents::RowMove {
        from_id: Some(from.to_lowercase()),
        from_name: from.to_owned(),
        reason,
        tool: Some(tool),
        at: NOW - 10 * 60_000,
    }
}

/// *04 → Variants · several tools (M23)* (`gef2j`) and the fallback and limit cards of *Ticket Card v2 · all variants*
/// (`SPy9r`): two or more tools connected, so the head shows whoever has the turn's avatar. shop-web's list here:
/// Strong, Light, Sonnet, Fable test on Claude Code; Standard, Strong GPT, Old GPT on Codex; Verifier on Copilot
/// (update Y's sample list). The gallery's "M23 · Several tools".
pub fn tool_variants() -> Vec<Variant> {
    use crate::view_models::tool_avatar::{AvatarTool, ToolAvatarView};
    use otaq_core::agent_tools::Tool;
    use otaq_core::board::tickets::StartProblem;
    use otaq_core::domain::{StartStep, ValidationRound};
    use otaq_core::named_agents::{HoldReason as H, MoveReason, Role, RowSetBy};
    let av = |tool: AvatarTool, model: Option<&str>| Some(ToolAvatarView::of(tool, model));
    let tools = |developer: Option<ToolAvatarView>, agents: AgentFacts| CardExtras {
        several_tools: true,
        developer,
        validator: av(AvatarTool::ClaudeCode, None),
        agents: AgentFacts { count: 8, ..agents },
        ..base()
    };
    // A held Developer is Held with its step (T-0357); the hold's row says the rest.
    let held = |key: &str, title: &str, step| {
        let mut t = ticket(key, title, Status::Held);
        t.start_problem = Some(StartProblem {
            step,
            message: "held before launch".into(),
        });
        t
    };
    let mut out = Vec::new();

    let mut t = ticket("SHOP-48", "Add gift wrap option", Status::Backlog);
    t.priority = Priority::High;
    t.flags.ready_to_start = true;
    out.push(v(
        "Set by you \u{b7} several tools",
        t,
        tools(
            av(AvatarTool::ClaudeCode, Some("claude-fable-5-1")),
            AgentFacts {
                off_default: Some(RowSetBy::User),
                ..named("Fable test")
            },
        ),
    ));

    let mut t = ticket("SHOP-48", "Add gift wrap option", Status::StartInterrupted);
    t.priority = Priority::High;
    t.start_problem = Some(StartProblem {
        step: StartStep::Model,
        message: MODEL_ISSUE_MESSAGE.into(),
    });
    out.push(v(
        "Start failed \u{b7} several tools",
        t,
        tools(
            av(AvatarTool::ClaudeCode, Some("claude-fable-5-1")),
            AgentFacts {
                off_default: Some(RowSetBy::User),
                ..named("Fable test")
            },
        ),
    ));

    let mut t = ticket("SHOP-18", "Checkout API", Status::StartInterrupted);
    t.start_problem = Some(StartProblem {
        step: StartStep::Model,
        message: "model gpt-5.4 is not supported on this account".into(),
    });
    out.push(v(
        "Start failed \u{b7} Codex",
        t,
        tools(av(AvatarTool::Codex, Some("gpt-5.4")), named("Standard")),
    ));

    let mut t = held(
        "SHOP-49",
        "Export orders as CSV",
        StartStep::ModelUnavailable,
    );
    t.priority = Priority::High;
    out.push(v(
        "Model unavailable",
        t,
        tools(
            av(AvatarTool::Codex, Some("gpt-5.4")),
            AgentFacts {
                holds: vec![hold(
                    Role::Developer,
                    H::ModelUnavailable,
                    Tool::Codex,
                    Some("gpt-5.4"),
                    "Old GPT",
                    None,
                    false,
                    false,
                )],
                ..named("Old GPT")
            },
        ),
    ));

    let mut t = ticket("SHOP-30", "Order history page", Status::Validating);
    t.priority = Priority::High;
    t.started_at = Some(NOW - 3_600_000);
    out.push(v(
        "Model unavailable \u{b7} the validator",
        t,
        CardExtras {
            validator: av(AvatarTool::Codex, Some("gpt-5.4")),
            ..tools(
                av(AvatarTool::ClaudeCode, None),
                AgentFacts {
                    validator: Some("Old GPT".into()),
                    holds: vec![hold(
                        Role::Validator,
                        H::ModelUnavailable,
                        Tool::Codex,
                        Some("gpt-5.4"),
                        "Old GPT",
                        None,
                        false,
                        false,
                    )],
                    ..named("Strong")
                },
            )
        },
    ));

    let mut swapped = ticket("SHOP-29", "Coupon field", Status::Validating);
    swapped.priority = Priority::High;
    swapped.started_at = Some(NOW - 3_600_000);
    swapped.status_since = Some(NOW - 2 * 60_000);
    swapped.flags.validating_round = Some(ValidationRound { round: 1, of: 3 });
    swapped.flags.tokens = Some(23_000);
    out.push(v(
        "Swapped model \u{b7} not on the card",
        swapped,
        CardExtras {
            validator: av(AvatarTool::Copilot, Some("auto")),
            ..tools(
                av(AvatarTool::ClaudeCode, None),
                AgentFacts {
                    validator: Some("Verifier".into()),
                    running_validator: Some("Verifier".into()),
                    ..named("Strong")
                },
            )
        },
    ));

    let mut t = held("SHOP-24", "Update footer links", StartStep::ToolLimit);
    t.priority = Priority::Low;
    out.push(v(
        "Tool at its limit, no fallback (0086)",
        t,
        tools(
            av(AvatarTool::ClaudeCode, None),
            AgentFacts {
                holds: vec![hold(
                    Role::Developer,
                    H::ToolLimit,
                    Tool::Claude,
                    None,
                    "Light",
                    None,
                    false,
                    false,
                )],
                ..named("Light")
            },
        ),
    ));

    let mut t = held("SHOP-31", "Hotfix: checkout crash", StartStep::ToolLimit);
    t.priority = Priority::Urgent;
    out.push(v(
        "Tool at its limit \u{b7} fallback off",
        t,
        tools(
            av(AvatarTool::ClaudeCode, Some("claude-opus-5-5")),
            AgentFacts {
                holds: vec![hold(
                    Role::Developer,
                    H::ToolLimit,
                    Tool::Claude,
                    Some("claude-opus-5-5"),
                    "Strong",
                    Some("Strong GPT"),
                    true,
                    false,
                )],
                ..named("Strong")
            },
        ),
    ));

    let t = held(
        "SHOP-36",
        "Cache product prices",
        StartStep::ToolUnavailable,
    );
    out.push(v(
        "Tool not available \u{b7} fallback off (0086)",
        t,
        tools(
            av(AvatarTool::Codex, None),
            AgentFacts {
                holds: vec![hold(
                    Role::Developer,
                    H::ToolUnavailable,
                    Tool::Codex,
                    None,
                    "Standard",
                    Some("Sonnet"),
                    true,
                    false,
                )],
                ..named("Standard")
            },
        ),
    ));

    let mut t = ticket("SHOP-33", "Shipping estimate", Status::Validating);
    t.started_at = Some(NOW - 3_600_000);
    out.push(v(
        "Validator held \u{b7} tool not available",
        t,
        CardExtras {
            validator: av(AvatarTool::Copilot, None),
            ..tools(
                av(AvatarTool::ClaudeCode, None),
                AgentFacts {
                    validator: Some("Verifier".into()),
                    holds: vec![hold(
                        Role::Validator,
                        H::ToolUnavailable,
                        Tool::Copilot,
                        None,
                        "Verifier",
                        Some("Strong"),
                        true,
                        false,
                    )],
                    ..named("Standard")
                },
            )
        },
    ));

    let mut t = ticket("SHOP-31", "Hotfix: checkout crash", Status::Working);
    t.priority = Priority::Urgent;
    t.started_at = Some(NOW - 600_000);
    t.flags.last_signal_at = Some(NOW - 30_000);
    t.flags.tokens = Some(80_000);
    out.push(v(
        "Moved by a fallback (0086)",
        t,
        tools(
            av(AvatarTool::Codex, Some("gpt-5.5")),
            AgentFacts {
                running_developer: Some("Strong GPT".into()),
                developer_move: Some(moved("Strong", MoveReason::ToolLimit, Tool::Claude)),
                ..named("Strong GPT")
            },
        ),
    ));

    let mut t = held("SHOP-31", "Hotfix: checkout crash", StartStep::ToolLimit);
    t.priority = Priority::Urgent;
    out.push(v(
        "Fallback also at its limit",
        t,
        tools(
            av(AvatarTool::ClaudeCode, Some("claude-opus-5-5")),
            AgentFacts {
                holds: vec![hold(
                    Role::Developer,
                    H::ToolLimit,
                    Tool::Claude,
                    Some("claude-opus-5-5"),
                    "Strong",
                    Some("Strong GPT"),
                    false,
                    true,
                )],
                ..named("Strong")
            },
        ),
    ));

    let mut t = ticket("SHOP-29", "Coupon field", Status::Validating);
    t.started_at = Some(NOW - 3_600_000);
    t.flags.tokens = Some(6_000);
    out.push(v(
        "Validator moved by its fallback",
        t,
        CardExtras {
            validator: av(AvatarTool::ClaudeCode, Some("claude-opus-5-5")),
            ..tools(
                av(AvatarTool::Copilot, None),
                AgentFacts {
                    validator: Some("Strong".into()),
                    running_validator: Some("Strong".into()),
                    validator_move: Some(moved(
                        "Verifier",
                        MoveReason::ToolUnavailable,
                        Tool::Copilot,
                    )),
                    ..named("Standard")
                },
            )
        },
    ));
    out
}

/// *04 → Row · Epic children & the lead (0083)* and *Row · Unread notes & same file (0083 · T)*, in the frames'
/// order, then *03 → 25*'s second stop (LEAD-06…LEAD-10). SHOP-40 Wishlist has five children here.
pub fn lead_variants() -> Vec<Variant> {
    let child = |key: &str, title: &str, status| {
        let mut t = ticket(key, title, status);
        t.epic = Some("SHOP-40".parse().expect("key"));
        t
    };
    let sleeve = |n: u32| CardExtras {
        sleeve: Some(Sleeve {
            epic_key: "SHOP-40".into(),
            name: "Wishlist".into(),
            position: format!("{n} of 5"),
            pinned: false,
        }),
        ..base()
    };
    let live = |t: &mut Ticket, ago_s: i64, tokens: u64| {
        t.flags.last_signal_at = Some(NOW - ago_s * 1000);
        t.flags.tokens = Some(tokens);
    };
    let same = |other: &str| {
        vec![SameFile {
            other: other.parse().expect("key"),
            files: vec!["lib/wishlist.ts".into()],
        }]
    };
    let unread = || {
        UnreadNotes {
        count: 1,
        from: WISHLIST_LEAD.into(),
        latest_text: "40.1 returns items newest first (recorded decision on SHOP-40.1). Use the same order on the \
                      shared list."
            .into(),
        latest_at: NOW - 2 * 60_000,
    }
    };
    let mut out = Vec::new();

    let mut t = child("SHOP-40.2", "Wishlist button", Status::Working);
    t.depends_on = keys(&["SHOP-40.1"]);
    live(&mut t, 60, 104_000);
    t.flags.asked_lead = Some(AskedLead {
        lead: WISHLIST_LEAD.into(),
        question: "Should the button show a filled heart once the item is saved?".into(),
    });
    out.push(v("Dev · asked lead · waiting", t, sleeve(2)));

    let mut t = child("SHOP-40.2", "Wishlist button", Status::AsksYou);
    live(&mut t, 3 * 60, 106_000);
    t.flags.via_lead = Some(WISHLIST_LEAD.into());
    out.push(v(
        "Dev · Asks you · via Wishlist lead",
        t,
        CardExtras {
            question: Some("Can guests save items without logging in?".into()),
            ..sleeve(2)
        },
    ));

    let mut t = child("SHOP-40.3", "Wishlist page", Status::Queued);
    t.depends_on = keys(&["SHOP-40.4"]);
    t.flags.by_lead = Some(ByLead {
        lead: WISHLIST_LEAD.into(),
        added: false,
        updated: true,
        fields: vec!["description".into()],
        waits_for: keys(&["SHOP-40.4"]),
    });
    out.push(v("Ready for Dev · Updated by Wishlist lead", t, sleeve(3)));

    let mut t = child("SHOP-40.5", "Wishlist empty state", Status::Queued);
    t.created_by = Actor::ProjectAgent("lead-1".into());
    t.flags.by_lead = Some(ByLead {
        lead: WISHLIST_LEAD.into(),
        added: true,
        ..ByLead::default()
    });
    out.push(v(
        "Ready for Dev · new · added by Wishlist lead",
        t,
        sleeve(5),
    ));

    let mut t = child("SHOP-40.3", "Wishlist page", Status::Working);
    live(&mut t, 20, 141_000);
    t.flags.lead_note_sent = Some(lead_note());
    out.push(v("Dev · Lead's note sent (round 0 of 3)", t, sleeve(3)));

    let mut t = child("SHOP-40.4", "Wishlist share link", Status::Working);
    live(&mut t, 40, 71_000);
    t.flags.unread_notes = Some(unread());
    out.push(v("Dev · 1 unread note from Wishlist lead", t, sleeve(4)));

    let mut t = child("SHOP-40.2", "Wishlist button", Status::Working);
    live(&mut t, 20, 118_000);
    t.flags.same_file = same("SHOP-40.4");
    out.push(v("Dev · same file as SHOP-40.4", t, sleeve(2)));

    let mut t = child("SHOP-40.4", "Wishlist share link", Status::Working);
    live(&mut t, 60, 71_000);
    t.flags.same_file = same("SHOP-40.2");
    out.push(v("Dev · same file as SHOP-40.2", t.clone(), sleeve(4)));

    t.flags.unread_notes = Some(unread());
    out.push(v("Dev · both on one card", t, sleeve(4)));

    let mut t = child("SHOP-40.3", "Wishlist page", Status::ValidationFailed);
    t.validation_round = 3;
    t.flags.findings = vec!["orders.test.ts expects list".into()];
    t.flags.tokens = Some(163_000);
    t.flags.lead_note_sent = Some(lead_note());
    out.push(v(
        "Dev · Failed 3 times after the lead's note",
        t,
        sleeve(3),
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
