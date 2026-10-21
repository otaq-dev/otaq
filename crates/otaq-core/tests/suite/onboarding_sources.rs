//! T-0427 (ONB-03, d0108): what there is for Flow onboarding to look at, through the service. `gh` and `glab` are
//! ordinary scripts on a test PATH and the history file is made up; nothing reads the user's own `~/.claude/`.

use std::path::{Path, PathBuf};

use otaq_core::agent_tools::Tool as AgentTool;
use otaq_core::agents::{AgentModel, Role};
use otaq_core::board::projects::AddProject;
use otaq_core::client::request_once;
use otaq_core::onboarding::{HISTORY_FILE, Sources};
use otaq_core::protocol::{ProjectParams, Reply, Request};
use otaq_core::service::testing::TestService;
use otaq_core::tools::CheckEnv;

/// Words of the made-up paste and prompt; none of them may leave the history file.
const PASTED: &str = "SENTINEL-pasted-stack-trace";
const TYPED: &str = "[Pasted text #1 +42 lines]";

fn stub(dir: &Path, name: &str, script: &str) {
    otaq_core::process::testing::stub_command(dir, name, script);
}

struct Mac {
    _tmp: tempfile::TempDir,
    bin: PathBuf,
    claude: PathBuf,
}

/// A throwaway Mac: an empty PATH folder and an empty Claude Code config folder.
fn mac() -> Mac {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    let claude = tmp.path().join("claude");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&claude).unwrap();
    Mac {
        bin,
        claude,
        _tmp: tmp,
    }
}

async fn start(mac: &Mac) -> TestService {
    let env = CheckEnv {
        path: mac.bin.as_os_str().to_owned(),
        claude_config_dir: Some(mac.claude.clone()),
        managed_settings_dir: mac.claude.join("managed"),
    };
    TestService::start_with(move |mut c| {
        c.tools = env;
        c
    })
    .await
}

async fn sources(w: &TestService, project: &str) -> (Sources, String) {
    let reply = request_once(
        &w.data.socket_path(),
        Request::OnboardingSources(ProjectParams {
            project: project.into(),
        }),
    )
    .await
    .unwrap();
    let text = serde_json::to_string(&reply).unwrap();
    let Reply::OnboardingSources(s) = reply else {
        panic!("{reply:?}")
    };
    (s, text)
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_mac_has_nothing_to_look_at() {
    let mac = mac();
    let w = start(&mac).await;
    let repo = w.repos.repo("empty-api", 0);
    w.board
        .add_project(&AddProject {
            path: repo.display().to_string(),
            key: Some("API".into()),
            base_branch: Some("main".into()),
        })
        .await
        .unwrap();

    let (s, _) = sources(&w, "API").await;
    assert_eq!(
        s,
        Sources {
            project_name: "empty-api".into(),
            history: false,
            commits: false,
            gh: false,
            glab: false,
            tool: AgentTool::Claude,
        }
    );
    assert!(!s.any());

    // An empty history file, and gh / glab found but signed out: still nothing.
    std::fs::write(mac.claude.join(HISTORY_FILE), "").unwrap();
    stub(&mac.bin, "gh", r#"case "$1" in auth) exit 1;; esac"#);
    stub(&mac.bin, "glab", r#"case "$1" in auth) exit 1;; esac"#);
    assert!(!sources(&w, "API").await.0.any());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_paste_only_history_counts_and_none_of_it_leaves_the_file() {
    let mac = mac();
    // Made up, shaped like the real file: one entry that is only a paste.
    std::fs::write(
        mac.claude.join(HISTORY_FILE),
        format!(
            r#"{{"display":"{TYPED}","pastedContents":{{"1":{{"id":1,"type":"text","content":"{PASTED}"}}}},"project":"/Users/sam/pocket-notes","sessionId":"3be94c10","timestamp":1760000000000}}"#
        ) + "\n",
    )
    .unwrap();
    stub(
        &mac.bin,
        "gh",
        r#"case "$1" in --version) echo "gh version 2.62.0 (2024-11-14)";; auth) exit 0;; esac"#,
    );
    let w = start(&mac).await;
    let project = w.add_project("pocket-notes").await;

    let (s, text) = sources(&w, project.key.as_str()).await;
    assert!(s.history && s.commits && s.gh && !s.glab, "{s:?}");
    assert_eq!(s.project_name, "pocket-notes");
    for words in [PASTED, TYPED, "/Users/sam", "3be94c10"] {
        assert!(!text.contains(words), "{words} in the answer: {text}");
    }
    // Nor in Otaq's own files (the log, the database).
    let mut files = Vec::new();
    files_under(w.data.root(), &mut files);
    for file in files {
        let bytes = std::fs::read(&file).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            !text.contains(PASTED) && !text.contains(TYPED),
            "{} holds history content",
            file.display()
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn glab_alone_and_the_agent_follows_the_project_agents_tool() {
    let mac = mac();
    stub(
        &mac.bin,
        "glab",
        r#"case "$1" in --version) echo "glab 1.46.1 (2024-09-12)";; auth) exit 0;; esac"#,
    );
    let w = start(&mac).await;
    let repo = w.repos.repo("billing-api", 0);
    w.board
        .add_project(&AddProject {
            path: repo.display().to_string(),
            key: Some("BILL".into()),
            base_branch: Some("main".into()),
        })
        .await
        .unwrap();
    let (s, _) = sources(&w, "BILL").await;
    assert!(
        s.glab && !s.gh && !s.commits && !s.history && s.any(),
        "{s:?}"
    );
    assert_eq!(s.tool, AgentTool::Claude);

    let agent = w
        .board
        .store()
        .role_agent(Role::ProjectAgent)
        .await
        .unwrap()
        .expect("a Project agent");
    w.set_agent_model(
        &agent.name,
        AgentModel::Tool {
            tool: AgentTool::Codex,
            model: None,
        },
    )
    .await;
    assert_eq!(sources(&w, "BILL").await.0.tool, AgentTool::Codex);
}
