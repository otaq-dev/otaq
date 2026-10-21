//! Ticket comments and reports (T-0442; CMT-01…CMT-04, CMT-06, CMT-14; d0109, d0060): the 2,000-character limit, N
//! per ticket never reused, the three History events, reports copied by streaming with their size and lines (20 MB
//! accepted, one byte more refused with nothing left behind), kept on a comment's delete and removed with the ticket;
//! and who may write, through the socket from inside sessions (an ordinary `sh`, never an agent tool).

use std::time::Duration;

use otaq_core::board::Refusal;
use otaq_core::board::reports::{REPORT_LIMIT_BYTES, ReportInfo, ReportSource};
use otaq_core::board::ticket_comments::{COMMENT_LIMIT_CHARS, CommentAuthor, TicketComment};
use otaq_core::board::tickets::{EventKind, HistoryEvent, NewTicketInput};
use otaq_core::caller::Caller;
use otaq_core::client::{Client, ClientConfig};
use otaq_core::domain::{Actor, Kind};
use otaq_core::protocol::{Event, Reply, Request, TicketCommentPostParams};
use otaq_core::service::testing::{AgentShell, TestService};
use otaq_core::session::SessionKind;
use serde_json::{Value, json};

fn me() -> Actor {
    Actor::User("nevzat".into())
}

fn you() -> CommentAuthor {
    CommentAuthor::User {
        name: "nevzat".into(),
    }
}

fn rust_developer() -> CommentAuthor {
    CommentAuthor::Agent {
        id: Some("rust-dev".into()),
        name: Some("Rust developer".into()),
        tool: None,
        stage: Some("dev".into()),
        round: None,
    }
}

async fn post(w: &TestService, key: &str, body: &str) -> TicketComment {
    w.board
        .post_ticket_comment(key, &you(), body, None, &me())
        .await
        .unwrap()
}

/// The ticket's comment events, oldest first.
async fn comment_events(w: &TestService, key: &str) -> Vec<HistoryEvent> {
    let mut events: Vec<HistoryEvent> = w
        .board
        .history(key)
        .await
        .unwrap()
        .into_iter()
        .filter(|e| {
            matches!(
                e.kind,
                EventKind::CommentPosted | EventKind::CommentEdited | EventKind::CommentDeleted
            )
        })
        .collect();
    events.reverse();
    events
}

