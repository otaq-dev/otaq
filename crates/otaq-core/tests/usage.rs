//! Token usage (T-0125, USG-01; d0020): counts read read-only from Claude Code's saved conversations for the Claude
//! session IDs Otaq stored. The Claude Code folder is a fixture in a temporary directory with real-shaped JSONL
//! records (copied from a Claude Code 2.1.296 session, content stripped); the real `~/.claude` is never read.

use std::io::Write;
use std::path::{Path, PathBuf};

use otaq_core::board::tickets::Ticket;
use otaq_core::client::{Client, ClientConfig};
use otaq_core::protocol::{Reply, Request, UsageParams};
use otaq_core::recovery::transcript::project_folder;
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;
use otaq_core::store::{new_id, now_ms};
use otaq_core::usage::{TicketUsage, Tokens, local_day};

const S1: &str = "0199a1b2-3c4d-7e5f-8a9b-0c1d2e3f4a5b";
const S2: &str = "0199a1b2-3c4d-7e5f-8a9b-0c1d2e3f4a5c";
const S3: &str = "0199a1b2-3c4d-7e5f-8a9b-0c1d2e3f4a5d";

/// `at` (Unix ms, UTC) as Claude Code writes timestamps: `2026-10-09T12:34:56.789Z`.
fn iso(at: i64) -> String {
    let days = at.div_euclid(86_400_000);
    let ms = at.rem_euclid(86_400_000);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

/// An assistant record as Claude Code 2.1.296 writes it, content stripped.
fn assistant(id: &str, at: i64, input: u64, output: u64, write: u64, read: u64) -> String {
    format!(
        r#"{{"parentUuid":"p","isSidechain":false,"userType":"external","cwd":"/wt","sessionId":"S","version":"2.1.296","gitBranch":"otaq/SHOP-1","message":{{"id":"{id}","type":"message","role":"assistant","model":"claude-opus-5-5","content":[{{"type":"text","text":""}}],"stop_reason":null,"stop_sequence":null,"usage":{{"input_tokens":{input},"cache_creation_input_tokens":{write},"cache_read_input_tokens":{read},"output_tokens":{output},"service_tier":"standard","cache_creation":{{"ephemeral_1h_input_tokens":{write},"ephemeral_5m_input_tokens":0}}}}}},"requestId":"req_{id}","type":"assistant","uuid":"u-{id}","timestamp":"{}"}}"#,
        iso(at)
    )
}

fn bookkeeping(at: i64) -> Vec<String> {
    vec![
        r#"{"type":"permission-mode","permissionMode":"default","sessionId":"S"}"#.to_owned(),
        format!(
            r#"{{"parentUuid":null,"isSidechain":false,"type":"user","message":{{"role":"user","content":"/otaq:work-ticket"}},"uuid":"u0","timestamp":"{}"}}"#,
            iso(at)
        ),
    ]
}

fn lines(lines: &[String]) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

fn append(path: &Path, text: &str) {
    std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
}

/// Every file under `dir` with its size and modification time.
fn listing(dir: &Path) -> Vec<(String, u64, std::time::SystemTime)> {
    let mut all = Vec::new();
    let mut stack = vec![dir.to_owned()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).unwrap().flatten() {
            let meta = entry.metadata().unwrap();
            if meta.is_dir() {
                stack.push(entry.path());
            }
            all.push((
                entry.path().display().to_string(),
                meta.len(),
                meta.modified().unwrap(),
            ));
        }
    }
    all.sort();
    all
}

struct World {
    w: TestService,
    claude: tempfile::TempDir,
}

impl World {
    async fn new() -> Self {
        let claude = tempfile::TempDir::new().unwrap();
        let dir = claude.path().to_owned();
        let w = TestService::start_with(move |mut config: ServiceConfig| {
            config.tools.claude_config_dir = Some(dir);
            // Only the test's explicit reads run.
            config.usage_tick = std::time::Duration::from_secs(3600);
            config
        })
        .await;
        Self { w, claude }
    }

