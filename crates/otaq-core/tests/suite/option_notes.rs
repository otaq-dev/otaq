//! A note per answer option (T-0374; ATT-17; d0061, d0083): `--note` rides with its `--option` through all three
//! routes (a ticket's agent, a Project agent, a lead's pass-on), is stored whole in `option_notes_json`, and reaches
//! `prime --epic`. Requests are sent over the socket from inside the sessions (an ordinary `sh` as each agent,
//! running `nc`), so each is judged as its caller. No test starts `claude`.

use otaq_core::agent_tools::LaunchContext;
use otaq_core::agent_tools::Tool;
use otaq_core::agent_tools::claude::ClaudeLaunch;
use otaq_core::board::questions::Choice;
use otaq_core::board::tickets::{NewTicketInput, Ticket};
use otaq_core::caller::Caller;
use otaq_core::domain::{Actor, Status};
use otaq_core::epic_lead::prime::{context, render};
use otaq_core::epic_lead::questions::{self, LeadQuestion, LeadQuestionState};
use otaq_core::git::testing::TempRepos;
use otaq_core::project_agents::lead::Lead;
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::{AgentShell, TestService, path_without_build_folders};
use serde_json::{Value, json};

const QUESTION: &str = "Which export format?";
const MD: &str = "Opens in any editor; one file per note, no attachments.";
const ZIP: &str = "One download with everything, but needs a ZIP library and an import later.";
const REASON: &str = "docs/storage.md stores notes as plain text files.";

/// The sample board (SHOP-40 Wishlist with 40.2 Working) plus 40.4 and 40.5 Working; `claude` is an interactive
/// `sh` in the service's data folder.
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
            bin_dir: "/nonexistent".into(),
            plugin_dir: "/nonexistent/plugin".into(),
            path: path_without_build_folders().into(),
            claude: ClaudeLaunch {
                program: vec!["sh".into(), "-c".into(), script, "sh".into()],
                config_dir: None,
            },
        }),
        ..config
    })
    .await;
    w.shop_sample().await;
    for title in ["Wishlist share link", "Wishlist export"] {
        let child = w
            .board
            .create_ticket(
                &NewTicketInput {
                    project: "SHOP".into(),
                    title: title.into(),
                    epic: Some("SHOP-40".into()),
                    ..Default::default()
                },
                &Actor::User("me".into()),
            )
            .await
            .unwrap();
        w.force_status(&child, Status::Working).await;
    }
    w
}

/// One request from inside the session.
async fn send(w: &TestService, shell: &AgentShell, method: &str, params: Value) -> Value {
    let request =
        json!({"protocol_version": 1, "id": 1, "request": {"method": method, "params": params}});
    let ran = shell
        .send_raw(&w.data.socket_path(), &request.to_string())
        .await;
    serde_json::from_str(ran.stdout.lines().next().unwrap_or_default())
        .unwrap_or_else(|e| panic!("{e}: {ran:?}"))
}

fn ok(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "{answer}");
    &answer["reply"]
}

fn refused(answer: &Value) -> String {
    assert_eq!(answer["error"]["kind"], "refused", "{answer}");
    answer["error"]["message"].as_str().unwrap().to_owned()
}

fn notes(entries: &[Option<&str>]) -> Vec<Option<String>> {
    entries.iter().map(|n| n.map(str::to_owned)).collect()
}

async fn ticket(w: &TestService, key: &str) -> Ticket {
    w.board.ticket(key).await.unwrap()
}

struct Child {
    ticket: Ticket,
    shell: AgentShell,
}

async fn running_child(w: &TestService, key: &str) -> Child {
    let ticket = ticket(w, key).await;
    let shell = w.agent_shell(&ticket).await;
    Child { ticket, shell }
}

impl Child {
    fn caller(&self) -> Caller {
        Caller::Agent {
            ticket: self.ticket.key.clone(),
            ticket_id: self.ticket.id.clone(),
            session: self.shell.session.clone(),
        }
    }

