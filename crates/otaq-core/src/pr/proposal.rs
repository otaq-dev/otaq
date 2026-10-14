//! The Open PR / Push skill's proposal (REV-10, d0050): `otaq pr propose --stdin` takes one JSON object,
//!
//! ```json
//! {
//!   "branch": "otaq/SHOP-18-login-redirect",
//!   "commits": [{"sha": "a1b2c3d", "message": "SHOP-18: return to the page you came from"}],
//!   "title": "SHOP-18 Login redirect",
//!   "description": "Markdown…"
//! }
//! ```
//!
//! `commits` lists every commit that goes into the PR (Push to PR: every commit not on the remote yet), oldest
//! first; `sha` may be short. `title` and `description` may be left out for Push to PR. The format is documented in
//! the open-pr and push skills. Also: branch name rules and PR URLs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    pub branch: String,
    pub commits: Vec<CommitMessage>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitMessage {
    pub sha: String,
    pub message: String,
}

/// Reads a proposal and checks its shape: a valid branch name, at least one commit, each with a hex `sha` and a
/// message. The answer to a malformed one says what to fix.
pub fn parse(text: &str) -> Result<Proposal, String> {
    let mut proposal: Proposal = serde_json::from_str(text.trim()).map_err(|e| {
        format!(
            "the proposal isn't the JSON object the skill documents ({e}); expected {{\"branch\", \"commits\": [{{\"sha\", \"message\"}}], \"title\", \"description\"}}"
        )
    })?;
    proposal.branch = proposal.branch.trim().to_owned();
    proposal.title = proposal.title.trim().to_owned();
    proposal.description = proposal.description.trim().to_owned();
    if let Some(problem) = branch_name_problem(&proposal.branch) {
        return Err(problem);
    }
    if proposal.commits.is_empty() {
        return Err("the proposal lists no commits".to_owned());
    }
    for (n, commit) in proposal.commits.iter_mut().enumerate() {
        commit.sha = commit.sha.trim().to_ascii_lowercase();
        commit.message = commit.message.trim().to_owned();
        if commit.sha.len() < 7 || !commit.sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!(
                "commit {} has no commit ID: \"sha\" must be at least 7 hex characters",
                n + 1
            ));
        }
        if commit.message.is_empty() {
            return Err(format!("commit {} ({}) has no message", n + 1, commit.sha));
        }
    }
    Ok(proposal)
}

/// Why a branch name isn't usable, if it isn't (git's `check-ref-format --branch` rules, kept simple).
pub fn branch_name_problem(name: &str) -> Option<String> {
    let bad = |why: &str| Some(format!("{name:?} isn't a valid branch name: {why}"));
    if name.is_empty() {
        return Some("The branch name is empty".to_owned());
    }
    if name.starts_with('-') {
        return bad("it can't start with -");
    }
    if name == "@" || name.contains("@{") {
        return bad("it can't contain @{");
    }
    if let Some(c) = name.chars().find(|c| {
        c.is_ascii_control() || matches!(c, ' ' | '~' | '^' | ':' | '?' | '*' | '[' | '\\')
    }) {
        return bad(&format!("it can't contain {c:?}"));
    }
    if name.contains("..") {
        return bad("it can't contain ..");
    }
    if name.ends_with('.') {
        return bad("it can't end with .");
    }
    for part in name.split('/') {
        if part.is_empty() {
            return bad("it can't start or end with / or contain //");
        }
        if part.starts_with('.') {
            return bad("no part can start with .");
        }
        if part.ends_with(".lock") {
            return bad("no part can end with .lock");
        }
    }
    None
}

/// The PR number in a GitHub (`…/pull/481`) or Azure DevOps (`…/pullrequest/481`) URL.
pub fn pr_number(url: &str) -> Option<u32> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return None;
    }
    let path = url.split(['?', '#']).next()?;
    let mut parts = path.trim_end_matches('/').rsplit('/');
    let number = parts.next()?.parse().ok()?;
    match parts.next()? {
        "pull" | "pulls" | "pullrequest" | "merge_requests" => Some(number),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_checks_the_shape() {
        let p = parse(
            r#"{"branch": " bugfix/SHOP-18-login ", "commits": [{"sha": "A1B2C3D", "message": " SHOP-18: x "}],
                "title": "SHOP-18 Login", "description": "Why.\n\nHow."}"#,
        )
        .unwrap();
        assert_eq!(p.branch, "bugfix/SHOP-18-login");
        assert_eq!(p.commits[0].sha, "a1b2c3d");
        assert_eq!(p.commits[0].message, "SHOP-18: x");
        // Push to PR leaves title and description out.
        let p =
            parse(r#"{"branch": "b", "commits": [{"sha": "abcdef0", "message": "m"}]}"#).unwrap();
        assert_eq!(p.title, "");
        for (bad, says) in [
            ("branch: x", "isn't the JSON object"),
            (r#"{"commits": []}"#, "isn't the JSON object"),
            (
                r#"{"branch": "a b", "commits": []}"#,
                "isn't a valid branch name",
            ),
            (r#"{"branch": "b", "commits": []}"#, "lists no commits"),
            (
                r#"{"branch": "b", "commits": [{"sha": "xyz", "message": "m"}]}"#,
                "commit 1 has no commit ID",
            ),
            (
                r#"{"branch": "b", "commits": [{"sha": "abcdef0", "message": " "}]}"#,
                "commit 1 (abcdef0) has no message",
            ),
        ] {
            let err = parse(bad).unwrap_err();
            assert!(err.contains(says), "{bad}: {err}");
        }
    }

    #[test]
    fn branch_names() {
        for good in [
            "team/SHOP-18-login",
            "feature/PAY-123-wishlist",
            "a.b",
            "x@y",
        ] {
            assert_eq!(branch_name_problem(good), None, "{good}");
        }
        for bad in [
            "", "-x", "a b", "a..b", "a/", "/a", "a//b", "a.lock", "a/.b", "a.", "a~1", "a^",
            "a:b", "a?", "a*", "a[", "a\\b", "@", "a@{1}",
        ] {
            assert!(branch_name_problem(bad).is_some(), "{bad:?}");
        }
    }

    #[test]
    fn pr_numbers() {
        assert_eq!(
            pr_number("https://github.com/acme/shop-web/pull/481"),
            Some(481)
        );
        assert_eq!(
            pr_number("https://github.com/acme/shop-web/pull/481/files"),
            None
        );
        assert_eq!(
            pr_number("https://github.com/acme/shop-web/pull/481/"),
            Some(481)
        );
        assert_eq!(
            pr_number(
                "https://dev.azure.com/acme/billing/_git/billing-api/pullrequest/90?_a=overview"
            ),
            Some(90)
        );
        assert_eq!(
            pr_number("https://acme.visualstudio.com/shop/_git/web/pullrequest/7"),
            Some(7)
        );
        assert_eq!(pr_number("https://github.com/acme/shop-web"), None);
        assert_eq!(pr_number("github.com/acme/shop-web/pull/481"), None);
    }
}
