//! The moves between stages (STG-06, d0108; the default flow's rows are SDD §7.3 and §7.4): one pure place that
//! says, from a project's flow, where a ticket goes. It does no I/O; the board, validation and review code carry
//! the result out (`board/validation.rs::transition`, `board/moves.rs::check_move` and the review moves).
//!
//! - [`step`]: what a [`Move`] does where a ticket stands: the agent's "next stage", an agent check's verdict, a
//!   human check's Pass or Fail, a Delivery finishing, and the user's shortcuts (Skip this check, Move on anyway…).
//!   A Fail returns the card to the nearest earlier Agent stage, which resumes its session; from there it walks
//!   every later stage again. Each agent check counts its own fails in a row ([`Counters`]); a human Fail resets
//!   them all. A human check directly followed by a click Delivery offers the delivery choices as its Pass.
//!   Moving on into an Agent stage waits before it (STG-07): only the scheduler's [`Move::Begin`] enters it.
//! - [`drag`]: a drag's verdict (today's drag rules, by stage kind).
//! - [`default_stage`]: the default flow's stage for a status, for the writes that don't know the flow yet (until
//!   T-0401–T-0403 move them onto stages).

use serde::{Deserialize, Serialize};

use super::default::{DEV, REVIEW, VALIDATE};
use super::model::{CheckBy, Column, Delivers, DeliveryMode, Flow, Stage, StageKind};
use crate::board::moves::{
    DropTarget, MERGED_LOCKED, MoveAction, MoveCheck, ONLY_AGENT_SUBMITS, STARTS_BY_ITSELF,
};
use crate::domain::{Kind, Place, Status, StatusSet, stored_enum};

stored_enum! {
    /// How a stage run ended (STG-05, `stage_runs.result`).
    pub enum RunResult ("stage run result") {
        /// A check passed the work.
        Passed = "passed",
        /// A check failed it.
        Failed = "failed",
        /// An Agent stage or a Delivery finished.
        Done = "done",
        /// The ticket skipped the stage, or the user skipped the running check.
        Skipped = "skipped",
        /// Stopped before it ended: parked, closed, the code changed under a check.
        Cancelled = "cancelled",
        /// Ended without a verdict.
        NoResult = "no_result",
    }
}

/// What a stage run's kind is stored as (`stage_runs.kind`).
pub fn run_kind(kind: StageKind) -> &'static str {
    match kind {
        StageKind::Agent => "agent",
        StageKind::Check { by: CheckBy::Agent } => "check",
        StageKind::Check { by: CheckBy::You } => "you",
        StageKind::Delivery => "delivery",
    }
}

/// The default flow's stage for a status written without a stage: the validation statuses (and an Interrupted or
/// limit-held validation, `interrupted_from`) are Validate's, the other Agent stage statuses Dev's, Ready for
/// Review is Review's, the Delivery statuses Done's. `None` for Backlog, waiting and Done.
pub fn default_stage(status: Status, interrupted_from: Option<Status>) -> Option<&'static str> {
    let check = |s: Status| s.set() == StatusSet::AgentCheck;
    match status.set() {
        StatusSet::Backlog | StatusSet::Waiting | StatusSet::Done => None,
        StatusSet::AgentCheck => Some(VALIDATE),
        StatusSet::AgentStage if interrupted_from.is_some_and(check) => Some(VALIDATE),
        StatusSet::AgentStage => Some(DEV),
        StatusSet::HumanCheck => Some(REVIEW),
        StatusSet::Delivery => Some(super::default::DELIVER),
    }
}

/// The status a ticket has on entering a stage, by its kind.
pub fn entry_status(stage: &Stage) -> Status {
    match stage.kind {
        StageKind::Agent => Status::Working,
        StageKind::Check { by: CheckBy::Agent } => Status::Checking,
        StageKind::Check { by: CheckBy::You } => Status::WaitingForYou,
        StageKind::Delivery => Status::ToDeliver,
    }
}

/// The set of statuses a stage of this kind allows (STG-04).
pub fn status_set(kind: StageKind) -> StatusSet {
    match kind {
        StageKind::Agent => StatusSet::AgentStage,
        StageKind::Check { by: CheckBy::Agent } => StatusSet::AgentCheck,
        StageKind::Check { by: CheckBy::You } => StatusSet::HumanCheck,
        StageKind::Delivery => StatusSet::Delivery,
    }
}