    /// `otaq ask human --option "Markdown files" --note … --option "One ZIP" --note … --recommended 1 --stdin`,
    /// then its turn ends.
    async fn asks_with_notes(&self, w: &TestService) {
        let asked = send(
            w,
            &self.shell,
            "ticket.ask_human",
            json!({"key": self.ticket.key, "question": QUESTION, "options": ["Markdown files", "One ZIP"],
                   "notes": [MD, ZIP], "recommended": 1}),
        )
        .await;
        ok(&asked);
        let signals = w
            .signals
            .hook(
                &self.caller(),
                Tool::Claude,
                "Stop",
                &json!({ "last_assistant_message": "Waiting for the answer." }),
            )
            .await
            .unwrap();
        assert!(signals.iter().all(otaq_core::signals::Signal::ends_turn));
        w.deliveries
            .deliver_if_ready(&self.shell.session)
            .await
            .unwrap();
    }
}

fn session(lead: &Lead) -> String {
    lead.agent.session.as_ref().expect("a session").id.clone()
}

fn lead_caller(lead: &Lead) -> Caller {
    Caller::EpicLead {
        project: lead.agent.project.clone(),
        agent: lead.agent.id.clone(),
        epic: lead.epic.clone(),
        epic_id: lead.epic_id.clone(),
        session: session(lead),
    }
}

async fn lead_hook(w: &TestService, lead: &Lead, event: &str, payload: Value) {
    let signals = w
        .signals
        .hook(&lead_caller(lead), Tool::Claude, event, &payload)
        .await
        .unwrap();
    for signal in &signals {
        w.project_agents.hook(&lead.agent.id, signal).await.unwrap();
        if signal.ends_turn() {
            w.deliveries.deliver_if_ready(&session(lead)).await.unwrap();
        }
    }
}

async fn lead_prompt(w: &TestService, lead: &Lead, text: &str) {
    lead_hook(w, lead, "UserPromptSubmit", json!({ "prompt": text })).await;
}

async fn lead_stop(w: &TestService, lead: &Lead) {
    lead_hook(
        w,
        lead,
        "Stop",
        json!({ "last_assistant_message": "Done." }),
    )
    .await;
}

/// The lead's turn that the wake-up of `child`'s question starts.
async fn lead_takes_the_wake_up(w: &TestService, lead: &Lead) {
    lead_stop(w, lead).await;
    lead_prompt(w, lead, "SHOP-40.x asks").await;
}

/// The question of `child` waiting for the epic's lead.
async fn waiting_for_lead(w: &TestService, child: &Child) -> LeadQuestion {
    let epic = ticket(w, "SHOP-40").await;
    questions::waiting(&w.board, &epic.id)
        .await
        .unwrap()
        .into_iter()
        .find(|q| q.child == child.ticket.key)
        .expect("waiting for the lead")
}

/// A ticket's agent: the notes pair with the options, null for one without; a 600-character note is stored whole;
/// notes on a free-text question or not one per option are refused. The delivered answer doesn't name the note.
#[tokio::test(flavor = "multi_thread")]
async fn a_ticket_agents_notes_are_stored_whole_with_its_options() {
    let w = world().await;
    let asker = running_child(&w, "SHOP-40.2").await;
    let long = "n".repeat(600);
    let free_text = send(
        &w,
        &asker.shell,
        "ticket.ask_human",
        json!({"key": "SHOP-40.2", "question": QUESTION, "notes": [MD]}),
    )
    .await;
    assert_eq!(
        refused(&free_text),
        "A free-text question takes no --note: a --note describes the --option before it"
    );
    let uneven = send(
        &w,
        &asker.shell,
        "ticket.ask_human",
        json!({"key": "SHOP-40.2", "question": QUESTION, "options": ["A", "B"], "notes": [MD]}),
    )
    .await;
    assert_eq!(
        refused(&uneven),
        "Give the notes one per option, in the options' order"
    );

    let asked = send(
        &w,
        &asker.shell,
        "ticket.ask_human",
        json!({"key": "SHOP-40.2", "question": QUESTION, "options": ["Markdown files", "One ZIP", "PDF"],
               "notes": [format!("  {MD}\n"), null, long], "recommended": 1}),
    )
    .await;
    let reply = ok(&asked);
    assert_eq!(
        reply["ticket"]["question"]["notes"],
        json!([MD, null, long])
    );
    let question = ticket(&w, "SHOP-40.2").await.question.expect("asks you");
    assert_eq!(question.notes, notes(&[Some(MD), None, Some(&long)]));
    let stored: Option<String> =
        sqlx::query_scalar("SELECT option_notes_json FROM human_questions WHERE id = ?")
            .bind(&question.id)
            .fetch_one(w.board.store().pool())
            .await
            .unwrap();
    assert_eq!(stored, Some(json!([MD, null, long]).to_string()));
    assert_eq!(
        otaq_core::board::questions::format_answer(&question, &Choice::Option(1)),
        "Answer to your question: Markdown files (option 1)."
    );
    w.stop().await;
}

