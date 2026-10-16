//! Claude Code's folder trust recorded by the service (PRJ-06, PRJ-08; d0085): at Add project, in the startup pass
//! for projects added earlier, the one-time note and the projects whose trust couldn't be recorded.
//!
//! Every service here gets a temporary Claude Code config file (or none); nothing reads or writes the real one.

use std::path::{Path, PathBuf};
use std::time::Duration;

use otaq_core::board::projects::AddProject;
use otaq_core::claude_trust::{ClaudeConfig, Outcome, TrustFailure, trust_key};
use otaq_core::client::{Client, ClientConfig};
use otaq_core::protocol::{Event, OnboardingState, Reply, Request};
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;
use serde_json::{Value, json};
use tempfile::TempDir;

/// A temporary Claude Code config folder; the file itself starts missing.
struct Claude {
    dir: TempDir,
}

impl Claude {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn file(&self) -> PathBuf {
        self.dir.path().join(".claude.json")
    }

    fn config(&self) -> ClaudeConfig {
        ClaudeConfig::new(self.file(), None)
    }

    /// A service configuration using this file.
    fn with(&self) -> impl FnOnce(ServiceConfig) -> ServiceConfig + use<> {
        let claude = Some(self.config());
        move |config| ServiceConfig { claude, ..config }
    }

    fn read(&self) -> Value {
        serde_json::from_slice(&std::fs::read(self.file()).unwrap()).unwrap()
    }

    fn write(&self, value: &Value) {
        std::fs::write(self.file(), serde_json::to_string_pretty(value).unwrap()).unwrap();
    }

    /// The trust entry of a stored repo path, `None` when there is none.
    fn trusted(&self, repo_path: &str) -> Option<bool> {
        self.read()["projects"][trust_key(Path::new(repo_path))]["hasTrustDialogAccepted"].as_bool()
    }
}

async fn client(w: &TestService) -> Client {
    Client::connect(ClientConfig::new(&w.data)).await.unwrap()
}

async fn state(client: &Client) -> OnboardingState {
    match client.request(Request::OnboardingState).await.unwrap() {
        Reply::OnboardingState(state) => state,
        other => panic!("{other:?}"),
    }
}

/// Adds a repository over the socket, as the app does: the project's key and the trust outcome.
async fn add(client: &Client, repo: &Path) -> (String, Option<Outcome>) {
    match client
        .request(Request::ProjectAdd(AddProject {
            path: repo.display().to_string(),
            ..Default::default()
        }))
        .await
        .unwrap()
    {
        Reply::ProjectAdded { project, trust } => (project.key, trust),
        other => panic!("{other:?}"),
    }
}

async fn repo_path(w: &TestService, key: &str) -> String {
    w.board.project(key).await.unwrap().repo_path
}

/// Restarts with the config file and waits for the startup pass.
async fn restart(w: TestService, claude: &Claude) -> TestService {
    let w = w.restart_with(claude.with()).await;
    w.board.trust().pass_finished().await;
    w
}

#[tokio::test]
async fn add_project_records_the_trust_and_answers_the_outcome() {
    let claude = Claude::new();
    let w = TestService::start_with(claude.with()).await;
    let c = client(&w).await;

    let (shop, trust) = add(&c, &w.repos.repo("shop-web", 1)).await;
    assert_eq!(trust, Some(Outcome::Recorded));
    assert_eq!(claude.trusted(&repo_path(&w, &shop).await), Some(true));

    // Trusted already (by Claude Code itself, say): the answer says so.
    let notes = w.repos.repo("pocket-notes", 1);
    let notes_key = trust_key(&otaq_core::paths::canonical(&notes).unwrap());
    let mut config = claude.read();
    config["projects"][&notes_key] = json!({ "hasTrustDialogAccepted": true });
    claude.write(&config);
    assert_eq!(add(&c, &notes).await.1, Some(Outcome::AlreadyTrusted));

    // Add project never sets the one-time note: the add-project line says it.
    let state = state(&c).await;
    assert!(!state.trust_note);
    assert_eq!(state.trust_failed, Vec::new());
    w.stop().await;
}

#[tokio::test]
async fn an_unwritable_config_still_adds_the_project_and_a_later_startup_records_it() {
    // The config folder doesn't exist: nothing can be written there.
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("not-yet");
    let config = ClaudeConfig::new(folder.join(".claude.json"), None);
    let w = TestService::start_with({
        let config = config.clone();
        move |c| ServiceConfig {
            claude: Some(config),
            ..c
        }
    })
    .await;
    let c = client(&w).await;
    let mut events = c.subscribe().await.unwrap();

    let (key, trust) = add(&c, &w.repos.repo("pocket-notes", 1)).await;
    let Some(Outcome::Failed { reason }) = trust else {
        panic!("{trust:?}")
    };
    assert_eq!(reason, "Claude Code's config file couldn't be written");
    assert_eq!(key, "POCK");
    assert!(
        w.board.project("POCK").await.is_ok(),
        "the project is added"
    );
    let failed = vec![TrustFailure {
        project_key: "POCK".into(),
        reason: reason.clone(),
    }];
    assert_eq!(state(&c).await.trust_failed, failed);
    tokio::time::timeout(Duration::from_secs(10), async {
        while events.recv().await.unwrap() != Event::TrustChanged {}
    })
    .await
    .expect("trust_changed sent");

    // A later start retries it: once the folder can be written, the project leaves the list.
    std::fs::create_dir(&folder).unwrap();
    drop(c);
    let w = w
        .restart_with(move |c| ServiceConfig {
            claude: Some(config),
            ..c
        })
        .await;
    w.board.trust().pass_finished().await;
    let c = client(&w).await;
    let state = state(&c).await;
    assert_eq!(state.trust_failed, Vec::new());
    // It was recorded by the startup pass, so the one-time note shows.
    assert!(state.trust_note);
    let written: Value =
        serde_json::from_slice(&std::fs::read(folder.join(".claude.json")).unwrap()).unwrap();
    let key = trust_key(Path::new(&repo_path(&w, "POCK").await));
    assert_eq!(written["projects"][key]["hasTrustDialogAccepted"], true);
    w.stop().await;
}