/// An agent check's limit: its rounds (3 when the flow doesn't say) plus the extra rounds the user allowed.
pub fn rounds_of(stage: &Stage, extra: u32) -> u32 {
    stage
        .rounds
        .unwrap_or(crate::flow::check::DEFAULT_VALIDATION_ROUNDS)
        + extra
}

/// What happened, or what the user or an agent asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Move {
    /// The agent's "next stage" from an Agent stage (`stage next`), or the user's Move to next stage.
    NextStage,
    /// The board's move for the user's Move to next stage when no agent check comes next (an agent check runs
    /// through `NextStage`, d0108 rule 10): to the next stage, any agent checks walked over counting as skipped
    /// (d0052; never delivered automatically).
    SubmitByUser,
    /// An agent check's `pass`.
    Pass,
    /// An agent check's `fail`.
    Fail,
    /// The checker ended without a verdict, or its verdict was discarded.
    NoResult,
    /// The check stopped without counting: the code or the description changed, or the user's Stop.
    CancelCheck,
    /// A fresh checker after no result.
    RetryCheck,
    /// The user skips the running check (Skip this check).
    SkipCheck,
    /// After the last allowed fail or no result: on to the next stage (Move on anyway).
    MoveOnAnyway,
    /// After the last allowed fail: back to the agent with the findings, one more round allowed.
    AnotherRound,
    /// After the last allowed fail: back to the agent with the user's note; this check's counter restarts.
    AnswerAgent,
    /// A human check's Pass (Approve).
    HumanPass,
    /// A human check's Fail (Send back).
    HumanFail,
    /// The work was delivered: the Local merge landed, or the PR was marked merged.
    Delivered,
    /// The PR was opened (or pushed again).
    PrOpened,
    /// PR rework: back from the open PR to the human check before the Delivery (d0041).
    PrRework,
    /// Send back with PR comments (FLW-16): from the open PR to the nearest earlier Agent stage, whose session
    /// resumes; every later stage runs again and the Delivery pushes to the same PR.
    PrComments,
    /// Mark closed, or Close ticket.
    Close,
    /// Into the queue from Backlog.
    Start,
    /// The scheduler starts a waiting ticket: into its Agent stage.
    Begin,
}

/// What the engine needs to know about a ticket.
#[derive(Debug, Clone, Copy)]
pub struct At<'f, 'a> {
    pub flow: &'f Flow,
    pub place: &'a Place,
    pub status: Status,
    /// The current agent check's fails in a row.
    pub fails: u32,
    /// The current agent check's limit ([`rounds_of`]).
    pub rounds: u32,
    /// The stages this ticket skips: its choices of `skip` ([`crate::board::Board::ticket_skips`], AGM-08).
    pub skips: &'a [String],
}

impl<'f, 'a> At<'f, 'a> {
    /// A ticket in `place` with `status` that skips nothing, its counter at 0 and its check's own rounds.
    pub fn new(flow: &'f Flow, place: &'a Place, status: Status) -> Self {
        let rounds = place
            .stage()
            .and_then(|id| flow.stage(id))
            .map_or(crate::flow::check::DEFAULT_VALIDATION_ROUNDS, |s| {
                rounds_of(s, 0)
            });
        Self {
            flow,
            place,
            status,
            fails: 0,
            rounds,
            skips: &[],
        }
    }

    fn index(&self) -> Option<usize> {
        self.place.stage().and_then(|id| self.flow.position(id))
    }

    fn stage(&self) -> Option<&'f Stage> {
        self.index().map(|i| &self.flow.stages[i])
    }
}

/// Whether the ticket skips `stage` (it can, and chose to).
fn skips(stage: &Stage, skips: &[String]) -> bool {
    stage.can_skip() && !stage.is_delivery() && skips.contains(&stage.id)
}

/// The ticket skips a human check of the flow: its Delivery is then in click mode (d0108: "a ticket that skipped a
/// human check this way always reaches Delivery in click mode").
fn skips_a_human_check(flow: &Flow, skipping: &[String]) -> bool {
    flow.stages
        .iter()
        .any(|s| s.is_human_check() && skips(s, skipping))
}