/// Without notes nothing is stored and nothing shows; old rows (NULL) read the same.
#[tokio::test(flavor = "multi_thread")]
async fn a_question_without_notes_stores_none() {
    let w = world().await;
    let t = w.create("SHOP", "Order confirmation email").await;
    let q = w
        .board
        .store_question(
            &t.id,
            "Which email provider?",
            &["Postmark".into(), "SES".into()],
            &[None, Some("  ".into())],
            Some(1),
        )
        .await
        .unwrap();
    assert!(q.notes.is_empty());
    let read = w.board.open_question(&t.id).await.unwrap().unwrap();
    assert!(read.notes.is_empty());
    assert!(serde_json::to_value(&read).unwrap().get("notes").is_none());
    let stored: Option<String> =
        sqlx::query_scalar("SELECT option_notes_json FROM human_questions WHERE id = ?")
            .bind(&q.id)
            .fetch_one(w.board.store().pool())
            .await
            .unwrap();
    assert_eq!(stored, None);

    // A round trip with a null entry.
    let t2 = w.create("SHOP", "Export").await;
    w.board
        .store_question(
            &t2.id,
            QUESTION,
            &["Markdown files".into(), "One ZIP".into(), "PDF".into()],
            &notes(&[None, Some(ZIP), None]),
            None,
        )
        .await
        .unwrap();
    let read = w.board.open_question(&t2.id).await.unwrap().unwrap();
    assert_eq!(read.notes, notes(&[None, Some(ZIP), None]));
    w.stop().await;
}

/// A Project agent's `ask human` carries the notes.
#[tokio::test(flavor = "multi_thread")]
async fn a_project_agents_question_carries_the_notes() {
    let w = world().await;
    let agent = w
        .project_agents
        .open("SHOP", "Plan the export")
        .await
        .unwrap();
    let shell = w.shell_in(&agent.session.as_ref().unwrap().id);
    let asked = send(
        &w,
        &shell,
        "agent.ask_human",
        json!({"question": QUESTION, "options": ["Markdown files", "One ZIP"], "notes": [MD, null],
               "recommended": 1}),
    )
    .await;
    assert_eq!(
        ok(&asked)["project_agent"]["question"]["notes"],
        json!([MD, null])
    );
    let question = w
        .project_agents
        .get(&agent.id)
        .await
        .unwrap()
        .question
        .unwrap();
    assert_eq!(question.notes, notes(&[Some(MD), None]));
    w.stop().await;
}