#[tokio::test]
async fn the_startup_pass_records_earlier_projects_and_the_note_shows_until_dismissed() {
    let claude = Claude::new();
    // Projects added before Otaq recorded trust: a service without a config file.
    let w = TestService::start().await;
    let shop = w.add_project("shop-web").await;
    let bill = w.add_project("billing-api").await;
    let notes = w.add_project("pocket-notes").await;
    claude.write(&json!({
        "numStartups": 12,
        "projects": {
            trust_key(Path::new(&notes.repo_path)): { "hasTrustDialogAccepted": true, "allowedTools": ["Bash"] },
            "/elsewhere": { "hasTrustDialogAccepted": false },
        },
    }));

    let w = restart(w, &claude).await;
    assert_eq!(claude.trusted(&shop.repo_path), Some(true));
    assert_eq!(claude.trusted(&bill.repo_path), Some(true));
    let config = claude.read();
    assert_eq!(config["numStartups"], 12);
    assert_eq!(
        config["projects"]["/elsewhere"]["hasTrustDialogAccepted"],
        false
    );
    assert_eq!(
        config["projects"][trust_key(Path::new(&notes.repo_path))]["allowedTools"],
        json!(["Bash"])
    );
    let c = client(&w).await;
    assert!(state(&c).await.trust_note);

    // Dismissed: cleared, and it stays away after a restart.
    let mut events = c.subscribe().await.unwrap();
    assert_eq!(
        c.request(Request::DismissTrustNote).await.unwrap(),
        Reply::Ok
    );
    assert_eq!(events.recv().await.unwrap(), Event::TrustChanged);
    assert!(!state(&c).await.trust_note);
    drop(c);

    // A later start that records again doesn't bring it back.
    let mut config = claude.read();
    config["projects"][trust_key(Path::new(&shop.repo_path))]["hasTrustDialogAccepted"] =
        json!(false);
    claude.write(&config);
    let w = restart(w, &claude).await;
    assert_eq!(claude.trusted(&shop.repo_path), Some(true));
    let c = client(&w).await;
    assert!(!state(&c).await.trust_note);
    w.stop().await;
}

#[tokio::test]
async fn a_startup_with_nothing_to_record_never_sets_the_note() {
    let claude = Claude::new();
    // No projects at all.
    let w = restart(TestService::start().await, &claude).await;
    assert!(!state(&client(&w).await).await.trust_note);
    assert!(
        !claude.file().exists(),
        "nothing to record: the file isn't created"
    );

    // Every project trusted at Add project already.
    w.add_project("shop-web").await;
    let w = restart(w, &claude).await;
    let state = state(&client(&w).await).await;
    assert!(!state.trust_note);
    assert_eq!(state.trust_failed, Vec::new());
    w.stop().await;
}

#[tokio::test]
async fn a_project_whose_folder_is_gone_is_skipped() {
    let claude = Claude::new();
    let w = TestService::start().await;
    let gone = w.add_project("old-site").await;
    let kept = w.add_project("shop-web").await;
    std::fs::remove_dir_all(&gone.repo_path).unwrap();

    let w = restart(w, &claude).await;
    assert_eq!(claude.trusted(&gone.repo_path), None);
    assert_eq!(claude.trusted(&kept.repo_path), Some(true));
    let state = state(&client(&w).await).await;
    assert_eq!(state.trust_failed, Vec::new(), "skipped, not failed");
    w.stop().await;
}

#[tokio::test]
async fn the_service_starts_when_the_config_file_is_not_json() {
    let claude = Claude::new();
    let w = TestService::start().await;
    w.add_project("pocket-notes").await;
    std::fs::write(claude.file(), "{ not json").unwrap();

    let w = restart(w, &claude).await;
    let c = client(&w).await;
    assert_eq!(c.request(Request::Ping).await.unwrap(), Reply::Pong);
    assert_eq!(std::fs::read(claude.file()).unwrap(), b"{ not json");
    assert_eq!(
        state(&c).await.trust_failed,
        vec![TrustFailure {
            project_key: "POCK".into(),
            reason: "Claude Code's config file couldn't be read".into(),
        }]
    );
    w.stop().await;
}
