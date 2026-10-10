//! Ask Claude for the run command (RUN-02; d0026, d0035, d0023): on the user's click only, `claude` runs
//! non-interactively in the project's folder, takes a short read-only look at the repository and suggests a run
//! command, an Open URL and a short why. The user confirms or edits it; nothing is saved without their Save. There
//! are no detection rules.
//!
//! Implementation choices (SDD §26): `claude -p --output-format stream-json --verbose --tools Read,Glob,Grep
//! <prompt>` (only reading tools exist in that conversation); the prompt is the Markdown file
//! `prompts/suggest-run-command.md`; the answer is a JSON object in the final result. Progress ("Read(package.json)")
//! and the outcome arrive as `run_suggestion` events, so the request itself returns at once. One suggestion per
//! project at a time; Cancel kills the process; three minutes at most.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::sync::oneshot;

use crate::board::{Board, BoardError, Refusal};
use crate::protocol::Event;
use crate::service::EventBus;

/// What Claude is asked (a Markdown file, not a Rust string, so it reads like the plugin's skills).
pub const PROMPT: &str = include_str!("../../prompts/suggest-run-command.md");

/// The longest a suggestion may take.
const TIME_LIMIT: Duration = Duration::from_secs(180);

/// Claude's suggestion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    pub command: String,
    #[serde(default)]
    pub open_url: String,
    #[serde(default)]
    pub why: String,
}

/// How a suggestion goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SuggestUpdate {
    /// Claude used a tool: "Read(package.json)".
    Progress {
        line: String,
    },
    Done {
        suggestion: Suggestion,
    },
    /// No usable suggestion; the dialog stays editable.
    Failed {
        message: String,
    },
}

/// Runs Ask Claude. Cheap to clone.
#[derive(Clone)]
pub struct Suggester {
    inner: Arc<Inner>,
}

struct Inner {
    board: Board,
    events: EventBus,
    /// `claude` and any leading arguments; `None`: not available (tests use an ordinary command instead).
    claude: Option<Vec<String>>,
    /// Running suggestions by project key: their number and how to cancel them.
    running: Mutex<HashMap<String, (u64, oneshot::Sender<()>)>>,
    next: AtomicU64,
}

impl Suggester {
    pub fn new(board: Board, events: EventBus, claude: Option<Vec<String>>) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                events,
                claude,
                running: Mutex::new(HashMap::new()),
                next: AtomicU64::new(1),
            }),
        }
    }

    /// Starts a suggestion for the project (the user's click). One already running is replaced.
    pub async fn start(&self, project: &str) -> Result<(), BoardError> {
        let project = self.inner.board.project(project).await?;
        let Some((program, leading)) = self
            .inner
            .claude
            .as_ref()
            .and_then(|c| c.split_first())
            .map(|(p, rest)| (p.clone(), rest.to_vec()))
        else {
            return Err(BoardError::refused(
                Refusal::NotAvailableYet,
                "Claude Code isn't available to the background service",
            ));
        };
        let mut command = tokio::process::Command::new(&program);
        command
            .args(leading)
            .args([
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--tools",
                "Read,Glob,Grep",
                PROMPT,
            ])
            .current_dir(&project.repo_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let child = command.spawn().map_err(|e| {
            BoardError::refused(
                Refusal::NotAvailableYet,
                format!("can't start {program}: {e}"),
            )
        })?;
        let (cancel, cancelled) = oneshot::channel();
        let number = self.inner.next.fetch_add(1, Ordering::SeqCst);
        let replaced = self
            .inner
            .running
            .lock()
            .expect("suggestions")
            .insert(project.key.clone(), (number, cancel));
        drop(replaced);
        tracing::info!(project = %project.key, "asking Claude for the run command");
        tokio::spawn(self.clone().follow(
            project.key.clone(),
            project.repo_path.clone(),
            number,
            child,
            cancelled,
        ));
        Ok(())
    }

    /// Cancel: the `claude` process ends. `true` when one was running.
    pub fn cancel(&self, project: &str) -> bool {
        let running = self
            .inner
            .running
            .lock()
            .expect("suggestions")
            .remove(project);
        running.is_some_and(|(_, cancel)| cancel.send(()).is_ok())
    }

    async fn follow(
        self,
        project: String,
        repo: String,
        number: u64,
        mut child: tokio::process::Child,
        mut cancelled: oneshot::Receiver<()>,
    ) {
        let stdout = child.stdout.take().expect("piped stdout");
        let mut stderr = child.stderr.take().expect("piped stderr");
        // Read alongside stdout, so a chatty stderr never blocks the process.
        let errors = tokio::spawn(async move {
            let mut errors = Vec::new();
            let _ = stderr.read_to_end(&mut errors).await;
            errors
        });
        let mut lines = BufReader::new(stdout).lines();
        let mut outcome: Option<SuggestUpdate> = None;
        let deadline = tokio::time::sleep(TIME_LIMIT);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                _ = &mut cancelled => {
                    let _ = child.kill().await;
                    tracing::info!(project, "run command suggestion cancelled");
                    return;
                }
                _ = &mut deadline => {
                    let _ = child.kill().await;
                    outcome = Some(SuggestUpdate::Failed {
                        message: "Claude took too long. Type the command yourself.".to_owned(),
                    });
                    break;
                }
                line = lines.next_line() => match line {
                    Ok(Some(line)) => match read_line(&line, Path::new(&repo)) {
                        Some(update @ SuggestUpdate::Progress { .. }) => self.publish(&project, number, update),
                        Some(update) => outcome = Some(update),
                        None => {}
                    },
                    Ok(None) | Err(_) => break,
                },
            }
        }
        let status = child.wait().await.ok();
        let errors = errors.await.unwrap_or_default();
        let outcome = outcome.unwrap_or_else(|| {
            let last = String::from_utf8_lossy(&errors)
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .map(|l| format!(": {}", l.trim()))
                .unwrap_or_default();
            SuggestUpdate::Failed {
                message: match status.and_then(|s| s.code()) {
                    Some(code) => {
                        format!("Claude stopped without a suggestion (exit {code}){last}.")
                    }
                    None => format!("Claude stopped without a suggestion{last}."),
                },
            }
        });
        let mine = {
            let mut running = self.inner.running.lock().expect("suggestions");
            let mine = running.get(&project).is_some_and(|(n, _)| *n == number);
            if mine {
                running.remove(&project);
            }
            mine
        };
        if mine {
            self.publish(&project, number, outcome);
        }
    }

    fn publish(&self, project: &str, number: u64, update: SuggestUpdate) {
        let current = self
            .inner
            .running
            .lock()
            .expect("suggestions")
            .get(project)
            .is_none_or(|(n, _)| *n == number);
        if current {
            self.inner.events.publish(Event::RunSuggestion {
                project: project.to_owned(),
                update,
            });
        }
    }
}