/// What a ticket's agent-check counters do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Counters {
    Keep,
    /// The current check counts the round that just ended: a fail adds to its fails in a row; a pass ends them (the
    /// default flow shows it as "Validated · round N").
    Count,
    /// The current check's counter restarts at 0 (Answer the agent).
    ResetThis,
    /// Every agent check's counter restarts at 0 (a human Fail, d0063's reset after Send back).
    ResetAll,
}

/// Where a move takes the ticket and what comes with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step<'f> {
    pub place: Place,
    pub status: Status,
    /// The stage the ticket is in afterwards, if any.
    pub stage: Option<&'f Stage>,
    /// How the run of the stage the ticket was in ends; `None` when that run goes on.
    pub ended: Option<RunResult>,
    /// Runs that start and end at once: stages walked over because the ticket skips them (or `SubmitByUser`
    /// walked past them), and a Delivery finished as soon as it was entered.
    pub passed_through: Vec<(&'f Stage, RunResult)>,
    pub counters: Counters,
    /// A fresh checker starts (an agent check was entered, or Retry).
    pub start_check: bool,
    /// An Agent stage that ran before takes the ticket back: its session resumes, with the findings or comments.
    pub resume: bool,
    /// A human check's Pass offers these delivery choices (Local merge, Open PR or both); the ticket stays put
    /// until the user picks one.
    pub choose: Option<Delivers>,
    /// The ticket entered an automatic Delivery stage: it delivers with no click.
    pub automatic: bool,
}

impl<'f> Step<'f> {
    fn stay(at: &At<'f, '_>, status: Status) -> Self {
        Self {
            place: at.place.clone(),
            status,
            stage: at.stage(),
            ended: None,
            passed_through: Vec::new(),
            counters: Counters::Keep,
            start_check: false,
            resume: false,
            choose: None,
            automatic: false,
        }
    }

    fn to(at: &At<'f, '_>, place: Place, status: Status, stage: Option<&'f Stage>) -> Self {
        Self {
            place,
            status,
            stage,
            ..Self::stay(at, status)
        }
    }

    fn ended(mut self, result: RunResult) -> Self {
        self.ended = Some(result);
        self
    }

    fn counters(mut self, counters: Counters) -> Self {
        self.counters = counters;
        self
    }
}

/// Why a move doesn't apply where the ticket stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotNow(pub String);

/// The stages after `from` (an index; `None` = before the first stage) up to the first one the ticket runs:
/// those it skips, then the one it enters. `None` when nothing follows (after the Delivery comes Done).
fn walk<'f>(
    flow: &'f Flow,
    skipping: &[String],
    from: Option<usize>,
    also_skip: impl Fn(&Stage) -> bool,
) -> (Vec<&'f Stage>, Option<&'f Stage>) {
    let start = from.map_or(0, |i| i + 1);
    let mut skipped = Vec::new();
    for stage in &flow.stages[start.min(flow.stages.len())..] {
        if skips(stage, skipping) || (also_skip(stage) && !stage.is_delivery()) {
            skipped.push(stage);
        } else {
            return (skipped, Some(stage));
        }
    }
    (skipped, None)
}

/// The nearest earlier Agent stage the ticket runs: where a Fail sends it back.
pub fn back_to<'f>(flow: &'f Flow, from: &str, skips: &[String]) -> Option<&'f Stage> {
    let index = flow.position(from)?;
    flow.stages[..index]
        .iter()
        .rev()
        .find(|s| s.is_agent_stage() && !self::skips(s, skips))
}

/// The first Agent stage the ticket runs: where a start takes it, and the stages it skips before it.
pub fn first_stage<'f>(flow: &'f Flow, skips: &[String]) -> (Vec<&'f Stage>, Option<&'f Stage>) {
    // Ready for Dev waits before the first Agent stage the ticket runs (checks never come before one, STG-02).
    walk(flow, skips, None, |s| !s.is_agent_stage())
}

/// Where a status written by the validation's own bookkeeping belongs, from the stage the ticket is in: a check's
/// status in that check, or (from its Agent stage, a stop the epic lead held at work) the next agent check; an Agent
/// stage's status in that stage, or (from a check) the Agent stage before it. `None` when the stage stays.
pub fn stage_for<'f>(flow: &'f Flow, place: &Place, status: Status) -> Option<&'f Stage> {
    let current = flow.stage(place.stage()?)?;
    let index = flow.position(&current.id)?;
    match status.set() {
        StatusSet::AgentCheck if current.is_agent_stage() => {
            flow.stages[index + 1..].iter().find(|s| s.is_agent_check())
        }
        StatusSet::AgentStage if current.is_agent_check() => back_to(flow, &current.id, &[]),
        _ => None,
    }
}

