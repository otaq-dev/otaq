//! The checks on a flow (STG-02; d0108's fixed rules and smaller rule 9, the second gap review's rules 1, 2 and 6).
//! Each problem names the stage and the rule in plain words, as `otaq flow propose` and the preview will show it.

use std::collections::HashSet;

use super::model::{Column, Delivers, DeliveryMode, Flow, Stage};
use crate::board::settings::VALIDATION_ROUNDS;

/// The rule a problem breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rule {
    /// Exactly one Delivery stage…
    OneDelivery,
    /// …and it is last.
    DeliveryLast,
    /// At least one Agent stage.
    AgentStage,
    /// No human check before the first Agent stage.
    HumanCheckFirst,
    /// `with-previous` on the first stage.
    FoldFirst,
    /// `with-previous` into a column owned by a human check.
    FoldIntoHumanCheck,
    /// `either` only in click mode.
    EitherClickOnly,
    /// An agent check's rounds are 1–5.
    Rounds,
    /// A Delivery can't be skipped.
    SkipDelivery,
    /// With the skips taken, no Agent stage runs.
    SkipLeavesNoAgent,
    /// With the skips taken, a human check has no running Agent stage before it.
    SkipStrandsHumanCheck,
    /// Two stages with one id or one name.
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub rule: Rule,
    /// The id of the stage it is about, when it is about one.
    pub stage: Option<String>,
    pub message: String,
}

/// Every problem with the flow, in stage order; `Ok` when there is none.
pub fn check(flow: &Flow) -> Result<(), Vec<Problem>> {
    let mut problems = Vec::new();
    let stages = &flow.stages;
    let mut add = |rule, stage: Option<&Stage>, message: String| {
        problems.push(Problem {
            rule,
            stage: stage.map(|s| s.id.clone()),
            message,
        });
    };

    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for stage in stages {
        if !ids.insert(stage.id.as_str()) {
            add(
                Rule::Duplicate,
                Some(stage),
                format!(
                    "Two stages have the id `{}`: each stage needs its own",
                    stage.id
                ),
            );
        }
        if !names.insert(stage.name.as_str()) {
            add(
                Rule::Duplicate,
                Some(stage),
                format!(
                    "Two stages are named {}: each stage needs its own name",
                    stage.name
                ),
            );
        }
    }

    // One Delivery stage, last.
    let deliveries: Vec<&Stage> = stages.iter().filter(|s| s.is_delivery()).collect();
    match deliveries.as_slice() {
        [] => add(
            Rule::OneDelivery,
            None,
            "The flow has no Delivery stage: it must end in one that merges or opens a PR"
                .to_owned(),
        ),
        [_] => {}
        [first, second, ..] => add(
            Rule::OneDelivery,
            Some(second),
            format!(
                "{} and {} are both Delivery stages: a flow has exactly one",
                first.name, second.name
            ),
        ),
    }
    let last = stages.len().saturating_sub(1);
    for misplaced in stages[..last].iter().filter(|s| s.is_delivery()) {
        add(
            Rule::DeliveryLast,
            Some(misplaced),
            format!(
                "{} is the Delivery stage but isn't last: the flow ends with it",
                misplaced.name
            ),
        );
    }

    // At least one Agent stage, and no human check before the first one.
    let first_agent = stages.iter().position(Stage::is_agent_stage);
    if first_agent.is_none() {
        add(
            Rule::AgentStage,
            None,
            "The flow has no Agent stage: at least one stage needs an agent working on the ticket"
                .to_owned(),
        );
    }
    for stage in &stages[..first_agent.unwrap_or(stages.len())] {
        if stage.is_human_check() {
            add(
                Rule::HumanCheckFirst,
                Some(stage),
                format!(
                    "{} is a check by you before any Agent stage: a Fail would have nowhere to go",
                    stage.name
                ),
            );
        }
    }

    // Folding limits.
    for (i, stage) in stages.iter().enumerate() {
        if stage.column != Column::WithPrevious {
            continue;
        }
        if i == 0 {
            add(
                Rule::FoldFirst,
                Some(stage),
                format!(
                    "{} is the first stage: it can't share the previous column, since Ready for Dev comes before it",
                    stage.name
                ),
            );
            continue;
        }
        // The Delivery stage may fold into a human check's column: the default flow (STG-03) shows its cards in
        // Review's Approved section, as today (d0108). The second gap review's rule 1 refuses this for every other
        // stage, whose card would sit under a check by you while an agent works on it.
        let owner = flow.column_owner(i).map(|o| &stages[o]);
        if let Some(owner) = owner.filter(|o| o.is_human_check() && !stage.is_delivery()) {
            add(
                Rule::FoldIntoHumanCheck,
                Some(stage),
                format!(
                    "{} can't share {}'s column: that column belongs to a check by you",
                    stage.name, owner.name
                ),
            );
        }
    }

    for stage in stages {
        if let Some(rounds) = stage.rounds.filter(|r| !VALIDATION_ROUNDS.contains(r)) {
            add(
                Rule::Rounds,
                Some(stage),
                format!(
                    "{} has {rounds} rounds: use {} to {}",
                    stage.name,
                    VALIDATION_ROUNDS.start(),
                    VALIDATION_ROUNDS.end()
                ),
            );
        }
        if stage
            .delivery
            .is_some_and(|d| d.delivers == Delivers::Either && d.mode == DeliveryMode::Automatic)
        {
            add(
                Rule::EitherClickOnly,
                Some(stage),
                format!(
                    "{} delivers `either` automatically: choosing between a Local merge and a PR needs your click, \
                     so use click mode or deliver `local` or `pr`",
                    stage.name
                ),
            );
        }
        if stage.is_delivery() && stage.can_skip() {
            add(
                Rule::SkipDelivery,
                Some(stage),
                format!("{} is the Delivery stage: it can't be skipped", stage.name),
            );
        }
    }

    // Skips. A rule broken by some combination of skips is broken by the combination that skips every skippable
    // Agent stage up to that point and nothing else (more skips only remove running Agent stages; a skipped human
    // check has nothing to send back), so checking those cases covers every combination, and names the smallest.
    let skippable = |s: &Stage| s.is_agent_stage() && s.can_skip();
    let agents: Vec<&Stage> = stages.iter().filter(|s| s.is_agent_stage()).collect();
    if !agents.is_empty() && agents.iter().all(|s| skippable(s)) {
        add(
            Rule::SkipLeavesNoAgent,
            None,
            format!("If {} skipped, no Agent stage runs", skipped(&agents)),
        );
    }
    for (i, stage) in stages.iter().enumerate() {
        let before: Vec<&Stage> = stages[..i].iter().filter(|s| s.is_agent_stage()).collect();
        // With none before it at all, HumanCheckFirst already says so.
        if stage.is_human_check() && !before.is_empty() && before.iter().all(|s| skippable(s)) {
            add(
                Rule::SkipStrandsHumanCheck,
                Some(stage),
                format!(
                    "If {} skipped, a Fail at {} has nowhere to go",
                    skipped(&before),
                    stage.name
                ),
            );
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

/// "Grill is", "Grill and Dev are both", "Grill, Plan and Dev are all".
fn skipped(stages: &[&Stage]) -> String {
    let names: Vec<&str> = stages.iter().map(|s| s.name.as_str()).collect();
    match names.as_slice() {
        [one] => format!("{one} is"),
        [a, b] => format!("{a} and {b} are both"),
        [rest @ .., last] => format!("{} and {last} are all", rest.join(", ")),
        [] => unreachable!("a skip case names at least one stage"),
    }
}
