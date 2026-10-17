//! `otaq create` / `otaq update` with `--developer`, `--validator`, `--lead`, `--reason` and `--by-user`, and `-f`
//! lines (T-0338; AGT-06, CLI-03, CLI-06; d0086, d0087): the user's rows are *set by you*, a Project agent's
//! *picked by* its name; `show` prints the rows. `claude` stands in as an ordinary `sh`; no test starts claude.

use std::process::{Command, Output};
use std::sync::Arc;

use otaq_core::agent_tools::claude::ClaudeLaunch;
use otaq_core::agent_tools::{LaunchContext, Tool};
use otaq_core::data_dir::DataDir;
use otaq_core::git::testing::TempRepos;
use otaq_core::named_agents::starter::{self, ConnectedTool};
use otaq_core::named_agents::{AgentFields, FixedModels, NamedAgents};
use otaq_core::naming;
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;
use serde_json::{Value, json};

const CB: &str = env!("CARGO_BIN_EXE_otaq");

/// The sample board with four named agents (Claude Code, Strong, Light, Sonnet); `claude` is an interactive `sh`.
async fn world() -> TestService {
    let home = tempfile::TempDir::new().unwrap();
    let data = home.path().join("h");
    let script = format!(
        "export {}='{}'; export PS1=''; exec sh",
        DataDir::env_var(),
        data.display()
    );
    let w = TestService::start_in(home, TempRepos::new(), |config| ServiceConfig {
        launch: Some(LaunchContext {
            bin_dir: "/nonexistent".into(),
            plugin_dir: "/nonexistent/plugin".into(),
            path: std::env::var_os("PATH").unwrap_or_default(),
            claude: ClaudeLaunch {
                program: vec!["sh".into(), "-c".into(), script, "sh".into()],
                config_dir: None,
            },
        }),
        ..config
    })
    .await;
    w.shop_sample().await;
    let claude_code = ConnectedTool {
        tool: Tool::ALL[0].as_str().into(),
        name: "Claude Code".into(),
    };
    starter::ensure(w.board.store(), &[claude_code])
        .await
        .unwrap();
    let agents = NamedAgents::new(w.board.clone(), Arc::new(FixedModels::sample()));
    for name in ["Strong", "Light", "Sonnet"] {
        agents
            .add(&AgentFields {
                name: name.into(),
                tool: Tool::ALL[0],
                model: None,
                use_for: String::new(),
                fallback: None,
            })
            .await
            .unwrap();
    }
    w
}

fn cb(w: &TestService, args: &[&str]) -> Output {
    Command::new(CB)
        .args(args)
        .env(DataDir::env_var(), w.data.root())
        .env_remove(naming::ticket_env_var())
        .current_dir(w.home.path())
        .output()
        .unwrap()
}

