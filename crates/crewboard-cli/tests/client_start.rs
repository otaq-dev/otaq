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
    let config = ClientConfig::new(data.socket_path());

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