/// The first human check after `from`: where Move on anyway takes a card.
pub fn next_human_check<'f>(flow: &'f Flow, from: &str) -> Option<&'f Stage> {
    let index = flow.position(from)?;
    flow.stages[index + 1..].iter().find(|s| s.is_human_check())
}

/// The stage whose column a stage shows in ([`Flow::column_owner`]).
fn owner<'f>(flow: &'f Flow, stage: &Stage) -> Option<&'f Stage> {
    flow.position(&stage.id)
        .and_then(|i| flow.column_owner(i))
        .map(|i| &flow.stages[i])
}

/// Entering `stage` from where `at` stands, after walking over `skipped`.
fn enter<'f>(at: &At<'f, '_>, skipped: Vec<&'f Stage>, stage: Option<&'f Stage>) -> Step<'f> {
    let mut step = match stage {
        None => Step::to(at, Place::Done, Status::Merged, None),
        Some(stage) => {
            let mut step = Step::to(
                at,
                Place::stage_of(&stage.id),
                entry_status(stage),
                Some(stage),
            );
            step.start_check = stage.is_agent_check();
            step.automatic = stage
                .delivery
                .is_some_and(|d| d.mode == DeliveryMode::Automatic)
                && !skips_a_human_check(at.flow, at.skips);
            step
        }
    };
    step.passed_through = skipped
        .into_iter()
        .map(|s| (s, RunResult::Skipped))
        .collect();
    step
}

/// Moving on from a stage to `stage`, after walking over `skipped`: an Agent stage is waited for (STG-07, d0108
/// "waiting is automatic before every Agent stage"): the ticket waits before it, as Ready for Dev waits before the
/// first, until the scheduler starts it ([`Move::Begin`]). Any other stage is entered at once.
fn move_on<'f>(at: &At<'f, '_>, skipped: Vec<&'f Stage>, stage: Option<&'f Stage>) -> Step<'f> {
    match stage {
        Some(stage) if stage.is_agent_stage() => {
            let mut step = Step::to(
                at,
                Place::Waiting(Some(stage.id.clone())),
                Status::Queued,
                None,
            );
            step.passed_through = skipped
                .into_iter()
                .map(|s| (s, RunResult::Skipped))
                .collect();
            step
        }
        _ => enter(at, skipped, stage),
    }
}

/// Back to the nearest earlier Agent stage, which resumes its session.
fn back<'f>(at: &At<'f, '_>, from: &Stage) -> Result<Step<'f>, NotNow> {
    let to = back_to(at.flow, &from.id, at.skips).ok_or_else(|| {
        NotNow(format!(
            "{} has no Agent stage before it to send the ticket back to",
            from.name
        ))
    })?;
    let mut step = Step::to(at, Place::stage_of(&to.id), Status::Working, Some(to));
    step.resume = true;
    Ok(step)
}

fn agent_works(status: Status) -> bool {
    matches!(
        status,
        Status::Working
            | Status::WorkingInBackground
            | Status::AsksYou
            | Status::NeedsYou
            | Status::Idle
            | Status::MergeConflictFixing
    )
}

fn checking(status: Status) -> bool {
    matches!(status, Status::Checking | Status::CheckerNeedsYou)
}

