//! Crewboard core: domain, SQLite store, scheduler, git and worktrees, sessions, hook handling and the
//! socket server. Runs as `crewboard daemon`. Has no UI dependency, so it is fully testable headless (d0002).

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds() {
        assert_eq!(env!("CARGO_PKG_NAME"), "crewboard-core");
    }
}
