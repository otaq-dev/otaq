//! The presets (T-0415; FLW-08, FLW-22; d0108; design `s14-18`, `s15-62`): the four shipped flow files read, write
//! back unchanged, pass the checks with the agents they define, and show the columns, lines and file names the
//! design draws. Each sets no `skip`, has Validate in its own column and delivers automatically only after a check.

use std::path::Path;

use otaq_core::agents::shipped;
use otaq_core::flow::presets::{self, preset, presets};
use otaq_core::flow::{Column, DeliveryMode, check, check_agents, yaml};
use otaq_core::naming;

#[test]
fn the_four_presets_are_listed_as_drawn() {
    let listed: Vec<(&str, &str, &str, Vec<String>, String)> = presets()
        .iter()
        .map(|p| (p.name.as_str(), p.line, p.file, p.columns(), p.delivers()))
        .collect();
    let cols = |names: &[&str]| names.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();
    let default_name = format!("{} default", naming::PRODUCT_NAME);
    assert_eq!(
        listed,
        [
            (
                default_name.as_str(),
                "Today\u{2019}s board",
                naming::DEFAULT_FLOW_FILE,
                cols(&["Ready for Dev", "Dev", "Validate", "Review", "Done"]),
                "Done \u{b7} click".to_owned(),
            ),
            (
                "Solo autopilot",
                "Solo projects that trust the checks",
                "solo-autopilot.yaml",
                cols(&["Ready for Dev", "Dev", "Validate", "Done"]),
                "Done \u{b7} Local merge, automatic".to_owned(),
            ),
            (
                "Grill first",
                "Settle the ticket with you before any code is written",
                "grill-first.yaml",
                cols(&[
                    "Ready for Dev",
                    "Grill",
                    "Dev",
                    "Validate",
                    "Review",
                    "Done"
                ]),
                "Done \u{b7} click".to_owned(),
            ),
            (
                "Team PR",
                "Teams that review on GitHub or GitLab: the PR is the review",
                "team-pr.yaml",
                cols(&["Ready for Dev", "Dev", "Validate", "Done"]),
                "Done \u{b7} PR, automatic".to_owned(),
            ),
        ]
    );
    assert_eq!(preset("TEAM-PR.yaml").map(|p| p.file), Some("team-pr.yaml"));
    assert_eq!(
        preset("grill first").map(|p| p.file),
        Some("grill-first.yaml")
    );
    assert_eq!(preset("plan-first.yaml"), None);
}

/// Each embedded preset is the file on disk, reads, writes back unchanged and passes the flow's and the agents'
/// checks with its own agents, whose skills Otaq ships and whose model is `default`.
#[test]
fn every_preset_reads_and_passes_the_checks() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("flows");
    for p in presets() {
        let on_disk = std::fs::read_to_string(dir.join(p.file)).unwrap();
        assert_eq!(on_disk, p.yaml, "{}", p.file);
        let file = p.read();
        assert_eq!(yaml::write(&file), p.yaml, "{} round-trips", p.file);
        assert_eq!(check(&file.flow), Ok(()), "{}", p.file);
        let known = |name: &str| file.agents.iter().any(|a| a.name == name);
        assert_eq!(check_agents(&file.flow, known), Ok(()), "{}", p.file);
        for agent in &file.agents {
            assert_eq!(agent.model, "default", "{}: {}", p.file, agent.name);
            for skill in &agent.skills {
                assert!(
                    shipped::SKILLS.contains(&skill.as_str()),
                    "{}: {} names {skill}, which Otaq doesn't ship",
                    p.file,
                    agent.name
                );
            }
        }
        for stage in &file.flow.stages {
            if let Some(skill) = &stage.skill {
                assert!(shipped::SKILLS.contains(&skill.as_str()), "{skill}");
            }
        }
    }
}

/// The presets set no skip, show Validate in its own column, and deliver automatically only after a check (d0108:
/// "The onboarding agent and the presets don't set a skip on their own"; changelog 2026-10-20, Validate's column).
#[test]
fn presets_skip_nothing_and_check_before_an_automatic_delivery() {
    for p in presets() {
        let flow = p.read().flow;
        assert!(flow.stages.iter().all(|s| !s.can_skip()), "{}", p.file);
        let validate = flow.stage("validate").expect("every preset validates");
        assert!(validate.is_agent_check() && validate.column == Column::Own);
        let (at, delivery) = flow
            .stages
            .iter()
            .enumerate()
            .find_map(|(i, s)| s.delivery.map(|d| (i, d)))
            .unwrap();
        if delivery.mode == DeliveryMode::Automatic {
            assert!(
                flow.stages[..at]
                    .iter()
                    .any(|s| s.is_agent_check() || s.is_human_check()),
                "{}",
                p.file
            );
        }
    }
}

/// Grill first's Grill runs Griller with `grill`; Team PR's Done opens the PR with no click, run by Shipper with
/// `open-pr`; Solo autopilot merges locally with no agent; the default flow's Delivery names `open-pr` (Q254).
#[test]
fn the_presets_stages_run_their_agents_and_skills() {
    let stage = |file: &str, id: &str| {
        let flow = preset(file).unwrap().read().flow;
        let s = flow.stage(id).unwrap().clone();
        (s.agent, s.skill, s.delivery.map(|d| (d.mode, d.delivers)))
    };
    use otaq_core::flow::Delivers;
    let some = |s: &str| Some(s.to_owned());
    assert_eq!(
        stage("grill-first.yaml", "grill"),
        (some("Griller"), some("grill"), None)
    );
    assert_eq!(
        preset("grill-first.yaml").unwrap().read().flow.stages[0].id,
        "grill"
    );
    assert_eq!(
        stage("team-pr.yaml", "deliver"),
        (
            some("Shipper"),
            some("open-pr"),
            Some((DeliveryMode::Automatic, Delivers::Pr))
        )
    );
    assert_eq!(
        stage("solo-autopilot.yaml", "deliver"),
        (None, None, Some((DeliveryMode::Automatic, Delivers::Local)))
    );
    assert_eq!(
        stage(naming::DEFAULT_FLOW_FILE, "deliver"),
        (
            some("Developer"),
            some("open-pr"),
            Some((DeliveryMode::Click, Delivers::Either))
        )
    );
    // The default flow is the first preset, its file the default's.
    assert_eq!(presets::presets()[0].yaml, naming::DEFAULT_FLOW_YAML);
}

/// An agent several presets define is the same agent in each (agents are shared by name, so a second preset adds
/// no import clash for Developer or Validator).
#[test]
fn shared_agents_read_the_same_in_every_preset() {
    let mut seen: Vec<otaq_core::flow::FlowAgent> = Vec::new();
    for p in presets() {
        for agent in p.read().agents {
            match seen.iter().find(|a| a.name == agent.name) {
                Some(first) => assert_eq!(first, &agent, "{}: {}", p.file, agent.name),
                None => seen.push(agent),
            }
        }
    }
    let names: Vec<&str> = seen.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["Developer", "Validator", "Griller", "Shipper"]);
}
