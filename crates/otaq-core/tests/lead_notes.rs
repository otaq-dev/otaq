//! The lead's notes to running children (T-0281; LEAD-10, CLI-07, CLI-08; d0083, d0048), through the socket:
//! requests sent from inside the lead's session (an ordinary `sh` started as the lead) and the child's agent session
//! (an `sh` started as SHOP-40.2's agent) are judged as them. A note to a running child is stored unread with
//! History on both; a never-started or finished child and anyone but the lead are refused. `notes` and the agent's
//! `prime` mark them read; `review submit` is refused while one is unread, the user's Submit isn't; at a Stop the
//! note is typed in instead of the child turning Idle.

use std::time::Duration;

use otaq_core::agent_tools::Tool;
use otaq_core::board::tickets::EventKind;
use otaq_core::caller::Caller;
use otaq_core::client::{Client, ClientConfig, ClientError};
use otaq_core::domain::{Actor, Status};
use otaq_core::epic_lead::notes::{self, ONLY_A_LEAD_NOTES, ReadHow};
use otaq_core::git::testing::TempRepos;
use otaq_core::protocol::Request;
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::{AgentShell, TestService, path_without_build_folders};
use otaq_core::session::launch::LaunchContext;
use serde_json::{Value, json};

const NOTE: &str =
    "40.1 returns items newest first (recorded decision on SHOP-40.1). Use the same order.";

/// The sample board (SHOP-40 Wishlist: 40.1 Merged, 40.2 Working, 40.3 Backlog); `claude` is an interactive `sh`
/// in the service's data folder. SHOP-40.2 has a clean worktree.
async fn world() -> TestService {
    let home = tempfile::TempDir::new().unwrap();
    let data = home.path().join("h");
    let script = format!(
        "export {}='{}'; export PS1=''; exec sh",
        otaq_core::data_dir::DataDir::env_var(),
        data.display()
    );
    let w = TestService::start_in(home, TempRepos::new(), |config| ServiceConfig {
        launch: Some(LaunchContext {
            claude: vec!["sh".into(), "-c".into(), script, "sh".into()],
            bin_dir: "/nonexistent".into(),
            plugin_dir: "/nonexistent/plugin".into(),
            path: path_without_build_folders().into(),
            claude_config_dir: None,
        }),
        ..config
    })
    .await;
    w.shop_sample().await;
    let child = w.board.ticket("SHOP-40.2").await.unwrap();
    w.give_worktree(&child).await;
    w
}

async fn lead_shell(w: &TestService) -> AgentShell {
    let lead = w.project_agents.leads().open("SHOP-40").await.unwrap();
    w.shell_in(&lead.agent.session.as_ref().unwrap().id)
}

async fn send(w: &TestService, shell: &AgentShell, method: &str, params: Value) -> Value {
    let request =
        json!({"protocol_version": 1, "id": 1, "request": {"method": method, "params": params}});
    let ran = shell
        .send_raw(&w.data.socket_path(), &request.to_string())
        .await;
    serde_json::from_str(ran.stdout.lines().next().unwrap_or_default())
        .unwrap_or_else(|e| panic!("{e}: {ran:?}"))
}

async fn send_as_user(w: &TestService, method: &str, params: Value) -> Value {
    let request: Request =
        serde_json::from_value(json!({"method": method, "params": params})).unwrap();
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    match client.request(request).await {
        Ok(reply) => json!({"reply": serde_json::to_value(reply).unwrap()}),
        Err(ClientError::Service(e)) => json!({"error": serde_json::to_value(e).unwrap()}),
        Err(e) => panic!("{method}: {e}"),
    }
}

fn ok(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "{answer}");
    &answer["reply"]
}

fn refused(answer: &Value) -> &str {
    assert_eq!(answer["error"]["kind"], "refused", "{answer}");
    answer["error"]["message"].as_str().unwrap()
}

