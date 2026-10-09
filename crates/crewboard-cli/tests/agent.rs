//! `crewboard ask human` and `crewboard review submit` (T-0072, ATT-01, REV-07) from inside an agent session,
//! with a temporary git repository as the ticket's worktree.

use std::path::Path;
use std::process::Command as Process;

use crewboard_core::domain::Status;
use crewboard_core::service::testing::{AgentShell, TestService};
use serde_json::{Value, json};

const CB: &str = env!("CARGO_BIN_EXE_crewboard");

fn git(dir: &Path, args: &[&str]) {
    let ok = Process::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "git {args:?}");
}

async fn stop_hook(agent: &AgentShell) {
    let payload = json!({"hook_event_name": "Stop", "background_tasks": [], "session_crons": []});
    let ran = agent
        .run_with_stdin(&format!("'{CB}' hook Stop"), &payload.to_string())
        .await;
    assert_eq!(ran.status, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn ask_human_shows_asks_you_with_the_question() {
    let w = TestService::start().await;
    w.shop_sample().await;
    let shop19 = w.board.ticket("SHOP-19").await.unwrap();
    let agent = w.agent_shell(&shop19).await;

    let question = "Should `fullName` be split into first and last name everywhere, or only in the profile form?";
    let ran = agent
        .run_with_stdin(&format!("'{CB}' ask human --stdin --json"), question)
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    let ticket: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(ticket["status"], "asks_you");
    assert_eq!(ticket["signal"]["text"], question);

    // The turn's Stop keeps Asks you (not Idle).
    stop_hook(&agent).await;
    assert_eq!(w.status("SHOP-19").await, Status::AsksYou);

    // An empty question or a missing --stdin is refused.
    let ran = agent
        .run_with_stdin(&format!("'{CB}' ask human --stdin --json"), "  \n")
        .await;
    assert_ne!(ran.status, 0);
    let ran = agent.run(&format!("'{CB}' ask human")).await;
    assert_ne!(ran.status, 0);
    assert!(ran.stderr.contains("--stdin"), "{ran:?}");
    let ran = agent
        .run_with_stdin(&format!("'{CB}' ask SHOP-18 --stdin"), "x")
        .await;
    assert_ne!(ran.status, 0);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn review_submit_refuses_uncommitted_files_then_submits() {
    let w = TestService::start().await;
    w.shop_sample().await;
    let shop19 = w.board.ticket("SHOP-19").await.unwrap();
    let worktree = w.give_worktree(&shop19).await;
    let agent = w.agent_shell(&shop19).await;
    let submit = format!("'{CB}' review submit --stdin");

    // A modified tracked file.
    std::fs::write(worktree.join("file1.txt"), "changed\n").unwrap();
    let ran = agent.run_with_stdin(&submit, "Price table done.").await;
    assert_ne!(ran.status, 0);
    assert_eq!(
        ran.stderr.trim(),
        "error: 1 file is not committed. Commit it, or discard it if it's a scratch file"
    );
    // …and an untracked one.
    std::fs::write(worktree.join("notes.tmp"), "scratch\n").unwrap();
    let ran = agent
        .run_with_stdin(&format!("{submit} --json"), "Price table done.")
        .await;
    let e: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(
        e["error"]["message"],
        "2 files are not committed. Commit them, or discard them if they're scratch files"
    );
    assert_eq!(e["error"]["data"]["refusal"], "uncommitted");
    assert_eq!(w.status("SHOP-19").await, Status::Working, "stays in Dev");

    // Committed: Ready for Review, with the summary and the review head.
    std::fs::remove_file(worktree.join("notes.tmp")).unwrap();
    git(&worktree, &["commit", "-qam", "SHOP-19: price table"]);
    let ran = agent
        .run_with_stdin(&submit, "Price table with sorting.\nPrices align right.")
        .await;
    assert_eq!(
        (ran.status, ran.stdout.as_str()),
        (0, "SHOP-19 is Ready for Review.\n"),
        "{ran:?}"
    );
    let row = w.board.store().ticket(&shop19.id).await.unwrap().unwrap();
    assert_eq!(row.state, "ready_for_review");
    let head = String::from_utf8(
        Process::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&worktree)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(row.review_head_sha.as_deref(), Some(head.trim()));
    assert!(row.review_entered_at.is_some());
    assert_eq!(
        w.board.review_summary(&shop19.id).await.unwrap().as_deref(),
        Some("Price table with sorting.\nPrices align right.")
    );
    // SHOP-20's prime shows it.
    let prime = crewboard_core::prime::render(&w.board.prime_context("SHOP-20").await.unwrap());
    assert!(
        prime.contains("  > Price table with sorting.\n  > Prices align right.\n"),
        "{prime}"
    );

    // The turn's Stop doesn't touch it.
    stop_hook(&agent).await;
    assert_eq!(w.status("SHOP-19").await, Status::ReadyForReview);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_submit_fact_keeps_stop_from_turning_idle() {
    let w = TestService::start().await;
    w.shop_sample().await;
    let shop19 = w.board.ticket("SHOP-19").await.unwrap();
    w.give_worktree(&shop19).await;
    let agent = w.agent_shell(&shop19).await;
    let ran = agent
        .run_with_stdin(&format!("'{CB}' review submit --stdin"), "done")
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    assert!(w.signals.turn(&agent.session).submitted_or_asked);
    w.stop().await;
}
