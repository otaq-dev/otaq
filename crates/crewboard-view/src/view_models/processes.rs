//! The header's running processes (RUN-04; *13 · Run & Shell → All running processes · header menu*): "▶ 3
//! processes" across every project, shown only while something runs ("3 running" stays the agents'), and a menu
//! with one row per Run (project, key, title, port, uptime) to open its ticket's Run tab or stop it.

use crewboard_core::domain::TicketKey;
use crewboard_core::terminals::run::RunInfo;

/// The header chip: `None` while nothing runs.
pub fn chip_label(runs: &[RunInfo]) -> Option<String> {
    match runs.len() {
        0 => None,
        1 => Some("1 process".to_owned()),
        n => Some(format!("{n} processes")),
    }
}

/// One row of the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRow {
    pub project: String,
    pub project_name: String,
    pub key: TicketKey,
    pub title: String,
    /// ":4020".
    pub port: Option<String>,
    /// "12 min", "2 h 5 min".
    pub uptime: Option<String>,
    /// The substituted Open URL, when the project has one.
    pub url: Option<String>,
}

/// The menu's rows: grouped by project (by name), then by key.
pub fn rows(runs: &[RunInfo], now_ms: i64) -> Vec<ProcessRow> {
    let mut rows: Vec<ProcessRow> = runs
        .iter()
        .filter(|r| r.state.is_live())
        .map(|r| ProcessRow {
            project: r.project.clone(),
            project_name: r.project_name.clone(),
            key: r.key.clone(),
            title: r.title.clone(),
            port: r.port.map(|p| format!(":{p}")),
            uptime: r.started_at.map(|at| super::clock::duration(now_ms - at)),
            url: r.url(),
        })
        .collect();
    rows.sort_by(|a, b| a.project_name.cmp(&b.project_name).then(a.key.cmp(&b.key)));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crewboard_core::terminals::run::RunState;

    fn run(project: &str, name: &str, key: &str, port: u16, url: Option<&str>) -> RunInfo {
        RunInfo {
            project: project.into(),
            project_name: name.into(),
            key: key.parse().unwrap(),
            title: format!("{key} title"),
            state: RunState::Running,
            port: Some(port),
            command: "x".into(),
            open_url: url.map(str::to_owned),
            session: None,
            started_at: Some(0),
            ended_at: None,
            exit_code: None,
            stopped_by: None,
        }
    }

    #[test]
    fn counts_and_grouping() {
        assert_eq!(chip_label(&[]), None);
        let runs = vec![
            run("BILL", "billing-api", "BILL-4", 4004, None),
            run(
                "SHOP",
                "shop-web",
                "SHOP-22",
                4024,
                Some("http://localhost:$PORT"),
            ),
            run(
                "SHOP",
                "shop-web",
                "SHOP-19",
                4020,
                Some("http://localhost:$PORT"),
            ),
        ];
        assert_eq!(chip_label(&runs[..1]).as_deref(), Some("1 process"));
        assert_eq!(chip_label(&runs).as_deref(), Some("3 processes"));
        let rows = rows(&runs, 12 * 60_000);
        let keys: Vec<String> = rows.iter().map(|r| r.key.to_string()).collect();
        assert_eq!(keys, ["BILL-4", "SHOP-19", "SHOP-22"]);
        assert_eq!(rows[0].url, None, "billing-api has no Open URL");
        assert_eq!(rows[1].url.as_deref(), Some("http://localhost:4020"));
        assert_eq!(rows[1].port.as_deref(), Some(":4020"));
        assert_eq!(rows[1].uptime.as_deref(), Some("12 min"));
    }

    #[test]
    fn stopped_runs_are_left_out() {
        let mut stopped = run("SHOP", "shop-web", "SHOP-19", 4020, None);
        stopped.state = RunState::Stopped;
        assert!(rows(&[stopped], 0).is_empty());
    }
}