    /// Records an agent session of `ticket` with a Claude session ID.
    async fn session(
        &self,
        ticket: &Ticket,
        claude_id: &str,
        started_at: i64,
        ended_at: Option<i64>,
    ) -> String {
        self.session_of_kind(ticket, "agent", claude_id, started_at, ended_at)
            .await
    }

    /// Records a session of `kind` ('agent', 'validator', …) of `ticket` with a Claude session ID.
    async fn session_of_kind(
        &self,
        ticket: &Ticket,
        kind: &str,
        claude_id: &str,
        started_at: i64,
        ended_at: Option<i64>,
    ) -> String {
        let project = self
            .w
            .board
            .project(ticket.key.project.as_str())
            .await
            .unwrap();
        let id = new_id();
        sqlx::query(
            "INSERT INTO sessions (id, project_id, ticket_id, kind, claude_session_id, state, started_at, ended_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&project.id)
        .bind(&ticket.id)
        .bind(kind)
        .bind(claude_id)
        .bind(if ended_at.is_some() { "ended" } else { "running" })
        .bind(started_at)
        .bind(ended_at)
        .execute(self.w.board.store().pool())
        .await
        .unwrap();
        id
    }

    /// Where Claude Code keeps the conversation of a session that ran in `cwd`.
    fn transcript(&self, cwd: &Path, claude_id: &str) -> PathBuf {
        let folder = self
            .claude
            .path()
            .join("projects")
            .join(project_folder(cwd));
        std::fs::create_dir_all(&folder).unwrap();
        folder.join(format!("{claude_id}.jsonl"))
    }

    async fn rows(&self, session: &str) -> Vec<(String, i64, i64, i64, i64)> {
        sqlx::query_as(
            "SELECT day, tokens_in, tokens_out, cache_write_tokens, cache_read_tokens FROM usage
             WHERE session_id = ? ORDER BY day",
        )
        .bind(session)
        .fetch_all(self.w.board.store().pool())
        .await
        .unwrap()
    }

    /// Records a validation round of `ticket` run by the validator `session`, ended with `outcome`.
    async fn round(&self, ticket: &Ticket, round: i64, session: &str, at: i64, outcome: &str) {
        sqlx::query(
            "INSERT INTO validations (id, ticket_id, round, trigger, session_id, started_at, ended_at, outcome)
             VALUES (?, ?, ?, 'agent', ?, ?, ?, ?)",
        )
        .bind(new_id())
        .bind(&ticket.id)
        .bind(round)
        .bind(session)
        .bind(at)
        .bind(at + 10_000)
        .bind(outcome)
        .execute(self.w.board.store().pool())
        .await
        .unwrap();
    }

    async fn ticket_tokens(&self, ticket: &Ticket) -> Option<Option<Tokens>> {
        self.w
            .usage
            .tickets("SHOP")
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.id == ticket.id)
            .map(|t| t.tokens)
    }
}

