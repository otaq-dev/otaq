//! Ask Claude (T-0135, RUN-02): only on a click, `claude` runs in the project's folder and suggests a run command;
//! progress and the outcome arrive as events; Cancel stops it; nothing is saved. An ordinary shell command that
//! prints a fixed stream-json answer stands in for `claude`.

use std::time::Duration;

use crewboard_core::board::Refusal;
use crewboard_core::protocol::Event;
use crewboard_core::service::testing::TestService;
use crewboard_core::terminals::suggest::{SuggestUpdate, Suggestion};

/// A stand-in for `claude` that prints `lines` (stream-json) and exits with `code`.
fn stand_in(lines: &[&str], code: i32) -> Vec<String> {
    let mut script = String::new();
    for line in lines {
        script.push_str(&format!("printf '%s\\n' '{line}'; "));
    }
    script.push_str(&format!("echo 'stand-in done' >&2; exit {code}"));
    vec!["sh".into(), "-c".into(), script, "stand-in".into()]
}

async fn updates(
    receiver: &mut tokio::sync::broadcast::Receiver<Event>,
    project: &str,
) -> Vec<SuggestUpdate> {
    let mut all = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(Event::RunSuggestion { project: p, update }) = receiver.recv().await
                && p == project
            {
                let last = !matches!(update, SuggestUpdate::Progress { .. });
                all.push(update);
                if last {
                    return;
                }
            }
        }
    })
    .await
    .expect("the suggestion ends");
    all
}

#[tokio::test(flavor = "multi_thread")]
async fn a_suggestion_with_progress() {
    let tool = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"package.json"}}]}}"#;
    let result = r#"{"type":"result","is_error":false,"result":"{\"command\": \"pnpm dev --port $PORT\", \"open_url\": \"http://localhost:$PORT\", \"why\": \"package.json has dev.\"}"}"#;
    let w = TestService::start_with(|mut c| {
        c.ask_claude = Some(stand_in(&[tool, result], 0));
        c
    })
    .await;
    w.add_project("shop-web").await;
    let mut listener = w.events.listen();
    w.suggester.start("SHOP").await.unwrap();
    let got = updates(&mut listener.receiver, "SHOP").await;
    assert_eq!(
        got,
        [
            SuggestUpdate::Progress {
                line: "Read(package.json)".into()
            },
            SuggestUpdate::Done {
                suggestion: Suggestion {
                    command: "pnpm dev --port $PORT".into(),
                    open_url: "http://localhost:$PORT".into(),
                    why: "package.json has dev.".into(),
                }
            }
        ]
    );
    // Nothing was saved: the project still has no run command.
    let settings = w.board.project_settings("SHOP").await.unwrap();
    assert!(settings.run_command.is_empty());
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn no_answer_is_a_failure() {
    let w = TestService::start_with(|mut c| {
        c.ask_claude = Some(stand_in(&[], 3));
        c
    })
    .await;
    w.add_project("shop-web").await;
    let mut listener = w.events.listen();
    w.suggester.start("SHOP").await.unwrap();
    let got = updates(&mut listener.receiver, "SHOP").await;
    assert_eq!(
        got,
        [SuggestUpdate::Failed {
            message: "Claude stopped without a suggestion (exit 3): stand-in done.".into()
        }]
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_stops_it_and_without_claude_it_is_refused() {
    let dir = tempfile::TempDir::new().unwrap();
    let pid_file = dir.path().join("suggest.pid");
    let script = format!("echo $$ > '{}'; exec sleep 30", pid_file.display());
    let w = TestService::start_with(|mut c| {
        c.ask_claude = Some(vec!["sh".into(), "-c".into(), script, "stand-in".into()]);
        c
    })
    .await;
    w.add_project("shop-web").await;
    let mut listener = w.events.listen();
    w.suggester.start("SHOP").await.unwrap();
    let pid: i32 = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(text) = std::fs::read_to_string(&pid_file)
                && let Ok(pid) = text.trim().parse()
            {
                return pid;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(w.suggester.cancel("SHOP"));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(unsafe { libc::kill(pid, 0) } != 0, "the process ended");
    assert!(
        listener.receiver.try_recv().is_err(),
        "a cancelled suggestion says nothing"
    );
    assert!(!w.suggester.cancel("SHOP"), "nothing left to cancel");
    w.stop().await;

    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let err = w.suggester.start("SHOP").await.unwrap_err();
    assert_eq!(err.refusal(), Some(Refusal::NotAvailableYet));
    w.stop().await;
}