/// What `mv` does where `at` stands.
pub fn step<'f>(at: At<'f, '_>, mv: Move) -> Result<Step<'f>, NotNow> {
    let not_now = || {
        Err(NotNow(format!(
            "{mv:?} doesn't apply while the ticket is {}",
            at.status.label()
        )))
    };
    let current = at.stage();
    let kind = current.map(|s| s.kind);
    let is = |k: StageKind| kind == Some(k);
    let agent_check = is(StageKind::Check { by: CheckBy::Agent });
    let human_check = is(StageKind::Check { by: CheckBy::You });
    let forward = |also_skip: &dyn Fn(&Stage) -> bool| {
        let (skipped, next) = walk(at.flow, at.skips, at.index(), also_skip);
        move_on(&at, skipped, next)
    };
    Ok(match mv {
        Move::Start if at.place.is_backlog() => {
            Step::to(&at, Place::Waiting(None), Status::Queued, None)
        }
        Move::Begin if at.place.is_waiting() => {
            let target = match at.place {
                Place::Waiting(Some(id)) => (Vec::new(), at.flow.stage(id)),
                _ => first_stage(at.flow, at.skips),
            };
            match target {
                (skipped, Some(stage)) => enter(&at, skipped, Some(stage)),
                (_, None) => return not_now(),
            }
        }
        Move::NextStage if is(StageKind::Agent) && agent_works(at.status) => {
            forward(&|_| false).ended(RunResult::Done)
        }
        Move::SubmitByUser if is(StageKind::Agent) && agent_works(at.status) => {
            let mut step = forward(&|s: &Stage| s.is_agent_check()).ended(RunResult::Done);
            step.automatic = false;
            step
        }
        Move::Pass if agent_check && checking(at.status) => forward(&|_| false)
            .ended(RunResult::Passed)
            .counters(Counters::Count),
        Move::Fail if agent_check && checking(at.status) => {
            let stage = current.expect("in a check");
            let step = if at.fails + 1 >= at.rounds {
                Step::stay(&at, Status::CheckFailed)
            } else {
                back(&at, stage)?
            };
            step.ended(RunResult::Failed).counters(Counters::Count)
        }
        Move::NoResult if agent_check && checking(at.status) => {
            Step::stay(&at, Status::CheckNoResult).ended(RunResult::NoResult)
        }
        Move::CancelCheck if agent_check && checking(at.status) => {
            let mut step = back(&at, current.expect("in a check"))?.ended(RunResult::Cancelled);
            step.resume = false;
            step
        }
        Move::RetryCheck if agent_check && at.status == Status::CheckNoResult => {
            let mut step = Step::stay(&at, Status::Checking);
            step.start_check = true;
            step
        }
        Move::SkipCheck if agent_check && checking(at.status) => {
            let mut step = forward(&|_| false).ended(RunResult::Skipped);
            step.automatic = false;
            step
        }
        Move::MoveOnAnyway
            if agent_check && matches!(at.status, Status::CheckFailed | Status::CheckNoResult) =>
        {
            let mut step = forward(&|_| false);
            step.automatic = false;
            step
        }
        Move::AnotherRound if agent_check && at.status == Status::CheckFailed => {
            back(&at, current.expect("in a check"))?
        }
        Move::AnswerAgent if agent_check && at.status == Status::CheckFailed => {
            back(&at, current.expect("in a check"))?.counters(Counters::ResetThis)
        }
        Move::HumanPass if human_check && at.status == Status::WaitingForYou => {
            let index = at.index().expect("in a check");
            match at.flow.stages.get(index + 1) {
                // The Delivery's own click is the Pass's (today's Approve → Local merge / Open PR).
                Some(next) if next.delivery.is_some_and(|d| d.mode == DeliveryMode::Click) => {
                    let mut step = Step::stay(&at, at.status);
                    step.choose = next.delivery.map(|d| d.delivers);
                    step
                }
                _ => forward(&|_| false).ended(RunResult::Passed),
            }
        }
        Move::HumanFail if human_check && at.status == Status::WaitingForYou => {
            back(&at, current.expect("in a check"))?
                .ended(RunResult::Failed)
                .counters(Counters::ResetAll)
        }
        // From the human check whose Pass offered the choice, or from the Delivery stage itself.
        Move::Delivered | Move::PrOpened
            if human_check
                && at.status == Status::WaitingForYou
                && at
                    .index()
                    .and_then(|i| at.flow.stages.get(i + 1))
                    .is_some_and(Stage::is_delivery) =>
        {
            let delivery = &at.flow.stages[at.index().expect("in a check") + 1];
            if mv == Move::Delivered {
                let mut step =
                    Step::to(&at, Place::Done, Status::Merged, None).ended(RunResult::Passed);
                step.passed_through = vec![(delivery, RunResult::Done)];
                step
            } else {
                Step::to(
                    &at,
                    Place::stage_of(&delivery.id),
                    Status::PrOpen,
                    Some(delivery),
                )
                .ended(RunResult::Passed)
            }
        }
        Move::Delivered if is(StageKind::Delivery) => {
            Step::to(&at, Place::Done, Status::Merged, None).ended(RunResult::Done)
        }
        Move::PrOpened if is(StageKind::Delivery) => Step::stay(&at, Status::PrOpen),
        Move::PrRework if is(StageKind::Delivery) && at.status == Status::PrOpen => {
            let index = at.index().expect("in a stage");
            let check = at.flow.stages[..index]
                .iter()
                .rev()
                .find(|s| s.is_human_check())
                .ok_or_else(|| NotNow("No human check before the Delivery".to_owned()))?;
            Step::to(
                &at,
                Place::stage_of(&check.id),
                Status::WaitingForYou,
                Some(check),
            )
            .ended(RunResult::Cancelled)
        }
        Move::PrComments if is(StageKind::Delivery) && at.status == Status::PrOpen => {
            back(&at, current.expect("in a stage"))?
                .ended(RunResult::Cancelled)
                .counters(Counters::ResetAll)
        }
        Move::Close if !at.place.is_done() => {
            Step::to(&at, Place::Done, Status::ClosedNotMerged, None).ended(RunResult::Cancelled)
        }
        _ => return not_now(),
    })
}

