//! Several Project terminals per project (T-0198, RUN-06, CLI-03; d0068, d0019, d0022): each open with `new` starts
//! another plain shell in the project folder, "Terminal 1", "Terminal 2"…; closing one ends only its shell; they
//! belong to the service (a new client still lists them) and are gone after a service restart. `sh` stands in for
//! the user's login shell.

use std::time::Duration;

use otaq_core::client::{Client, ClientConfig};
use otaq_core::protocol::{ProjectParams, ProjectTerminalParams, Reply, Request, SessionIdParams};
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;
use otaq_core::session::SessionKind;
use otaq_core::terminals::shell::ProjectTerminal;

fn plain_sh(mut c: ServiceConfig) -> ServiceConfig {
    c.terminals.shell = "sh".into();
    c.terminals.shell_args = Vec::new();
    c
}

async fn open(client: &Client, project: &str, new: bool) -> ProjectTerminal {
    let reply = client
        .request(Request::ProjectTerminalOpen(ProjectTerminalParams {
            project: project.into(),
            new,
        }))
        .await
        .unwrap();
    let Reply::ProjectTerminal(terminal) = reply else {
        panic!("{reply:?}")
    };
    *terminal
}

async fn list(client: &Client, project: &str) -> Vec<(String, bool)> {
    let reply = client
        .request(Request::ProjectTerminalList(ProjectParams {
            project: project.into(),
        }))
        .await
        .unwrap();
    let Reply::ProjectTerminals(terminals) = reply else {
        panic!("{reply:?}")
    };
    terminals
        .into_iter()
        .map(|t| (t.name, t.session.is_running()))
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn several_terminals_open_list_close_and_are_named_by_the_next_free_number() {
    let w = TestService::start_with(plain_sh).await;
    w.add_project("shop-web").await;
    w.add_project("billing-api").await;
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();

    // Without `new`, opening starts the first one, then shows it again.
    let first = open(&client, "SHOP", false).await;
    assert_eq!((first.number, first.name.as_str()), (1, "Terminal 1"));
    assert_eq!(first.session.kind, SessionKind::Project);
    assert!(first.session.is_running());
    assert_eq!(
        open(&client, "SHOP", false).await.session.id,
        first.session.id
    );

    // `new` starts another shell; numbers are per project.
    let second = open(&client, "SHOP", true).await;
    assert_eq!(second.name, "Terminal 2");
    assert_ne!(second.session.id, first.session.id);
    assert_eq!(open(&client, "BILL", false).await.name, "Terminal 1");
    assert_eq!(
        list(&client, "SHOP").await,
        vec![
            ("Terminal 1".to_owned(), true),
            ("Terminal 2".to_owned(), true)
        ]
    );
    // Without `new`, the newest running one.
    assert_eq!(
        open(&client, "SHOP", false).await.session.id,
        second.session.id
    );

    // Closing one ends only its shell; its number is free again.
    let reply = client
        .request(Request::ProjectTerminalClose(SessionIdParams {
            session: first.session.id.clone(),
        }))
        .await
        .unwrap();
    assert_eq!(reply, Reply::Ok);
    let ended = tokio::time::timeout(
        Duration::from_secs(10),
        w.sessions.wait_ended(&first.session.id),
    )
    .await
    .unwrap();
    assert!(ended.is_some_and(|s| !s.is_running()));
    assert!(w.sessions.info(&second.session.id).unwrap().is_running());
    assert_eq!(
        list(&client, "SHOP").await,
        vec![("Terminal 2".to_owned(), true)]
    );
    let third = open(&client, "SHOP", true).await;
    assert_eq!(third.name, "Terminal 1");
    assert_eq!(
        list(&client, "SHOP").await,
        vec![
            ("Terminal 1".to_owned(), true),
            ("Terminal 2".to_owned(), true)
        ]
    );

    // Closing an unknown or already closed terminal is "not found".
    assert!(
        client
            .request(Request::ProjectTerminalClose(SessionIdParams {
                session: first.session.id.clone(),
            }))
            .await
            .is_err()
    );

    // A shell the user exits stays listed (ended, its output kept) until closed.
    w.sessions.write(&second.session.id, b"exit\n").unwrap();
    tokio::time::timeout(
        Duration::from_secs(10),
        w.sessions.wait_ended(&second.session.id),
    )
    .await
    .unwrap();
    assert_eq!(
        list(&client, "SHOP").await,
        vec![
            ("Terminal 1".to_owned(), true),
            ("Terminal 2".to_owned(), false)
        ]
    );
    w.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn terminals_outlive_the_window_and_are_gone_after_a_service_restart() {
    let w = TestService::start_with(plain_sh).await;
    w.add_project("shop-web").await;
    let window = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let one = open(&window, "SHOP", false).await;
    open(&window, "SHOP", true).await;
    // The window closes; a new one still finds both running.
    drop(window);
    let again = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    assert_eq!(
        list(&again, "SHOP").await,
        vec![
            ("Terminal 1".to_owned(), true),
            ("Terminal 2".to_owned(), true)
        ]
    );
    drop(again);

    let w = w.restart_with(plain_sh).await;
    let client = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    assert_eq!(list(&client, "SHOP").await, Vec::new());
    // Opening after the restart starts a fresh "Terminal 1", not the old one.
    let fresh = open(&client, "SHOP", false).await;
    assert_eq!(fresh.name, "Terminal 1");
    assert_ne!(fresh.session.id, one.session.id);
    assert!(fresh.session.is_running());
    w.stop().await;
}