/// One line of `--output-format stream-json`: a tool Claude used becomes progress; the final result becomes the
/// suggestion (or a failure). Anything else is ignored.
pub fn read_line(line: &str, repo: &Path) -> Option<SuggestUpdate> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    match value.get("type")?.as_str()? {
        "assistant" => {
            let content = value.get("message")?.get("content")?.as_array()?;
            let tools: Vec<String> = content
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
                .filter_map(|block| tool_line(block, repo))
                .collect();
            (!tools.is_empty()).then(|| SuggestUpdate::Progress {
                line: tools.join(" · "),
            })
        }
        "result" => {
            let text = value.get("result").and_then(Value::as_str).unwrap_or("");
            let is_error = value
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if is_error {
                return Some(SuggestUpdate::Failed {
                    message: if text.trim().is_empty() {
                        "Claude couldn't suggest a command.".to_owned()
                    } else {
                        format!("Claude couldn't suggest a command: {}", text.trim())
                    },
                });
            }
            Some(match parse_suggestion(text) {
                Some(suggestion) => SuggestUpdate::Done { suggestion },
                None => SuggestUpdate::Failed {
                    message: "Claude's answer had no command. Type it yourself, or ask again."
                        .to_owned(),
                },
            })
        }
        _ => None,
    }
}

/// `Read(package.json)`, `Glob(**/*.csproj)`: the tool and what it looked at, relative to the repository.
fn tool_line(block: &Value, repo: &Path) -> Option<String> {
    let name = block.get("name")?.as_str()?;
    let input = block.get("input");
    let field = |f: &str| input.and_then(|i| i.get(f)).and_then(Value::as_str);
    let target = field("file_path")
        .or_else(|| field("pattern"))
        .or_else(|| field("path"))
        .map(|t| {
            Path::new(t)
                .strip_prefix(repo)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| t.to_owned())
        });
    Some(match target {
        Some(t) => format!("{name}({t})"),
        None => name.to_owned(),
    })
}

/// The JSON object in Claude's answer (it may be wrapped in a code fence or a sentence).
pub fn parse_suggestion(text: &str) -> Option<Suggestion> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    let mut suggestion: Suggestion = serde_json::from_str(text.get(start..=end)?).ok()?;
    suggestion.command = suggestion.command.trim().to_owned();
    suggestion.open_url = suggestion.open_url.trim().to_owned();
    suggestion.why = suggestion.why.trim().to_owned();
    (!suggestion.command.is_empty()).then_some(suggestion)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_become_progress_lines() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/r/shop/package.json"}},{"type":"text","text":"x"}]}}"#;
        assert_eq!(
            read_line(line, Path::new("/r/shop")),
            Some(SuggestUpdate::Progress {
                line: "Read(package.json)".into()
            })
        );
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Looking"}]}}"#;
        assert_eq!(read_line(line, Path::new("/r")), None);
        assert_eq!(read_line("not json", Path::new("/r")), None);
        assert_eq!(read_line(r#"{"type":"system"}"#, Path::new("/r")), None);
    }

    #[test]
    fn the_result_is_the_suggestion() {
        let line = r#"{"type":"result","is_error":false,"result":"```json\n{\"command\": \"pnpm dev --port $PORT\", \"open_url\": \"http://localhost:$PORT\", \"why\": \"package.json has dev.\"}\n```"}"#;
        assert_eq!(
            read_line(line, Path::new("/r")),
            Some(SuggestUpdate::Done {
                suggestion: Suggestion {
                    command: "pnpm dev --port $PORT".into(),
                    open_url: "http://localhost:$PORT".into(),
                    why: "package.json has dev.".into(),
                }
            })
        );
        let empty = r#"{"type":"result","result":"{\"command\": \"  \"}"}"#;
        assert!(matches!(
            read_line(empty, Path::new("/r")),
            Some(SuggestUpdate::Failed { .. })
        ));
        let error = r#"{"type":"result","is_error":true,"result":"Credit balance is too low"}"#;
        assert_eq!(
            read_line(error, Path::new("/r")),
            Some(SuggestUpdate::Failed {
                message: "Claude couldn't suggest a command: Credit balance is too low".into()
            })
        );
    }

    #[test]
    fn the_prompt_asks_for_port_and_json() {
        assert!(PROMPT.contains("$PORT"));
        assert!(PROMPT.contains(r#"{"command""#));
    }
}