fn tok(input: u64, output: u64, cache_write: u64, cache_read: u64) -> Tokens {
    Tokens {
        input,
        output,
        cache_write,
        cache_read,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn counts_are_read_per_response_incrementally_and_read_only() {
    let world = World::new().await;
    let w = &world.w;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Upgrade to Next.js 15").await;
    let worktree = w.give_worktree(&shop1).await;
    let t0 = now_ms() - 60_000;
    let session = world.session(&shop1, S1, t0 - 1000, None).await;
    let file = world.transcript(&worktree, S1);

    // A response written as two records (two content blocks) counts once; a broken line is skipped.
    let mut first = bookkeeping(t0);
    first.push(assistant("msg_1", t0 + 1000, 2, 226, 11_524, 28_134));
    first.push(assistant("msg_1", t0 + 1000, 2, 226, 11_524, 28_134));
    first.push(r#"{"type":"assistant","message":{"#.to_owned());
    first.push(assistant("msg_2", t0 + 2000, 3, 100, 500, 40_000));
    std::fs::write(&file, lines(&first)).unwrap();
    // A helper agent's conversation counts for the same session.
    let helpers = file.with_extension("").join("subagents");
    std::fs::create_dir_all(&helpers).unwrap();
    std::fs::write(
        helpers.join("agent-a82be8a61b78e612e.jsonl"),
        lines(&[assistant("msg_h", t0 + 3000, 2, 25, 28_269, 0)]),
    )
    .unwrap();
    let before = listing(world.claude.path());

    let events = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let mut stream = events.subscribe().await.unwrap();
    assert!(w.usage.read_now().await.unwrap());
    let day = local_day(t0 + 1000);
    assert_eq!(
        world.rows(&session).await,
        vec![(day.clone(), 7, 351, 40_293, 68_134)]
    );
    assert_eq!(
        world.ticket_tokens(&shop1).await,
        Some(Some(tok(7, 351, 40_293, 68_134)))
    );
    // "usage_changed" tells clients to ask again.
    let event = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Ok(otaq_core::protocol::Event::UsageChanged) = stream.recv().await {
                return;
            }
        }
    })
    .await;
    assert!(event.is_ok(), "usage_changed published");

    // Nothing new: nothing changes.
    assert!(!w.usage.read_now().await.unwrap());

    // A line still being written isn't read until it is complete; then only the new lines are read.
    let next = assistant("msg_3", t0 + 4000, 1, 10, 0, 50_000);
    let (head, tail) = next.split_at(40);
    append(&file, head);
    assert!(!w.usage.read_now().await.unwrap());
    append(&file, &format!("{tail}\n"));
    assert!(w.usage.read_now().await.unwrap());
    assert_eq!(
        world.rows(&session).await,
        vec![(day, 8, 361, 40_293, 118_134)]
    );

    // Today's total across all projects (written now, so today).
    let today = w.usage.day_total(&local_day(now_ms())).await.unwrap();
    if local_day(t0 + 4000) == local_day(now_ms()) {
        assert_eq!(today.tokens, tok(8, 361, 40_293, 118_134));
        // The dollar estimate (Opus 5.5 prices, 1-hour cache writes): $0.353223.
        assert_eq!(today.cost_micro_usd, 353_223);
    }

    // Read-only: the test's own appends aside, nothing in Claude Code's folder was created or removed.
    let names = |l: Vec<(String, u64, std::time::SystemTime)>| -> Vec<String> {
        l.into_iter().map(|(p, _, _)| p).collect()
    };
    assert_eq!(names(listing(world.claude.path())), names(before));
    drop(events);
    world.w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_or_unrecognised_file_is_unknown_and_nothing_else() {
    let world = World::new().await;
    let w = &world.w;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Coupon codes").await;
    let shop2 = w.create("SHOP", "Order confirmation email").await;
    let worktree1 = w.give_worktree(&shop1).await;
    let worktree2 = w.give_worktree(&shop2).await;
    let t0 = now_ms() - 60_000;
    let gone = world.session(&shop1, S1, t0, Some(t0 + 10_000)).await;
    world.session(&shop2, S2, t0, None).await;
    // Not Claude Code's format at all.
    std::fs::write(world.transcript(&worktree2, S2), "hello\nworld\n").unwrap();
    let _ = worktree1;

    assert!(w.usage.read_now().await.unwrap());
    assert_eq!(world.ticket_tokens(&shop1).await, Some(None));
    assert_eq!(world.ticket_tokens(&shop2).await, Some(None));
    assert!(world.rows(&gone).await.is_empty());
    let sessions = w.usage.sessions_of(&shop1.id).await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].tokens, None);
    // The tickets themselves are untouched.
    assert_eq!(w.board.ticket("SHOP-1").await.unwrap().status, shop1.status);
    // Asking again while nothing changed publishes nothing.
    assert!(!w.usage.read_now().await.unwrap());

    // Claude Code's records appear later in the file: the running conversation is known again.
    append(
        &world.transcript(&worktree2, S2),
        &lines(&[assistant("msg_1", t0 + 1000, 1, 2, 3, 4)]),
    );
    assert!(w.usage.read_now().await.unwrap());
    assert_eq!(
        world.ticket_tokens(&shop2).await,
        Some(Some(tok(1, 2, 3, 4)))
    );
    world.w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_ticket_sums_its_sessions_and_a_resumed_conversation_is_split_between_them() {
    let world = World::new().await;
    let w = &world.w;
    w.add_project("shop-web").await;
    w.add_project("billing-api").await;
    let shop1 = w.create("SHOP", "Split user full name").await;
    let bill1 = w.create("BILL", "Invoices").await;
    let worktree = w.give_worktree(&shop1).await;
    let bill_wt = w.give_worktree(&bill1).await;
    let t0 = now_ms() - 600_000;
    // The agent, then a resume of the same conversation (same Claude session ID), then a new conversation.
    let first = world.session(&shop1, S1, t0, Some(t0 + 100_000)).await;
    let resumed = world
        .session(&shop1, S1, t0 + 200_000, Some(t0 + 300_000))
        .await;
    let scratch = world.session(&shop1, S3, t0 + 400_000, None).await;
    world.session(&bill1, S2, t0, None).await;
    std::fs::write(
        world.transcript(&worktree, S1),
        lines(&[
            assistant("m1", t0 + 10_000, 1, 10, 100, 1000),
            assistant("m2", t0 + 210_000, 2, 20, 200, 2000),
        ]),
    )
    .unwrap();
    // Filed under another folder (long paths get a hash suffix): still found.
    let other = world
        .claude
        .path()
        .join("projects")
        .join("-long-path-1a2b3c");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(
        other.join(format!("{S3}.jsonl")),
        lines(&[assistant("m3", t0 + 410_000, 4, 40, 400, 4000)]),
    )
    .unwrap();
    std::fs::write(
        world.transcript(&bill_wt, S2),
        lines(&[assistant("b1", t0 + 1000, 8, 80, 800, 8000)]),
    )
    .unwrap();

    assert!(w.usage.read_now().await.unwrap());
    let sum = |rows: Vec<(String, i64, i64, i64, i64)>| rows.iter().map(|r| r.2).sum::<i64>();
    assert_eq!(sum(world.rows(&first).await), 10);
    assert_eq!(sum(world.rows(&resumed).await), 20);
    assert_eq!(sum(world.rows(&scratch).await), 40);
    assert_eq!(
        world.ticket_tokens(&shop1).await,
        Some(Some(tok(7, 70, 700, 7000)))
    );
    let per_session: Vec<_> = w
        .usage
        .sessions_of(&shop1.id)
        .await
        .unwrap()
        .into_iter()
        .map(|s| s.tokens.map(|t| t.output))
        .collect();
    assert_eq!(per_session, vec![Some(10), Some(20), Some(40)]);

    // Today across both projects, through the protocol.
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let reply = client
        .request(Request::UsageSummary(UsageParams {
            project: Some("BILL".into()),
        }))
        .await
        .unwrap();
    let Reply::Usage(summary) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(summary.day, local_day(now_ms()));
    // No status-line data and no limit here.
    assert_eq!(summary.plan, None);
    assert_eq!(summary.limit_waiting, 0);
    if local_day(t0) == summary.day {
        assert_eq!(summary.today.output, 150);
    }
    assert_eq!(summary.tickets.len(), 1);
    assert_eq!(summary.tickets[0].id, bill1.id);
    assert_eq!(summary.tickets[0].tokens.map(|t| t.shown()), Some(888));
    world.w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn after_a_restart_counts_are_rebuilt_not_doubled() {
    let world = World::new().await;
    world.w.add_project("shop-web").await;
    let shop1 = world.w.create("SHOP", "Checkout API").await;
    let worktree = world.w.give_worktree(&shop1).await;
    let t0 = now_ms() - 60_000;
    let session = world.session(&shop1, S1, t0, Some(t0 + 30_000)).await;
    std::fs::write(
        world.transcript(&worktree, S1),
        lines(&[assistant("m1", t0 + 1000, 1, 2, 3, 4)]),
    )
    .unwrap();
    assert!(world.w.usage.read_now().await.unwrap());
    let World { w, claude } = world;
    let dir = claude.path().to_owned();
    let w = w
        .restart_with(move |mut config| {
            config.tools.claude_config_dir = Some(dir);
            config
        })
        .await;
    let world = World { w, claude };
    world.w.usage.read_now().await.unwrap();
    assert_eq!(
        world.rows(&session).await,
        vec![(local_day(t0 + 1000), 1, 2, 3, 4)]
    );
    world.w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn validators_count_in_the_ticket_and_today_and_are_split_from_the_agent() {
    // VAL-14 (d0063): the card shows agent + validation; the panel splits them and counts the rounds. A cancelled
    // round's validator still counts its tokens but not as a round.
    let world = World::new().await;
    let w = &world.w;
    w.add_project("shop-web").await;
    let shop1 = w.create("SHOP", "Split user full name").await;
    let shop2 = w.create("SHOP", "Checkout API").await;
    let worktree = w.give_worktree(&shop1).await;
    let t0 = now_ms() - 600_000;
    world.session(&shop1, S1, t0, Some(t0 + 100_000)).await;
    let passed = world
        .session_of_kind(&shop1, "validator", S2, t0 + 200_000, Some(t0 + 250_000))
        .await;
    let cancelled = world
        .session_of_kind(&shop1, "validator", S3, t0 + 300_000, Some(t0 + 350_000))
        .await;
    world.round(&shop1, 1, &passed, t0 + 200_000, "pass").await;
    world
        .round(&shop1, 2, &cancelled, t0 + 300_000, "cancelled")
        .await;
    std::fs::write(
        world.transcript(&worktree, S1),
        lines(&[assistant("m1", t0 + 10_000, 1, 10, 100, 1000)]),
    )
    .unwrap();
    std::fs::write(
        world.transcript(&worktree, S2),
        lines(&[assistant("v1", t0 + 210_000, 2, 20, 200, 2000)]),
    )
    .unwrap();
    std::fs::write(
        world.transcript(&worktree, S3),
        lines(&[assistant("v2", t0 + 310_000, 4, 40, 400, 4000)]),
    )
    .unwrap();

    assert!(w.usage.read_now().await.unwrap());
    let all = w.usage.tickets("SHOP").await.unwrap();
    let one: &TicketUsage = all.iter().find(|t| t.id == shop1.id).unwrap();
    assert_eq!(one.tokens, Some(tok(7, 70, 700, 7000)));
    assert_eq!(one.validation, Some(tok(6, 60, 600, 6000)));
    assert_eq!(one.agent(), Some(tok(1, 10, 100, 1000)));
    assert_eq!(one.rounds, 1);
    // A ticket that never ran has no entry; rounds belong to their own ticket.
    assert!(all.iter().all(|t| t.id != shop2.id));

    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let reply = client
        .request(Request::UsageSummary(UsageParams {
            project: Some("SHOP".into()),
        }))
        .await
        .unwrap();
    let Reply::Usage(summary) = reply else {
        panic!("{reply:?}")
    };
    if local_day(t0) == summary.day {
        assert_eq!(summary.today.output, 70);
    }
    let wire = summary.tickets.iter().find(|t| t.id == shop1.id).unwrap();
    assert_eq!(wire, one);
    world.w.stop().await;
}
