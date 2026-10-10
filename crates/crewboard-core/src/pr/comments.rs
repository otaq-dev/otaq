//! Unresolved PR comments for `crewboard review feedback` (REV-05; d0017), fetched **only while that command runs**
//! with the user's own `gh` or `az` login. Nothing else ever fetches PR data, and PR feedback never re-queues a
//! ticket: the user presses Send back. A missing provider, a logged-out CLI or a failing call adds a one-line note
//! instead of failing the command.
//!
//! - GitHub: `gh api graphql` reads the PR's review threads (`isResolved`, `path`, `line`, comments).
//! - Azure DevOps: `az devops invoke` reads the PR's threads (`status` active or pending = unresolved). `az repos`
//!   has no thread command, so the REST call goes through `az devops invoke` with the same login.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::provider::{Provider, origin_url};
use crate::board::{Board, BoardError};

/// How long `gh` / `az` may take.
const TIMEOUT: Duration = Duration::from_secs(30);

/// One unresolved PR review comment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrComment {
    /// The file it is on; `None` for a comment on the whole PR.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    pub author: String,
    pub text: String,
}

/// The PR part of the feedback.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrFeedback {
    pub number: u32,
    pub comments: Vec<PrComment>,
    /// Why no comments could be fetched: "PR comments weren't fetched: `gh` isn't logged in (`gh auth login`)".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// GitHub's unresolved review threads from `gh api graphql` output.
pub fn parse_github(json: &str) -> Result<Vec<PrComment>, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| format!("unexpected answer: {e}"))?;
    let threads = value
        .pointer("/data/repository/pullRequest/reviewThreads/nodes")
        .and_then(Value::as_array)
        .ok_or("unexpected answer: no review threads")?;
    let mut out = Vec::new();
    for thread in threads {
        if thread["isResolved"].as_bool().unwrap_or(false) {
            continue;
        }
        let file = thread["path"].as_str().map(str::to_owned);
        let line = thread["line"]
            .as_u64()
            .or_else(|| thread["originalLine"].as_u64())
            .and_then(|l| u32::try_from(l).ok());
        for comment in thread["comments"]["nodes"].as_array().into_iter().flatten() {
            out.push(PrComment {
                file: file.clone(),
                line,
                author: comment["author"]["login"]
                    .as_str()
                    .unwrap_or("someone")
                    .to_owned(),
                text: comment["body"]
                    .as_str()
                    .unwrap_or_default()
                    .trim()
                    .to_owned(),
            });
        }
    }
    Ok(out)
}

/// Azure DevOps' unresolved threads from `az devops invoke` output (`{"value": [threads]}`).
pub fn parse_azure(json: &str) -> Result<Vec<PrComment>, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| format!("unexpected answer: {e}"))?;
    let threads = value
        .get("value")
        .and_then(Value::as_array)
        .ok_or("unexpected answer: no threads")?;
    let mut out = Vec::new();
    for thread in threads {
        let status = thread["status"].as_str().unwrap_or_default();
        if thread["isDeleted"].as_bool().unwrap_or(false) || !matches!(status, "active" | "pending")
        {
            continue;
        }
        let context = &thread["threadContext"];
        let file = context["filePath"]
            .as_str()
            .map(|p| p.trim_start_matches('/').to_owned());
        let line = context["rightFileStart"]["line"]
            .as_u64()
            .or_else(|| context["leftFileStart"]["line"].as_u64())
            .and_then(|l| u32::try_from(l).ok());
        for comment in thread["comments"].as_array().into_iter().flatten() {
            // System comments ("updated the PR") aren't review feedback.
            if comment["commentType"].as_str() == Some("system")
                || comment["isDeleted"].as_bool() == Some(true)
            {
                continue;
            }
            out.push(PrComment {
                file: file.clone(),
                line,
                author: comment["author"]["displayName"]
                    .as_str()
                    .unwrap_or("someone")
                    .to_owned(),
                text: comment["content"]
                    .as_str()
                    .unwrap_or_default()
                    .trim()
                    .to_owned(),
            });
        }
    }
    Ok(out)
}

/// `acme/shop-web` from a GitHub remote, or `(org, project, repo)` from an Azure DevOps one.
fn github_repo(url: &str) -> Option<(String, String)> {
    let shown = super::provider::display_remote(url);
    let mut parts = shown.split('/').skip(1);
    Some((parts.next()?.to_owned(), parts.next()?.to_owned()))
}

fn azure_repo(url: &str) -> Option<(String, String, String)> {
    let shown = super::provider::display_remote(url);
    let parts: Vec<&str> = shown.split('/').collect();
    match parts.as_slice() {
        // dev.azure.com/org/project/_git/repo
        [host, org, project, "_git", repo, ..] if host.ends_with("dev.azure.com") => {
            Some((org.to_string(), project.to_string(), repo.to_string()))
        }
        // ssh.dev.azure.com/v3/org/project/repo
        [host, "v3", org, project, repo, ..]
            if host.ends_with("azure.com") || host.ends_with("visualstudio.com") =>
        {
            Some((org.to_string(), project.to_string(), repo.to_string()))
        }
        // org.visualstudio.com/project/_git/repo
        [host, project, "_git", repo, ..] if host.ends_with(".visualstudio.com") => Some((
            host.trim_end_matches(".visualstudio.com").to_owned(),
            project.to_string(),
            repo.to_string(),
        )),
        _ => None,
    }
}

