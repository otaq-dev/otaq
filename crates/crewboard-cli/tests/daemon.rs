//! `crewboard daemon start | stop | status` against a temporary data folder. A guard stops the service even
//! when a test fails, so no background process is left behind.

use std::path::Path;
use std::process::{Command, Output};

use crewboard_core::data_dir::DataDir;
use serde_json::Value;

struct Home {
    tmp: tempfile::TempDir,
}

impl Home {
    fn new() -> Self {
        Self {
            tmp: tempfile::tempdir().unwrap(),
        }
    }

    fn data(&self) -> DataDir {
        DataDir::new(self.tmp.path())
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_crewboard"))
            .args(args)
            .env(DataDir::env_var(), self.tmp.path())
            .output()
            .unwrap()
    }

    fn json(&self, args: &[&str]) -> Value {
        let out = self.run(args);
        assert!(out.status.success(), "{args:?}: {out:?}");
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {out:?}"))
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = self.run(&["daemon", "stop"]);
    }
}

fn pid_alive(pid: u64) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .unwrap()
        .status
        .success()
}

#[test]
fn start_status_stop() {
    let home = Home::new();
    assert_eq!(
        home.json(&["daemon", "status", "--json"]),
        serde_json::json!({"running": false})
    );

    let started = home.json(&["daemon", "start", "--json"]);
    assert_eq!(started["running"], true);
    assert_eq!(started["already_running"], false);
    let pid = started["pid"].as_u64().unwrap();
    assert!(pid_alive(pid));

    let status = home.json(&["daemon", "status", "--json"]);
    assert_eq!(status["running"], true);
    assert_eq!(status["pid"], pid);
    assert_eq!(status["build_id"], crewboard_core::protocol::build_id());
    assert_eq!(
        status["protocol_version"],
        crewboard_core::protocol::PROTOCOL_VERSION
    );
    assert!(status["uptime_secs"].is_u64());

    let human = home.run(&["daemon", "status"]);
    let text = String::from_utf8_lossy(&human.stdout);
    assert!(
        text.contains("running") && text.contains(&format!("PID: {pid}")),
        "{text}"
    );

    // A second start finds the running service and exits 0.
    let again = home.json(&["daemon", "start", "--json"]);
    assert_eq!(again["already_running"], true);
    assert_eq!(again["pid"], pid);

    let stopped = home.json(&["daemon", "stop", "--json"]);
    assert_eq!(stopped["stopped"], true);
    let data = home.data();
    assert!(!data.socket_path().exists());
    assert!(!data.service_lock_path().exists());
    assert_eq!(home.json(&["daemon", "status", "--json"])["running"], false);
    wait_for_exit(pid);

    // Stopping again is fine.
    assert_eq!(home.json(&["daemon", "stop", "--json"])["stopped"], false);
}

fn wait_for_exit(pid: u64) {
    for _ in 0..100 {
        if !pid_alive(pid) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("service process {pid} still running");
}

#[test]
fn start_replaces_a_stale_socket_and_lock() {
    let home = Home::new();
    let data = home.data();
    drop(std::os::unix::net::UnixListener::bind(data.socket_path()).unwrap());
    std::fs::write(data.service_lock_path(), "999999\n").unwrap();
    assert!(Path::new(&data.socket_path()).exists());

    let started = home.json(&["daemon", "start", "--json"]);
    assert_eq!(started["already_running"], false);
    assert_eq!(home.json(&["daemon", "status", "--json"])["running"], true);
}

#[test]
fn silent_prints_only_the_pid() {
    let home = Home::new();
    let out = home.run(&["daemon", "start", "--silent"]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.trim().parse::<u32>().is_ok(), "{text:?}");
}
