//! T-0141 (REV-11, REV-12): the PR provider follows `origin`'s URL and its CLI's login, through the service. The CLIs
//! are ordinary scripts named `gh` / `az` on a test PATH; nothing calls a real provider.

use std::path::{Path, PathBuf};

use crewboard_core::client::request_once;
use crewboard_core::git::testing::git;
use crewboard_core::pr::provider::{PrProvider, Provider};
use crewboard_core::protocol::{ProjectParams, Reply, Request};
use crewboard_core::service::testing::TestService;
use crewboard_core::tools::CheckEnv;

/// What a stand-in `gh auth status` prints; it must never end up in Otaq's files.
const SENTINEL: &str = "gho_SENTINELtoken0000";

fn stub(dir: &Path, name: &str, script: &str) {
    crewboard_core::process::testing::stub_command(dir, name, script);
}

fn env(bin: &Path, tmp: &Path) -> CheckEnv {
    CheckEnv {
        path: bin.as_os_str().to_owned(),
        claude_config_dir: Some(tmp.join("claude")),
        managed_settings_dir: tmp.join("managed"),
    }
}

async fn provider(w: &TestService) -> PrProvider {
    let reply = request_once(
        &w.data.socket_path(),
        Request::ProjectPrProvider(ProjectParams {
            project: "SHOP".into(),
        }),
    )
    .await
    .unwrap();
    let Reply::PrProvider(p) = reply else {
        panic!("{reply:?}")
    };
    p
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
async fn github_origin_with_gh_logged_in_then_other_remotes() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    stub(
        &bin,
        "gh",
        &format!(
            r#"case "$1" in --version) echo "gh version 2.62.0 (2024-11-14)";; auth) echo "Token: {SENTINEL}"; exit 0;; esac"#
        ),
    );
    stub(
        &bin,
        "az",
        r#"case "$1" in version) echo '{"azure-cli": "2.67.0", "extensions": {"azure-devops": "1.0.1"}}';; account) exit 1;; esac"#,
    );
    let check_env = env(&bin, tmp.path());
    let w = TestService::start_with(move |mut c| {
        c.tools = check_env;
        c
    })
    .await;
    let project = w.add_project("shop-web").await;
    let repo = Path::new(&project.repo_path);

    let none = provider(&w).await;
    assert_eq!(none.remote, None, "no origin");
    assert_eq!(none.provider, None);
    assert_eq!(none.open_pr_blocked(), Some("PR provider not detected"));

    git(
        repo,
        &[
            "remote",
            "add",
            "origin",
            "git@github.com:acme/shop-web.git",
        ],
    );
    let gh = provider(&w).await;
    assert_eq!(gh.provider, Some(Provider::GitHub));
    assert_eq!(gh.remote.as_deref(), Some("github.com/acme/shop-web"));
    assert_eq!(
        gh.summary(),
        "GitHub (detected from origin) \u{b7} gh: logged in"
    );
    assert_eq!(gh.open_pr_blocked(), None);

    git(
        repo,
        &[
            "remote",
            "set-url",
            "origin",
            "https://dev.azure.com/acme/shop/_git/shop-web",
        ],
    );
    let az = provider(&w).await;
    assert_eq!(az.provider, Some(Provider::AzureDevOps));
    assert_eq!(
        az.summary(),
        "Azure DevOps (detected from origin) \u{b7} az: not logged in"
    );

    git(
        repo,
        &[
            "remote",
            "set-url",
            "origin",
            "https://git.acme.internal/web/handbook.git",
        ],
    );
    let other = provider(&w).await;
    assert_eq!(other.provider, None);
    assert_eq!(
        other.remote.as_deref(),
        Some("git.acme.internal/web/handbook")
    );
    assert_eq!(other.summary(), "Not detected: use your own Open PR skill");

    // A custom Open PR skill works with any remote.
    let mut settings = w.board.project_settings("SHOP").await.unwrap();
    settings.workflow_skills.open_pr = "/my-pr".into();
    w.board
        .set_project_settings("SHOP", &settings)
        .await
        .unwrap();
    assert_eq!(provider(&w).await.open_pr_blocked(), None);

    // Nothing Otaq stores holds what the CLI printed about its token (REV-12).
    let _ = provider(&w).await;
    let mut files = Vec::new();
    files_under(w.data.root(), &mut files);
    for file in files {
        let bytes = std::fs::read(&file).unwrap_or_default();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(SENTINEL),
            "{}",
            file.display()
        );
    }
    w.stop().await;
}