async fn unread_count(w: &TestService, key: &str) -> usize {
    let id = w.board.ticket_id(key).await.unwrap();
    notes::unread(&w.board, &id).await.unwrap().len()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_note_to_a_running_child_is_stored_unread_with_history_on_both() {
    let w = world().await;
    let lead = lead_shell(&w).await;

    let answer = send(
        &w,
        &lead,
        "lead.note",
        json!({"key": "SHOP-40.2", "text": NOTE}),
    )
    .await;
    let note = ok(&answer)["lead_note"].clone();
    assert_eq!(note["child"], "SHOP-40.2");
    assert_eq!(note["lead"], "Wishlist lead");
    assert_eq!(note["text"], NOTE);
    assert!(note.get("read_at").is_none());
    assert_eq!(
        notes::left_message(&"SHOP-40.2".parse().unwrap()),
        "Note left for SHOP-40.2. Its agent reads it before its next commit or submit."
    );

    // Unread, with the card's flag.
    let child = w.board.ticket("SHOP-40.2").await.unwrap();
    let flag = child.flags.unread_notes.expect("the unread-note flag");
    assert_eq!((flag.count, flag.from.as_str()), (1, "Wishlist lead"));
    assert_eq!(flag.latest_text, NOTE);
    // "Note from Wishlist lead" on the child and the epic, by the lead.
    for key in ["SHOP-40.2", "SHOP-40"] {
        let event = w
            .board
            .history(key)
            .await
            .unwrap()
            .into_iter()
            .find(|e| e.kind == EventKind::LeadNote)
            .unwrap_or_else(|| panic!("no note in {key}'s History"));
        assert!(matches!(event.actor, Actor::ProjectAgent(_)), "{key}");
        assert_eq!(event.payload["lead"], "Wishlist lead");
        assert_eq!(event.payload["child"], "SHOP-40.2");
        assert_eq!(event.payload["text"], NOTE);
    }
    let listed = notes::list_for_epic(&w.board, "SHOP-40").await.unwrap();
    assert_eq!(listed.len(), 1);
    // The Messages log (T-0290): on the epic and on that child, not on a sibling.
    for (key, count) in [("SHOP-40", 1), ("SHOP-40.2", 1), ("SHOP-40.3", 0)] {
        let log = otaq_core::epic_lead::log::of(&w.board, key).await.unwrap();
        assert_eq!(log.notes.len(), count, "{key}");
    }
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn never_started_finished_and_non_lead_notes_are_refused() {
    let w = world().await;
    let lead = lead_shell(&w).await;

    let never = send(
        &w,
        &lead,
        "lead.note",
        json!({"key": "SHOP-40.3", "text": "x"}),
    )
    .await;
    assert_eq!(
        refused(&never),
        "SHOP-40.3 hasn't started; edit its description instead (`otaq update`)"
    );
    let merged = send(
        &w,
        &lead,
        "lead.note",
        json!({"key": "SHOP-40.1", "text": "x"}),
    )
    .await;
    assert_eq!(
        refused(&merged),
        "SHOP-40.1 isn't running (Merged); a note reaches only a running child"
    );
    let empty = send(
        &w,
        &lead,
        "lead.note",
        json!({"key": "SHOP-40.2", "text": " "}),
    )
    .await;
    assert_eq!(refused(&empty), notes::MISSING_TEXT);
    // Outside its epic: the lead's one refusal.
    let other = send(
        &w,
        &lead,
        "lead.note",
        json!({"key": "SHOP-15", "text": "x"}),
    )
    .await;
    assert_eq!(
        refused(&other),
        "Wishlist lead can only change SHOP-40's children that haven't started"
    );

    // A ticket's agent and the user aren't leads.
    let child = w.board.ticket("SHOP-40.2").await.unwrap();
    let agent = w.agent_shell(&child).await;
    let by_agent = send(
        &w,
        &agent,
        "lead.note",
        json!({"key": "SHOP-40.2", "text": "x"}),
    )
    .await;
    assert_eq!(refused(&by_agent), ONLY_A_LEAD_NOTES);
    let by_user = send_as_user(&w, "lead.note", json!({"key": "SHOP-40.2", "text": "x"})).await;
    assert_eq!(refused(&by_user), ONLY_A_LEAD_NOTES);
    assert_eq!(unread_count(&w, "SHOP-40.2").await, 0);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn notes_and_the_agents_prime_mark_read_and_submit_waits_for_them() {
    let w = world().await;
    let lead = lead_shell(&w).await;
    let child = w.board.ticket("SHOP-40.2").await.unwrap();
    let agent = w.agent_shell(&child).await;
    ok(&send(
        &w,
        &lead,
        "lead.note",
        json!({"key": "SHOP-40.2", "text": NOTE}),
    )
    .await);

    // Submit refused while unread, with the exact text; nothing moved.
    let submit = json!({"key": "SHOP-40.2", "summary": "Wishlist button done"});
    let answer = send(&w, &agent, "ticket.review_submit", submit.clone()).await;
    assert_eq!(
        refused(&answer),
        "Submit refused: 1 unread note from Wishlist lead. Run `otaq notes`, then submit again."
    );
    assert_eq!(w.status("SHOP-40.2").await, Status::Working);

    // The user's prime shows the note but leaves it unread.
    let primed = ok(&send_as_user(&w, "ticket.prime", json!({"key": "SHOP-40.2"})).await).clone();
    assert_eq!(primed["prime"]["unread_notes"][0]["text"], NOTE);
    assert_eq!(unread_count(&w, "SHOP-40.2").await, 1);

    // `otaq notes` from the child's agent: printed and marked read; the flag clears.
    let read = send(&w, &agent, "ticket.notes", json!({"key": "SHOP-40.2"})).await;
    let read = ok(&read)["lead_notes"].as_array().unwrap().clone();
    assert_eq!(read.len(), 1);
    assert_eq!(read[0]["text"], NOTE);
    assert_eq!(unread_count(&w, "SHOP-40.2").await, 0);
    let stored = notes::get(&w.board, read[0]["id"].as_str().unwrap())
        .await
        .unwrap();
    assert_eq!(stored.read_how, Some(ReadHow::Notes));
    assert!(
        w.board
            .ticket("SHOP-40.2")
            .await
            .unwrap()
            .flags
            .unread_notes
            .is_none()
    );
    let again = send(&w, &agent, "ticket.notes", json!({"key": "SHOP-40.2"})).await;
    assert!(ok(&again)["lead_notes"].as_array().unwrap().is_empty());
    assert_eq!(notes::unread_message(&[]), "No unread notes.");

    // A second note, read by the agent's own prime: listed at the top and marked read.
    ok(&send(
        &w,
        &lead,
        "lead.note",
        json!({"key": "SHOP-40.2", "text": "Second."}),
    )
    .await);
    let primed = send(&w, &agent, "ticket.prime", json!({"key": "SHOP-40.2"})).await;
    let context: otaq_core::prime::PrimeContext =
        serde_json::from_value(ok(&primed)["prime"].clone()).unwrap();
    assert_eq!(context.unread_notes.len(), 1);
    let rendered = otaq_core::prime::render(&context);
    let top = rendered.split("## What and why").next().unwrap();
    assert!(
        top.contains("1 unread note from Wishlist lead (") && top.contains("): Second."),
        "{rendered}"
    );
    assert_eq!(unread_count(&w, "SHOP-40.2").await, 0);
    let stored = notes::get(&w.board, &context.unread_notes[0].id)
        .await
        .unwrap();
    assert_eq!(stored.read_how, Some(ReadHow::Prime));

    // Accepted once read.
    let answer = send(&w, &agent, "ticket.review_submit", submit).await;
    ok(&answer);
    assert_ne!(w.status("SHOP-40.2").await, Status::Working);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_users_submit_for_review_isnt_refused() {
    let w = world().await;
    let lead = lead_shell(&w).await;
    ok(&send(
        &w,
        &lead,
        "lead.note",
        json!({"key": "SHOP-40.2", "text": NOTE}),
    )
    .await);
    let child = w.board.ticket("SHOP-40.2").await.unwrap();
    w.force_status(&child, Status::Idle).await;

    let answer = send_as_user(&w, "ticket.submit_by_user", json!({"key": "SHOP-40.2"})).await;
    ok(&answer);
    assert_eq!(w.status("SHOP-40.2").await, Status::ReadyForReview);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn at_a_stop_the_note_is_typed_in_and_the_child_stays_at_work() {
    let w = world().await;
    let lead = lead_shell(&w).await;
    let child = w.board.ticket("SHOP-40.2").await.unwrap();
    let agent = w.agent_shell(&child).await;
    ok(&send(
        &w,
        &lead,
        "lead.note",
        json!({"key": "SHOP-40.2", "text": NOTE}),
    )
    .await);
    let me = Caller::Agent {
        ticket: child.key.clone(),
        ticket_id: child.id.clone(),
        session: agent.session.clone(),
    };

    // The turn ends with nothing pending: the note is typed in and read, and the card isn't Idle.
    w.signals
        .hook(
            &me,
            Tool::Claude,
            "Stop",
            &json!({"background_tasks": [], "session_crons": []}),
        )
        .await
        .unwrap();
    assert_eq!(w.status("SHOP-40.2").await, Status::Working);
    assert_eq!(unread_count(&w, "SHOP-40.2").await, 0);
    let stored = notes::list_for_epic(&w.board, "SHOP-40").await.unwrap();
    assert_eq!(stored[0].read_how, Some(ReadHow::Typed));
    assert!(
        w.deliveries
            .pending(&agent.session)
            .await
            .unwrap()
            .is_empty()
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let out =
                String::from_utf8_lossy(&w.sessions.attach(&agent.session).unwrap().scrollback)
                    .into_owned();
            if out.contains(&format!("Note from Wishlist lead: {NOTE}")) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the note was typed");
    // The agent isn't idle between turns any more: the typed note started its next turn.
    assert!(!w.signals.is_idle_between_turns(&agent.session));

    // The next Stop with nothing unread is the child's first stop: it goes to the live lead (LEAD-07, T-0285).
    w.signals
        .hook(
            &me,
            Tool::Claude,
            "Stop",
            &json!({"background_tasks": [], "session_crons": []}),
        )
        .await
        .unwrap();
    assert_eq!(w.status("SHOP-40.2").await, Status::Working);

    // A note to it answers that stop and is typed in.
    ok(&send(
        &w,
        &lead,
        "lead.note",
        json!({"key": "SHOP-40.2", "text": "Third."}),
    )
    .await);
    assert_eq!(unread_count(&w, "SHOP-40.2").await, 0);

    // Its next Stop is an ordinary Idle, and a note to a child that is idle already is typed in at once.
    w.signals
        .hook(
            &me,
            Tool::Claude,
            "Stop",
            &json!({"background_tasks": [], "session_crons": []}),
        )
        .await
        .unwrap();
    assert_eq!(w.status("SHOP-40.2").await, Status::Idle);
    ok(&send(
        &w,
        &lead,
        "lead.note",
        json!({"key": "SHOP-40.2", "text": "Fourth."}),
    )
    .await);
    assert_eq!(unread_count(&w, "SHOP-40.2").await, 0);
    w.stop().await;
}
