//! Helpers for the ticket resource tests (M19-d, d0087): project commands that write markers, on `sh` and on
//! Windows' `cmd` (Q33), and a ticket with a recorded resource.

use std::path::{Path, PathBuf};
use std::time::Duration;

use otaq_core::board::tickets::Ticket;
use otaq_core::domain::ResourceState;
use otaq_core::service::testing::TestService;

/// `unix` on macOS and Linux, `windows` on Windows, where project commands run with `cmd`.
pub fn by_os(unix: &str, windows: &str) -> String {
    if cfg!(windows) { windows } else { unix }.to_owned()
}

/// `$NAME` in `sh`, `%NAME%` in `cmd`.
pub fn var(name: &str) -> String {
    by_os(&format!("${name}"), &format!("%{name}%"))
}

/// A command appending the line `text` (variables expanded) to `file`.
pub fn append(file: &Path, text: &str) -> String {
    by_os(
        &format!("echo \"{text}\" >> '{}'", file.display()),
        &format!("echo {text}>> \"{}\"", file.display()),
    )
}

/// Commands run one after the other.
pub fn then(commands: &[String]) -> String {
    commands.join(if cfg!(windows) { " & " } else { "; " })
}

/// A command that waits about `secs` seconds.
pub fn sleep(secs: u32) -> String {
    by_os(
        &format!("sleep {secs}"),
        &format!("ping -n {} 127.0.0.1 >nul", secs + 1),
    )
}

/// A file's lines, trimmed; none when it doesn't exist.
pub fn lines(file: &Path) -> Vec<String> {
    std::fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Records a created resource `name` for `ticket`, as its Create would have, and makes its resource folder with a
/// file in it. Returns the folder.
pub async fn created(w: &TestService, ticket: &Ticket, name: &str) -> PathBuf {
    w.board
        .store()
        .set_resource(&ticket.id, Some(name), ResourceState::Created, None)
        .await
        .unwrap();
    let dir = w.data.resource_path("shop-web", &ticket.key).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("db.sqlite"), "data").unwrap();
    dir
}

/// The ticket's resource columns: name, state and problem.
pub async fn resource_row(
    w: &TestService,
    ticket: &Ticket,
) -> (Option<String>, String, Option<String>) {
    sqlx::query_as(
        "SELECT resource_name, resource_state, resource_problem FROM tickets WHERE id = ?",
    )
    .bind(&ticket.id)
    .fetch_one(w.board.store().pool())
    .await
    .unwrap()
}

/// Waits until `file` exists (30 s at most).
pub async fn appears(file: &Path) {
    tokio::time::timeout(Duration::from_secs(30), async {
        while !file.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{} never appeared", file.display()));
}
