//! What a Project terminal created (T-0167, TKT-08): a process inside the terminal (the user's shell, or a `claude`
//! started there) is attributed to that terminal by caller identity; the summary lists what was created from it
//! since it was last dismissed.

use std::time::Duration;

use crewboard_core::caller;
use crewboard_core::service::ServiceConfig;
use crewboard_core::service::testing::TestService;

fn plain_sh(mut c: ServiceConfig) -> ServiceConfig {
    c.terminals.shell = "sh".into();
    c.terminals.shell_args = Vec::new();
    c
}

#[tokio::test(flavor = "multi_thread")]
async fn processes_in_the_project_terminal_are_attributed_to_it() {
    let w = TestService::start_with(plain_sh).await;
    w.add_project("shop-web").await;
    let terminal = w.shells.open_project("SHOP", false).await.unwrap();
    let pid_file = w.home.path().join("child.pid");
    w.sessions
        .write(
            &terminal.id,
            // Git for Windows' sh numbers processes its own way; `/proc/<pid>/winpid` is the Windows ID.
            if cfg!(windows) {
                format!(
                    "sleep 30 & cat /proc/$!/winpid > '{}'\r",
                    pid_file.display()
                )
            } else {
                format!("sleep 30 & echo $! > '{}'\n", pid_file.display())
            }
            .as_bytes(),
        )
        .unwrap();
    let pid: u32 = tokio::time::timeout(Duration::from_secs(10), async {
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
    assert_eq!(
        caller::project_terminal_of(Some(pid), &w.sessions),
        Some(terminal.id.clone())
    );
    assert_eq!(
        caller::project_terminal_of(Some(std::process::id()), &w.sessions),
        None,
        "a process outside it isn't"
    );
    assert_eq!(caller::project_terminal_of(None, &w.sessions), None);
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_summary_lists_what_was_created_since_it_was_dismissed() {
    let w = TestService::start_with(plain_sh).await;
    w.add_project("shop-web").await;
    assert_eq!(
        w.board.created_in_terminal("SHOP").await.unwrap().session,
        None
    );
    let terminal = w.shells.open_project("SHOP", false).await.unwrap();
    let elsewhere = w.create("SHOP", "Made in the app").await;
    let a = w.create("SHOP", "Checkout API").await;
    let b = w.create("SHOP", "Price table").await;
    w.board
        .mark_created_in(&[a.id.clone(), b.id.clone()], &terminal.id)
        .await
        .unwrap();
    let here = w.board.created_in_terminal("SHOP").await.unwrap();
    assert_eq!(here.session.as_deref(), Some(terminal.id.as_str()));
    let keys: Vec<String> = here.tickets.iter().map(|t| t.key.to_string()).collect();
    assert_eq!(keys, ["SHOP-2", "SHOP-3"]);
    assert!(!keys.contains(&elsewhere.key.to_string()));

    // Deleted tickets leave the summary.
    w.board.delete_ticket("SHOP-3").await.unwrap();
    assert_eq!(
        w.board
            .created_in_terminal("SHOP")
            .await
            .unwrap()
            .tickets
            .len(),
        1
    );

    // Dismissed: only what comes after.
    w.board.dismiss_created("SHOP").await.unwrap();
    assert!(
        w.board
            .created_in_terminal("SHOP")
            .await
            .unwrap()
            .tickets
            .is_empty()
    );
    tokio::time::sleep(Duration::from_millis(5)).await;
    let c = w.create("SHOP", "Order confirmation email").await;
    w.board
        .mark_created_in(std::slice::from_ref(&c.id), &terminal.id)
        .await
        .unwrap();
    let keys: Vec<String> = w
        .board
        .created_in_terminal("SHOP")
        .await
        .unwrap()
        .tickets
        .iter()
        .map(|t| t.key.to_string())
        .collect();
    assert_eq!(keys, ["SHOP-4"]);
    w.stop().await;
}

/// The drawer asks for the summary and dismisses it over the socket. Both are answered, and the connection
/// answers later requests too (they used to end the connection's reader, so every later request of the app
/// hung: the Project terminal stayed black).
#[tokio::test(flavor = "multi_thread")]
async fn the_summary_is_answered_over_the_socket() {
    use crewboard_core::client::{Client, ClientConfig};
    use crewboard_core::protocol::{ProjectParams, Reply, Request};

    let w = TestService::start_with(plain_sh).await;
    w.add_project("shop-web").await;
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    // Like the app: subscribed to events, which keeps the connection open.
    let _events = client.subscribe().await.unwrap();
    let project = || ProjectParams {
        project: "SHOP".into(),
    };
    let ask = |request| {
        let client = client.clone();
        async move {
            tokio::time::timeout(Duration::from_secs(10), client.request(request))
                .await
                .expect("an answer in time")
                .expect("a reply")
        }
    };
    assert!(matches!(
        ask(Request::ProjectTerminalCreated(project())).await,
        Reply::CreatedHere(_)
    ));
    assert_eq!(
        ask(Request::ProjectTerminalDismiss(project())).await,
        Reply::Ok
    );
    assert_eq!(ask(Request::Ping).await, Reply::Pong);
}