/// Runs a provider CLI found on `path` in `cwd`; its standard output, or a one-line reason.
async fn run(
    program: &str,
    args: &[String],
    cwd: &Path,
    path: &OsString,
) -> Result<String, String> {
    let Some(exe) = crate::tools::find_on_path(program, path) else {
        return Err(format!("`{program}` isn't installed"));
    };
    let mut command = tokio::process::Command::new(exe);
    command
        .args(args)
        .current_dir(cwd)
        .env("PATH", path)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(TIMEOUT, command.output())
        .await
        .map_err(|_| format!("`{program}` took too long"))?
        .map_err(|e| format!("`{program}` didn't run: {e}"))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    if stderr.contains("auth login") || stderr.contains("not logged") || stderr.contains("az login")
    {
        let login = if program == "gh" {
            "gh auth login"
        } else {
            "az login"
        };
        return Err(format!("`{program}` isn't logged in (`{login}`)"));
    }
    let first = String::from_utf8_lossy(&output.stderr)
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("it failed")
        .trim()
        .to_owned();
    Err(format!("`{program}` failed: {first}"))
}

const GITHUB_QUERY: &str = "query($owner: String!, $name: String!, $number: Int!) { repository(owner: $owner, name: $name) { pullRequest(number: $number) { reviewThreads(first: 100) { nodes { isResolved path line originalLine comments(first: 50) { nodes { author { login } body } } } } } } }";

impl Board {
    /// The ticket's unresolved PR comments, if it has a PR; `None` otherwise. `path` is where `gh` / `az` are
    /// looked up. Never fails: problems become the note.
    pub async fn pr_feedback(
        &self,
        key: &str,
        path: &OsString,
    ) -> Result<Option<PrFeedback>, BoardError> {
        let ticket = self.ticket(key).await?;
        let Some(number) = ticket.flags.pr_number else {
            return Ok(None);
        };
        let repo = self.project(ticket.key.project.as_str()).await?.repo_path;
        let repo = Path::new(&repo);
        let url = origin_url(self.git()?, repo).await;
        let not_fetched = |why: String| {
            Ok(Some(PrFeedback {
                number,
                comments: Vec::new(),
                note: Some(format!("PR #{number} comments weren't fetched: {why}")),
            }))
        };
        let Some(url) = url else {
            return not_fetched("the repository has no origin remote".to_owned());
        };
        let fetched = match Provider::detect(&url) {
            None => {
                return not_fetched(
                    "the PR provider isn't detected (only GitHub and Azure DevOps are read)"
                        .to_owned(),
                );
            }
            Some(Provider::GitHub) => {
                let Some((owner, name)) = github_repo(&url) else {
                    return not_fetched(format!("can't read the repository from {url}"));
                };
                let args: Vec<String> = [
                    "api".to_owned(),
                    "graphql".to_owned(),
                    "-f".to_owned(),
                    format!("query={GITHUB_QUERY}"),
                    "-F".to_owned(),
                    format!("owner={owner}"),
                    "-F".to_owned(),
                    format!("name={name}"),
                    "-F".to_owned(),
                    format!("number={number}"),
                ]
                .into();
                run("gh", &args, repo, path)
                    .await
                    .and_then(|out| parse_github(&out))
            }
            Some(Provider::AzureDevOps) => {
                let Some((org, project, name)) = azure_repo(&url) else {
                    return not_fetched(format!("can't read the repository from {url}"));
                };
                let args: Vec<String> = [
                    "devops".to_owned(),
                    "invoke".to_owned(),
                    "--org".to_owned(),
                    format!("https://dev.azure.com/{org}"),
                    "--area".to_owned(),
                    "git".to_owned(),
                    "--resource".to_owned(),
                    "pullRequestThreads".to_owned(),
                    "--route-parameters".to_owned(),
                    format!("project={project}"),
                    format!("repositoryId={name}"),
                    format!("pullRequestId={number}"),
                    "--api-version".to_owned(),
                    "7.0".to_owned(),
                    "--output".to_owned(),
                    "json".to_owned(),
                ]
                .into();
                run("az", &args, repo, path)
                    .await
                    .and_then(|out| parse_azure(&out))
            }
        };
        match fetched {
            Ok(comments) => Ok(Some(PrFeedback {
                number,
                comments,
                note: None,
            })),
            Err(why) => not_fetched(why),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_paths() {
        assert_eq!(
            github_repo("git@github.com:acme/shop-web.git"),
            Some(("acme".into(), "shop-web".into()))
        );
        assert_eq!(
            azure_repo("https://acme@dev.azure.com/acme/billing/_git/billing-api"),
            Some(("acme".into(), "billing".into(), "billing-api".into()))
        );
        assert_eq!(
            azure_repo("git@ssh.dev.azure.com:v3/acme/billing/billing-api"),
            Some(("acme".into(), "billing".into(), "billing-api".into()))
        );
        assert_eq!(
            azure_repo("https://acme.visualstudio.com/billing/_git/billing-api"),
            Some(("acme".into(), "billing".into(), "billing-api".into()))
        );
    }
}