/// Every file under the data folder's `reports/`.
fn report_files(w: &TestService) -> Vec<std::path::PathBuf> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&w.data.reports_dir(), &mut out);
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn the_limit_counts_characters_and_numbers_are_never_reused() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    w.create("SHOP", "Export orders as CSV").await;

    // 2,000 characters of two-byte text are accepted; 2,001 refused with CMT-02's text and the real count.
    let at_limit = "ş".repeat(COMMENT_LIMIT_CHARS);
    assert_eq!(post(&w, "SHOP-1", &at_limit).await.n, 1);
    let refused = w
        .board
        .post_ticket_comment("SHOP-1", &you(), &"🦀".repeat(2_001), None, &me())
        .await
        .unwrap_err();
    assert_eq!(refused.refusal(), Some(Refusal::TooLong));
    assert_eq!(
        refused.to_string(),
        "Comment is 2,001 characters; the limit is 2,000. Put the full text in a report with `--report-file` and \
         keep the comment to what the next agent needs."
    );
    for blank in ["", "  \n "] {
        let refused = w
            .board
            .post_ticket_comment("SHOP-1", &you(), blank, None, &me())
            .await
            .unwrap_err();
        assert_eq!(refused.refusal(), Some(Refusal::EmptyField));
        assert_eq!(refused.to_string(), "Write the comment first");
    }

    // Post 2, delete 2: the next is 3, and the list is 1 and 3, oldest first.
    let second = w
        .board
        .post_ticket_comment(
            "SHOP-1",
            &rust_developer(),
            "Reused the CSV writer.",
            None,
            &Actor::Agent("SHOP-1".parse().unwrap()),
        )
        .await
        .unwrap();
    assert_eq!(second.n, 2);
    assert_eq!(second.stage_name.as_deref(), Some("Dev"));
    w.board
        .delete_ticket_comment("SHOP-1", 2, &me())
        .await
        .unwrap();
    assert_eq!(post(&w, "SHOP-1", "Also keep the column order.").await.n, 3);
    let listed = w.board.ticket_comments("SHOP-1").await.unwrap();
    assert_eq!(
        listed.comments.iter().map(|c| c.n).collect::<Vec<_>>(),
        [1, 3]
    );
    // A deleted comment can't be edited or deleted again.
    assert!(
        w.board
            .edit_ticket_comment("SHOP-1", 2, "again", &me())
            .await
            .is_err()
    );

    // Editing keeps the limit and marks the comment edited.
    let refused = w
        .board
        .edit_ticket_comment("SHOP-1", 3, &"x".repeat(48_210), &me())
        .await
        .unwrap_err();
    assert_eq!(
        refused.to_string(),
        "Comment is 48,210 characters; the limit is 2,000. Put the full text in a report with `--report-file` and \
         keep the comment to what the next agent needs."
    );
    let edited = w
        .board
        .edit_ticket_comment("SHOP-1", 3, "Keep the column order of the table.", &me())
        .await
        .unwrap();
    assert!(edited.edited_at.is_some());
    assert_eq!(edited.body, "Keep the column order of the table.");

    // The three events, with the actor and their payloads.
    let events = comment_events(&w, "SHOP-1").await;
    let kinds: Vec<EventKind> = events.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        [
            EventKind::CommentPosted,
            EventKind::CommentPosted,
            EventKind::CommentDeleted,
            EventKind::CommentPosted,
            EventKind::CommentEdited,
        ]
    );
    assert_eq!(events[1].actor.to_string(), "agent:SHOP-1");
    assert_eq!(events[1].payload["n"], 2);
    assert_eq!(events[1].payload["excerpt"], "Reused the CSV writer.");
    assert_eq!(events[1].payload["report"], Value::Null);
    assert_eq!(events[1].payload["author"]["name"], "Rust developer");
    assert_eq!(
        events[0].payload["excerpt"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        121,
        "a long comment's excerpt is its start"
    );
    assert_eq!(events[2].actor, me());
    assert_eq!(
        (&events[2].payload["n"], &events[2].payload["body"]),
        (&json!(2), &json!("Reused the CSV writer."))
    );
    assert_eq!(
        (
            &events[4].payload["n"],
            &events[4].payload["before"],
            &events[4].payload["after"]
        ),
        (
            &json!(3),
            &json!("Also keep the column order."),
            &json!("Keep the column order of the table.")
        )
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn reports_are_copied_sized_and_counted_and_live_as_long_as_the_ticket() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    w.create("SHOP", "Export orders as CSV").await;
    let resources = tempfile::TempDir::new().unwrap();
    let run = resources.path().join("test-run.txt");
    let text = "running 3 tests\ntest export::limit ... FAILED\nfailures: 1\n".repeat(1_000);
    std::fs::write(&run, &text).unwrap();

    let fail = w
        .board
        .post_ticket_comment(
            "SHOP-1",
            &rust_developer(),
            "Fail: the 10,000 row limit isn't enforced.",
            Some(ReportSource::Path(run.clone())),
            &me(),
        )
        .await
        .unwrap();
    assert_eq!(
        fail.report,
        Some(ReportInfo {
            bytes: text.len() as u64,
            lines: 3_000
        })
    );
    // Copied, never moved.
    assert!(run.exists());
    let report = w.board.ticket_report("SHOP-1", fail.n).await.unwrap();
    assert!(
        report.path.starts_with(
            &w.data
                .reports_dir()
                .join("shop-web")
                .join("SHOP-1")
                .display()
                .to_string()
        ),
        "{}",
        report.path
    );
    assert!(report.path.ends_with(".txt"));
    assert_eq!(std::fs::read_to_string(&report.path).unwrap(), text);
    assert!(!report.deleted);
    let events = comment_events(&w, "SHOP-1").await;
    assert_eq!(
        events[0].payload["report"],
        json!({"bytes": text.len(), "lines": 3_000})
    );

    // Pasted text from the app goes to the same store.
    let pasted = w
        .board
        .post_ticket_comment(
            "SHOP-1",
            &you(),
            "The log from my run.",
            Some(ReportSource::Text("one\ntwo".into())),
            &me(),
        )
        .await
        .unwrap();
    assert_eq!(pasted.report, Some(ReportInfo { bytes: 7, lines: 2 }));

    // Exactly 20 MB is accepted; one byte more refused, with nothing posted and no file left.
    let at_limit = resources.path().join("at-limit.txt");
    std::fs::File::create(&at_limit)
        .unwrap()
        .set_len(REPORT_LIMIT_BYTES)
        .unwrap();
    let big = w
        .board
        .post_ticket_comment(
            "SHOP-1",
            &you(),
            "The whole run.",
            Some(ReportSource::Path(at_limit.clone())),
            &me(),
        )
        .await
        .unwrap();
    assert_eq!(big.report.unwrap().bytes, REPORT_LIMIT_BYTES);
    let files_before = report_files(&w);
    assert_eq!(files_before.len(), 3);
    std::fs::OpenOptions::new()
        .append(true)
        .open(&at_limit)
        .unwrap()
        .set_len(REPORT_LIMIT_BYTES + 1)
        .unwrap();
    let refused = w
        .board
        .post_ticket_comment(
            "SHOP-1",
            &you(),
            "The whole run again.",
            Some(ReportSource::Path(at_limit.clone())),
            &me(),
        )
        .await
        .unwrap_err();
    assert_eq!(refused.refusal(), Some(Refusal::BadReport));
    assert_eq!(
        refused.to_string(),
        "The report is more than 20 MB; the limit is 20 MB. Keep the part that matters, for example the failing \
         tests."
    );
    std::fs::OpenOptions::new()
        .append(true)
        .open(&at_limit)
        .unwrap()
        .set_len(212 * 1024 * 1024)
        .unwrap();
    let refused = w
        .board
        .post_ticket_comment(
            "SHOP-1",
            &you(),
            "The runaway log.",
            Some(ReportSource::Path(at_limit.clone())),
            &me(),
        )
        .await
        .unwrap_err();
    assert_eq!(
        refused.to_string(),
        "The report is 212 MB; the limit is 20 MB. Keep the part that matters, for example the failing tests."
    );
    // A missing file and a relative path: refused before anything is posted.
    let missing = resources.path().join("nope.txt");
    let refused = w
        .board
        .post_ticket_comment(
            "SHOP-1",
            &you(),
            "See the log.",
            Some(ReportSource::Path(missing.clone())),
            &me(),
        )
        .await
        .unwrap_err();
    assert_eq!(
        refused.to_string(),
        format!(
            "Can't read the report {}: it doesn't exist",
            missing.display()
        )
    );
    assert!(
        w.board
            .post_ticket_comment(
                "SHOP-1",
                &you(),
                "See the log.",
                Some(ReportSource::Path("test-run.txt".into())),
                &me(),
            )
            .await
            .is_err()
    );
    assert_eq!(report_files(&w), files_before, "no partial file is left");
    assert_eq!(
        w.board
            .ticket_comments("SHOP-1")
            .await
            .unwrap()
            .comments
            .len(),
        3,
        "nothing was posted"
    );

    // A deleted comment's report stays (History opens it).
    w.board
        .delete_ticket_comment("SHOP-1", fail.n, &me())
        .await
        .unwrap();
    let kept = w.board.ticket_report("SHOP-1", fail.n).await.unwrap();
    assert!(kept.deleted);
    assert_eq!(std::fs::read_to_string(&kept.path).unwrap(), text);
    assert_eq!(report_files(&w).len(), 3);

    // Deleting the never-started ticket removes its reports and its folder (d0060).
    w.board.delete_ticket("SHOP-1").await.unwrap();
    assert!(report_files(&w).is_empty());
    assert!(
        !w.data
            .reports_dir()
            .join("shop-web")
            .join("SHOP-1")
            .exists()
    );
    w.stop().await;
}

/// Sends one raw request from inside a session and returns the answer (no `'` in it: it goes through `sh`).
async fn send(w: &TestService, shell: &AgentShell, method: &str, params: Value) -> Value {
    let request =
        json!({"protocol_version": 1, "id": 1, "request": {"method": method, "params": params}});
    let ran = shell
        .send_raw(&w.data.socket_path(), &request.to_string())
        .await;
    serde_json::from_str(ran.stdout.lines().next().unwrap_or_default())
        .unwrap_or_else(|e| panic!("{e}: {ran:?}"))
}

fn refusal(answer: &Value) -> &str {
    answer["error"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("not refused: {answer}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn agents_write_on_their_own_ticket_and_edit_only_their_own() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Export orders as CSV").await;
    w.create("SHOP", "Order list paging").await;
    post(&w, "SHOP-1", "Also keep the column order.").await;
    let dev = w.agent_shell(&shop1).await;
    w.run_as_agent(&dev, "rust-dev", "Rust developer").await;

    // Its own ticket: posted as the agent its session runs, with its tool.
    let posted = send(
        &w,
        &dev,
        "ticket.comment_post",
        json!({"key": "SHOP-1", "body": "Reused the CSV writer in export.rs."}),
    )
    .await;
    let comment = &posted["reply"]["ticket_comment"];
    assert_eq!(comment["n"], 2, "{posted}");
    assert_eq!(
        comment["author"],
        json!({"kind": "agent", "id": "rust-dev", "name": "Rust developer", "tool": "claude"})
    );
    // Another ticket: refused.
    let other = send(
        &w,
        &dev,
        "ticket.comment_post",
        json!({"key": "SHOP-2", "body": "Hello"}),
    )
    .await;
    assert_eq!(refusal(&other), "SHOP-1's agent can't comment on SHOP-2");
    // Its own comment: edited and deleted; the user's: refused.
    let edited = send(
        &w,
        &dev,
        "ticket.comment_edit",
        json!({"key": "SHOP-1", "n": 2, "body": "Reused the CSV writer from SHOP-12."}),
    )
    .await;
    assert_eq!(
        edited["reply"]["ticket_comment"]["body"],
        "Reused the CSV writer from SHOP-12."
    );
    for (method, params) in [
        (
            "ticket.comment_edit",
            json!({"key": "SHOP-1", "n": 1, "body": "x"}),
        ),
        ("ticket.comment_delete", json!({"key": "SHOP-1", "n": 1})),
    ] {
        let answer = send(&w, &dev, method, params).await;
        assert_eq!(
            refusal(&answer),
            "SHOP-1's agent can only edit or delete its own comments"
        );
    }
    // Another agent on the same ticket (a later stage) can't edit Rust developer's comment.
    // (The same session stands in for the later stage's: one agent session per ticket at a time.)
    let reviewer = &dev;
    w.run_as_agent(reviewer, "reviewer", "Code reviewer").await;
    let answer = send(
        &w,
        reviewer,
        "ticket.comment_edit",
        json!({"key": "SHOP-1", "n": 2, "body": "x"}),
    )
    .await;
    assert_eq!(
        refusal(&answer),
        "SHOP-1's agent can only edit or delete its own comments"
    );
    // Anyone reads them.
    let listed = send(&w, reviewer, "ticket.comments", json!({"key": "SHOP-1"})).await;
    assert_eq!(
        listed["reply"]["ticket_comments"]["comments"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let deleted = send(
        &w,
        reviewer,
        "ticket.comment_delete",
        json!({"key": "SHOP-1", "n": 2}),
    )
    .await;
    assert!(deleted.get("error").is_some());
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_checker_posts_only_with_its_verdict() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Export orders as CSV").await;
    let checker = w.shell(&shop1, SessionKind::Validator).await;
    let answer = send(
        &w,
        &checker,
        "ticket.comment_post",
        json!({"key": "SHOP-1", "body": "Fail: the limit is not enforced."}),
    )
    .await;
    assert_eq!(refusal(&answer), "SHOP-1's check can only give its verdict");
    // It reads them.
    let listed = send(&w, &checker, "ticket.comments", json!({"key": "SHOP-1"})).await;
    assert!(listed.get("reply").is_some(), "{listed}");
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_epic_lead_owns_its_epics_lead_comments() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let epic = w
        .board
        .create_ticket(
            &NewTicketInput {
                project: "SHOP".into(),
                title: "Wishlist".into(),
                kind: Kind::Epic,
                ..Default::default()
            },
            &me(),
        )
        .await
        .unwrap();
    let lead = |epic_id: &str| Caller::EpicLead {
        project: "SHOP".into(),
        agent: "lead-1".into(),
        epic: epic.key.clone(),
        epic_id: epic_id.into(),
        session: "s".into(),
    };
    let author = w
        .board
        .caller_author(&lead(&epic.id), &Actor::ProjectAgent("lead-1".into()))
        .await
        .unwrap();
    assert_eq!(
        author,
        CommentAuthor::Lead {
            epic_id: epic.id.clone(),
            name: "Wishlist lead".into(),
            tool: None,
        }
    );
    w.board
        .post_ticket_comment(
            "SHOP-1",
            &author,
            "Keep SHOP-1.2 on the same API.",
            None,
            &me(),
        )
        .await
        .unwrap();
    post(&w, "SHOP-1", "Mine.").await;
    assert!(
        w.board
            .is_own_comment(&lead(&epic.id), "SHOP-1", 1)
            .await
            .unwrap()
    );
    assert!(
        !w.board
            .is_own_comment(&lead(&epic.id), "SHOP-1", 2)
            .await
            .unwrap()
    );
    assert!(
        !w.board
            .is_own_comment(&lead("another-epic"), "SHOP-1", 1)
            .await
            .unwrap()
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn each_change_tells_clients_the_ticket_changed() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    w.create("SHOP", "Export orders as CSV").await;
    let app = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let mut events = w.events.subscribe();
    let requests = [
        Request::TicketCommentPost(TicketCommentPostParams {
            key: "SHOP-1".into(),
            body: "Also keep the column order.".into(),
            report: None,
        }),
        serde_json::from_value(json!({"method": "ticket.comment_edit",
            "params": {"key": "SHOP-1", "n": 1, "body": "Keep the order."}}))
        .unwrap(),
        serde_json::from_value(
            json!({"method": "ticket.comment_delete", "params": {"key": "SHOP-1", "n": 1}}),
        )
        .unwrap(),
    ];
    for request in requests {
        while events.try_recv().is_ok() {}
        let reply = app.request(request).await.unwrap();
        assert!(matches!(reply, Reply::TicketComment(_)), "{reply:?}");
        let changed = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(Event::TicketChanged { key, .. }) = events.recv().await
                    && key.to_string() == "SHOP-1"
                {
                    return;
                }
            }
        })
        .await;
        assert!(changed.is_ok(), "ticket_changed after each change");
    }
    w.stop().await;
}