fn ok(out: &Output) -> String {
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn json_of(out: &Output) -> Value {
    serde_json::from_str(&ok(out)).unwrap()
}

/// The error of a failed `--json` command: (kind, message).
fn error(out: &Output) -> (String, String) {
    assert!(!out.status.success());
    let e = &serde_json::from_slice::<Value>(&out.stdout).unwrap()["error"];
    (
        e["kind"].as_str().unwrap().to_owned(),
        e["message"].as_str().unwrap().to_owned(),
    )
}

fn usage(w: &TestService, args: &[&str]) -> String {
    let mut args = args.to_vec();
    args.push("--json");
    let (kind, message) = error(&cb(w, &args));
    assert_eq!(kind, "usage", "{message}");
    message
}

#[tokio::test(flavor = "multi_thread")]
async fn each_flag_on_create_and_update_from_the_user() {
    let w = world().await;

    // A ticket's two rows, names ignoring case; set by you, the reason unused.
    let ticket = json_of(&cb(
        &w,
        &[
            "-P",
            "SHOP",
            "create",
            "Fix README typo",
            "--developer",
            "light",
            "--validator",
            "Strong",
            "--json",
        ],
    ));
    assert_eq!(ticket["key"], "SHOP-41");
    assert_eq!(
        ticket["agents"],
        json!([
            {"role": "developer", "agent": "Light", "by": "user"},
            {"role": "validator", "agent": "Strong", "by": "user"},
        ])
    );
    assert_eq!(
        ok(&cb(
            &w,
            &[
                "-P",
                "SHOP",
                "create",
                "Update footer links",
                "--developer",
                "Light"
            ]
        )),
        "Created SHOP-42 · Light\n"
    );
    // An epic's lead.
    assert_eq!(
        ok(&cb(
            &w,
            &[
                "-P",
                "SHOP",
                "create",
                "Order history",
                "--type",
                "epic",
                "--lead",
                "strong"
            ]
        )),
        "Created SHOP-43 · Strong\n"
    );
    // No flags: the title, as before; the rows are empty and name the default.
    let plain = json_of(&cb(&w, &["-P", "SHOP", "create", "Plain", "--json"]));
    assert_eq!(
        plain["agents"],
        json!([
            {"role": "developer", "default": "Claude Code"},
            {"role": "validator", "default": "Claude Code"},
        ])
    );

    // update: one flag, then `default` empties the row.
    assert_eq!(
        ok(&cb(&w, &["update", "SHOP-42", "--validator", "Sonnet"])),
        "Updated SHOP-42 · Validator Sonnet\n"
    );
    assert_eq!(
        ok(&cb(&w, &["update", "SHOP-41", "--developer", "default"])),
        "Updated SHOP-41 · Developer: project default\n"
    );
    assert_eq!(
        ok(&cb(&w, &["update", "SHOP-43", "--lead", "Light"])),
        "Updated SHOP-43 · Light\n"
    );
    let shown = ok(&cb(&w, &["show", "SHOP-41"]));
    assert!(
        shown.contains(
            "\nDeveloper:   Project default (Claude Code)\nValidator:   Strong · set by you\n"
        ),
        "{shown}"
    );
    let shown = ok(&cb(&w, &["show", "SHOP-43"]));
    assert!(
        shown.contains("\nEpic lead:   Light · set by you\n"),
        "{shown}"
    );
    let shown = json_of(&cb(&w, &["show", "SHOP-42", "--json"]));
    assert_eq!(
        shown["agents"][1],
        json!({"role": "validator", "agent": "Sonnet", "by": "user"})
    );

    // An unknown name lists the agents.
    let (kind, message) = error(&cb(
        &w,
        &["update", "SHOP-41", "--developer", "Lite", "--json"],
    ));
    assert_eq!(kind, "not_found");
    assert_eq!(
        message,
        "No agent named Lite. Agents: Claude Code, Strong, Light, Sonnet"
    );
    let count = w.board.tickets("SHOP").await.unwrap().len();
    error(&cb(
        &w,
        &[
            "-P",
            "SHOP",
            "create",
            "Nope",
            "--developer",
            "Lite",
            "--json",
        ],
    ));
    assert_eq!(w.board.tickets("SHOP").await.unwrap().len(), count);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn usage_errors_for_the_wrong_role_and_lone_flags() {
    let w = world().await;
    assert_eq!(
        usage(&w, &["-P", "SHOP", "create", "T", "--lead", "Strong"]),
        "--lead is for epics (--type epic); a ticket has --developer and --validator"
    );
    for flag in ["--developer", "--validator"] {
        assert_eq!(
            usage(
                &w,
                &[
                    "-P", "SHOP", "create", "E", "--type", "epic", flag, "Strong"
                ]
            ),
            "an epic has an Epic lead row: give --lead, not --developer or --validator"
        );
    }
    for flags in [&["--reason", "x"][..], &["--by-user"][..]] {
        let mut args = vec!["-P", "SHOP", "create", "T"];
        args.extend_from_slice(flags);
        assert_eq!(
            usage(&w, &args),
            "--reason and --by-user go with --developer, --validator or --lead"
        );
    }
    assert_eq!(
        usage(&w, &["update", "SHOP-13", "--reason", "x"]),
        "--reason and --by-user go with --developer, --validator or --lead"
    );
    // update doesn't know the kind: the service refuses the row.
    let (kind, message) = error(&cb(
        &w,
        &["update", "SHOP-40", "--developer", "Strong", "--json"],
    ));
    assert_eq!(kind, "refused");
    assert_eq!(
        message,
        "SHOP-40 is an epic: it has an Epic lead row, not a Developer row"
    );
    let plan = w.home.path().join("plan.md");
    std::fs::write(&plan, "### One\n").unwrap();
    assert!(
        usage(
            &w,
            &[
                "-P",
                "SHOP",
                "create",
                "-f",
                plan.to_str().unwrap(),
                "--developer",
                "Light"
            ]
        )
        .starts_with("-f takes everything from the file")
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_with_agent_lines_and_a_project_agents_picks() {
    let w = world().await;
    let opened = w
        .project_agents
        .open("SHOP", "Look around the checkout code")
        .await
        .unwrap();
    let agent = w.shell_in(&opened.session.as_ref().unwrap().id);
    let ran = agent
        .run(&format!("'{CB}' agent name 'Checkout slowness'"))
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");

    // `otaq create "Fix README typo" --developer Light --reason "docs-only change"` (18 `dg0ak`).
    let ran = agent
        .run(&format!(
            "'{CB}' -P SHOP create 'Fix README typo' --developer Light --reason 'docs-only change'"
        ))
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    assert_eq!(
        ran.stdout.trim(),
        "Created SHOP-41 · Light: docs-only change"
    );
    let shown = ok(&cb(&w, &["show", "SHOP-41"]));
    assert!(
        shown.contains("\nDeveloper:   Light · picked by Checkout slowness: docs-only change\n"),
        "{shown}"
    );
    // Without a reason: refused.
    let ran = agent
        .run(&format!("'{CB}' update SHOP-13 --developer Light"))
        .await;
    assert_ne!(ran.status, 0);
    assert!(ran.stderr.contains("Say why with --reason"), "{ran:?}");

    // A file: one line per row, its reason after " · "; one bad name fails the whole file.
    let plan = w.home.path().join("plan.md");
    std::fs::write(
        &plan,
        "### E1 · Order history\n- **Type:** epic\n- **Epic lead:** Strong · two API changes\n\n\
         ### Order list\n- **Parent:** E1\n- **Developer:** Sonnet · normal ticket\n\
         - **Validator:** Light · small UI\n",
    )
    .unwrap();
    let ran = agent
        .run(&format!("'{CB}' -P SHOP create -f '{}'", plan.display()))
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    assert_eq!(
        ran.stdout.trim(),
        "Created SHOP-42 · Strong: two API changes\n\
         Created SHOP-42.1 · Sonnet: normal ticket · Validator Light: small UI"
    );
    let bad = w.home.path().join("bad.md");
    std::fs::write(&bad, "### A\n\n### B\n- **Developer:** Lite · x\n").unwrap();
    let count = w.board.tickets("SHOP").await.unwrap().len();
    let ran = agent
        .run(&format!("'{CB}' -P SHOP create -f '{}'", bad.display()))
        .await;
    assert_ne!(ran.status, 0);
    assert!(ran.stderr.contains("No agent named Lite"), "{ran:?}");
    assert_eq!(w.board.tickets("SHOP").await.unwrap().len(), count);

    // `--by-user` makes the whole file the user's.
    std::fs::write(&plan, "### Gift wrap\n- **Developer:** Strong\n").unwrap();
    let ran = agent
        .run(&format!(
            "'{CB}' -P SHOP create -f '{}' --by-user --json",
            plan.display()
        ))
        .await;
    assert_eq!(ran.status, 0, "{ran:?}");
    let created: Value = serde_json::from_str(&ran.stdout).unwrap();
    assert_eq!(
        created[0]["agents"][0],
        json!({"role": "developer", "agent": "Strong", "by": "user"})
    );
    w.stop().await;
}