/// A child's question that goes to its lead keeps its notes; `prime --epic` lists them under the options. A pass-on
/// without options keeps the child's options and notes; with its own options it takes its own notes, none if it gave
/// none; a note with no option is refused.
#[tokio::test(flavor = "multi_thread")]
async fn the_lead_sees_the_notes_and_a_pass_on_keeps_or_replaces_them() {
    let w = world().await;
    let lead = w.project_agents.leads().open("SHOP-40").await.unwrap();
    let lead_shell = w.shell_in(&session(&lead));
    lead_prompt(&w, &lead, "Plan it").await;

    // Kept: the child's options and notes go on to the user.
    let button = running_child(&w, "SHOP-40.2").await;
    button.asks_with_notes(&w).await;
    let q = waiting_for_lead(&w, &button).await;
    assert_eq!(q.state, LeadQuestionState::WithLead);
    assert_eq!(q.notes, notes(&[Some(MD), Some(ZIP)]));
    let epic = ticket(&w, "SHOP-40").await;
    let text = render(&context(&w.board, &epic.key.to_string()).await.unwrap());
    assert!(
        text.contains(&format!(
            "from SHOP-40.2: {QUESTION} Options:\n  1. Markdown files (recommended)\n     {MD}\n  2. One ZIP\n     \
             {ZIP}\n"
        )),
        "{text}"
    );
    lead_takes_the_wake_up(&w, &lead).await;
    let stray = send(
        &w,
        &lead_shell,
        "lead.pass_on",
        json!({"id": q.id, "notes": ["x", null], "reason": REASON}),
    )
    .await;
    assert_eq!(
        refused(&stray),
        "--note must follow the --option it describes: without --option the child's options and notes stay"
    );
    let kept = send(
        &w,
        &lead_shell,
        "lead.pass_on",
        json!({"id": q.id, "recommended": 1, "reason": REASON}),
    )
    .await;
    assert_eq!(ok(&kept)["lead_question"]["notes"], json!([MD, ZIP]));
    let asks = ticket(&w, "SHOP-40.2").await.question.expect("asks you");
    assert_eq!(asks.notes, notes(&[Some(MD), Some(ZIP)]));
    assert!(asks.via_lead.is_some());
    lead_stop(&w, &lead).await;
    // The user's question in `prime --epic`: the notes, then the lead's reason.
    let text = render(&context(&w.board, &epic.key.to_string()).await.unwrap());
    assert!(
        text.contains(&format!(
            "from SHOP-40.2: {QUESTION} Options:\n  1. Markdown files (recommended)\n     {MD}\n  2. One ZIP\n     \
             {ZIP}\n  (passed on by Wishlist lead: {REASON})\n"
        )),
        "{text}"
    );

    // Replaced: the lead's own options take its own notes, a null for one without.
    let share = running_child(&w, "SHOP-40.4").await;
    share.asks_with_notes(&w).await;
    let q = waiting_for_lead(&w, &share).await;
    lead_prompt(&w, &lead, "SHOP-40.4 asks").await;
    let replaced = send(
        &w,
        &lead_shell,
        "lead.pass_on",
        json!({"id": q.id, "options": ["Plain text files", "One ZIP"], "notes": [null, "Needs an import."],
               "recommended": 1, "reason": REASON}),
    )
    .await;
    ok(&replaced);
    let asks = ticket(&w, "SHOP-40.4").await.question.expect("asks you");
    assert_eq!(asks.options, ["Plain text files", "One ZIP"]);
    assert_eq!(asks.notes, notes(&[None, Some("Needs an import.")]));
    lead_stop(&w, &lead).await;

    // Own options without notes: none, the child's don't carry over.
    let export = running_child(&w, "SHOP-40.5").await;
    export.asks_with_notes(&w).await;
    let q = waiting_for_lead(&w, &export).await;
    lead_prompt(&w, &lead, "SHOP-40.5 asks").await;
    let bare = send(
        &w,
        &lead_shell,
        "lead.pass_on",
        json!({"id": q.id, "options": ["Plain text files", "One ZIP"], "recommended": 2, "reason": REASON}),
    )
    .await;
    assert!(ok(&bare)["lead_question"].get("notes").is_none(), "{bare}");
    let asks = ticket(&w, "SHOP-40.5").await.question.expect("asks you");
    assert!(asks.notes.is_empty());
    let stored: Option<String> =
        sqlx::query_scalar("SELECT option_notes_json FROM human_questions WHERE id = ?")
            .bind(&asks.id)
            .fetch_one(w.board.store().pool())
            .await
            .unwrap();
    assert_eq!(stored, None);
    w.stop().await;
}