/// Where a drag drops a card, in flow terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<'f> {
    Backlog,
    /// Ready for Dev.
    Waiting,
    Stage(&'f Stage),
    Done,
}

impl DropTarget {
    /// The place this board target means in `flow`: Dev is the first Agent stage's column, Ready for Review the
    /// human check before the Delivery, Approved the Delivery (the default flow's Review column and its Approved
    /// section). `None` when the flow has no such stage.
    pub fn in_flow(self, flow: &Flow) -> Option<Target<'_>> {
        let delivery = flow.stages.iter().position(Stage::is_delivery);
        Some(match self {
            Self::Backlog => Target::Backlog,
            Self::ReadyForDev => Target::Waiting,
            Self::Dev => Target::Stage(flow.first_agent_stage()?),
            Self::ReadyForReview => Target::Stage(
                flow.stages[..delivery.unwrap_or(flow.stages.len())]
                    .iter()
                    .rev()
                    .find(|s| s.is_human_check())?,
            ),
            Self::Approved => Target::Stage(&flow.stages[delivery?]),
            Self::Done => Target::Done,
        })
    }
}

/// A drag's verdict: today's drag rules (§7.4), by the kinds of the stages it leaves and enters.
pub fn drag(
    flow: &Flow,
    place: &Place,
    status: Status,
    kind: Kind,
    target: DropTarget,
) -> MoveCheck {
    let refused = |reason: &str| MoveCheck::Refused {
        reason: reason.to_owned(),
    };
    let Some(to) = target.in_flow(flow) else {
        return refused(ONLY_AGENT_SUBMITS);
    };
    if status == Status::Merged && to != Target::Done {
        return refused(MERGED_LOCKED);
    }
    let from = match place {
        Place::Backlog => Target::Backlog,
        Place::Waiting(_) => Target::Waiting,
        Place::Done => Target::Done,
        Place::InStage(id) => match flow.stage(id) {
            Some(stage) => Target::Stage(stage),
            // A stage the flow no longer has (Validation turned off while it validated): where its kind shows.
            None => match status.set() {
                StatusSet::HumanCheck => DropTarget::ReadyForReview,
                StatusSet::Delivery => DropTarget::Approved,
                _ => DropTarget::Dev,
            }
            .in_flow(flow)
            .unwrap_or(Target::Backlog),
        },
    };
    if from == to {
        return MoveCheck::Unchanged;
    }
    let at_work = |s: &Stage| s.is_agent_stage() || s.is_agent_check();
    let right_after = |a: &Stage, b: &Stage| {
        flow.position(&a.id)
            .zip(flow.position(&b.id))
            .is_some_and(|(a, b)| b == a + 1)
    };
    match (from, to) {
        // Dropped on the column it shows in: an agent check folded into its Agent stage's column.
        (Target::Stage(a), Target::Stage(b))
            if at_work(a) && a.column == Column::WithPrevious && owner(flow, a) == Some(b) =>
        {
            MoveCheck::Unchanged
        }
        // An agent check in its own column (the default flow's Validate, FLW-01) dropped on the Agent stage before
        // it: nothing happens, as when Validate was folded into Dev (drags back with instructions are T-0421's).
        (Target::Stage(a), Target::Stage(b))
            if a.is_agent_check()
                && b.is_agent_stage()
                && flow
                    .position(&b.id)
                    .zip(flow.position(&a.id))
                    .is_some_and(|(b, a)| b < a) =>
        {
            MoveCheck::Unchanged
        }
        // An epic in Backlog is its card or its track (EPC-11): the drop queues its children (d0080).
        (Target::Backlog, Target::Waiting) if kind == Kind::Epic => MoveCheck::Allowed {
            action: MoveAction::QueueEpic,
        },
        (Target::Backlog, Target::Stage(b)) if b.is_agent_stage() && kind == Kind::Epic => {
            MoveCheck::Allowed {
                action: MoveAction::QueueEpic,
            }
        }
        // The go-ahead; on an Agent stage it waits until it can start (d0066).
        (Target::Backlog, Target::Waiting) => MoveCheck::Allowed {
            action: MoveAction::Start,
        },
        (Target::Backlog, Target::Stage(b)) if b.is_agent_stage() => MoveCheck::Allowed {
            action: MoveAction::Start,
        },
        (Target::Waiting, Target::Backlog) => MoveCheck::Allowed {
            action: MoveAction::Unqueue,
        },
        (Target::Waiting, Target::Stage(b)) if b.is_agent_stage() => refused(STARTS_BY_ITSELF),
        (Target::Stage(a), Target::Waiting) if at_work(a) => {
            refused("Drag it to Backlog to park it")
        }
        (Target::Stage(a), Target::Backlog) if at_work(a) => MoveCheck::Confirm {
            action: MoveAction::Park,
        },
        (Target::Stage(a), Target::Stage(b))
            if a.is_human_check() && b.is_delivery() && right_after(a, b) =>
        {
            MoveCheck::Choose {
                action: MoveAction::Approve,
            }
        }
        (Target::Stage(a), Target::Stage(b))
            if a.is_delivery() && b.is_human_check() && right_after(b, a) =>
        {
            MoveCheck::Allowed {
                action: MoveAction::PrRework,
            }
        }
        (Target::Done, Target::Backlog) => MoveCheck::Allowed {
            action: MoveAction::Reopen,
        },
        (_, Target::Stage(b)) if b.is_human_check() => refused(ONLY_AGENT_SUBMITS),
        (Target::Done, _) => refused("Drag it to Backlog to reopen it first"),
        (Target::Stage(a), Target::Waiting | Target::Stage(_))
            if a.is_human_check() && !matches!(to, Target::Stage(b) if b.is_delivery()) =>
        {
            refused("Use Send back to return it to Dev with your comments")
        }
        (_, Target::Waiting) => {
            refused("Drag it back through Ready for Review: Approved tickets return with PR rework")
        }
        (_, Target::Stage(b)) if at_work(b) => {
            refused("Drag it back through Ready for Review: Approved tickets return with PR rework")
        }
        (_, Target::Stage(_)) => refused("Approve a ticket from Ready for Review"),
        (_, Target::Done) => refused("Use Close ticket, Approve or Mark merged to finish a ticket"),
        (_, Target::Backlog) => {
            refused("Only Ready for Dev, Dev and Closed tickets go back to Backlog")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::default::{DELIVER, default_flow};

    #[test]
    fn default_stages_of_every_status() {
        for status in Status::ALL {
            let stage = default_stage(*status, None);
            assert_eq!(stage.is_some(), status.set().in_stage(), "{status}");
            assert!(stage.is_none_or(|id| default_flow().stage(id).is_some()));
        }
        assert_eq!(default_stage(Status::PrOpen, None), Some(DELIVER));
        assert_eq!(
            default_stage(Status::WaitingForLimitReset, Some(Status::Checking)),
            Some(VALIDATE)
        );
    }

    #[test]
    fn the_first_stage_and_back() {
        let flow = default_flow();
        assert_eq!(first_stage(&flow, &[]).1.map(|s| s.id.as_str()), Some(DEV));
        assert_eq!(
            back_to(&flow, REVIEW, &[]).map(|s| s.id.as_str()),
            Some(DEV)
        );
        assert_eq!(back_to(&flow, DEV, &[]), None);
        assert_eq!(
            next_human_check(&flow, DEV).map(|s| s.id.as_str()),
            Some(REVIEW)
        );
    }
}
