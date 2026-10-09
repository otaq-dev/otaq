//! The client starts the real service through `crewboard daemon start` when none runs, and never a second one.
//! Uses a temporary data folder; the service is stopped at the end even if the test fails.

use std::process::Command;

use crewboard_core::client::{Client, ClientConfig, CommandLauncher};
use crewboard_core::data_dir::DataDir;
use crewboard_core::protocol::{Reply, Request};

struct StopOnDrop(std::path::PathBuf);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        let _ = Command::new(env!("CARGO_BIN_EXE_crewboard"))
            .args(["daemon", "stop"])
            .env(DataDir::env_var(), &self.0)
            .output();
    }
}

#[tokio::test]
async fn connect_or_start_launches_the_daemon_once() {
    let tmp = tempfile::tempdir().unwrap();
    let _guard = StopOnDrop(tmp.path().to_owned());
    let data = DataDir::new(tmp.path());
    let launcher = CommandLauncher::new(env!("CARGO_BIN_EXE_crewboard"))
        .with_env(DataDir::env_var(), tmp.path());
    let config = ClientConfig::new(&data);

    let client = Client::connect_or_start(config.clone(), &launcher)
        .await
        .unwrap();
    let Reply::ServiceStatus(first) = client.request(Request::ServiceStatus).await.unwrap() else {
        panic!()
    };
    assert_ne!(first.pid, std::process::id(), "a separate service process");

    // A second client (like a second app window or launch) finds the same service.
    let second = Client::connect_or_start(config, &launcher).await.unwrap();
    let Reply::ServiceStatus(again) = second.request(Request::ServiceStatus).await.unwrap() else {
        panic!()
    };
    assert_eq!(again.pid, first.pid);

    // Dropping the clients (closing the app) leaves the service running.
    drop((client, second));
    let out = Command::new(env!("CARGO_BIN_EXE_crewboard"))
        .args(["daemon", "status", "--json"])
        .env(DataDir::env_var(), tmp.path())
        .output()
        .unwrap();
    let status: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(status["running"], true);
    assert_eq!(status["pid"], first.pid);
}

#[tokio::test]
async fn restart_replaces_the_service_process() {
    let tmp = tempfile::tempdir().unwrap();
    let _guard = StopOnDrop(tmp.path().to_owned());
    let data = DataDir::new(tmp.path());
    let launcher = CommandLauncher::new(env!("CARGO_BIN_EXE_crewboard"))
        .with_env(DataDir::env_var(), tmp.path());
    let mut config = ClientConfig::new(&data);
    // Pretend this program was rebuilt after the service started.
    config.build_id = "rebuilt.1".into();

    let client = Client::connect_or_start(config, &launcher).await.unwrap();
    assert_eq!(
        client.compatibility(),
        Some(crewboard_core::client::Compatibility::ServiceOlder)
    );
    let Reply::ServiceStatus(old) = client.request(Request::ServiceStatus).await.unwrap() else {
        panic!()
    };

    let new = client.restart(&launcher).await.unwrap();
    assert_ne!(new.status.pid, old.pid, "a new service process");
    let alive = || {
        Command::new("kill")
            .args(["-0", &old.pid.to_string()])
            .output()
            .unwrap()
            .status
            .success()
    };
    for _ in 0..100 {
        if !alive() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!alive(), "the old service process ended");
    assert_eq!(client.request(Request::Ping).await.unwrap(), Reply::Pong);
}
