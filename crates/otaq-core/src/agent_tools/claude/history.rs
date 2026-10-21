//! Claude Code's history (ONB-01, d0108): `<config dir>/history.jsonl`, one JSON object per thing the user typed,
//! slash commands included. Read line by line, never whole; only `display` (the text), `project` (the folder) and
//! `timestamp` (Unix milliseconds) are read. `pastedContents`, the text the user pasted, is skipped unread, and so
//! are the session transcripts in `projects/`.

use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::transcript::iso_ms;
use crate::agent_tools::history::{HistoryEntry, HistoryLine, HistorySource, PASTE_LEFT_OUT};

/// The history file in Claude Code's folder.
const FILE: &str = "history.jsonl";

/// The fields read from a line; every other field (`pastedContents`, `sessionId`) is skipped by the parser.
#[derive(Deserialize)]
struct Line {
    display: String,
    #[serde(default)]
    project: Option<String>,
    timestamp: Value,
}

/// The history in the Claude Code folder `dir`.
pub(super) fn read(dir: &Path) -> HistorySource {
    let path = dir.join(FILE);
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::NotFound => return HistorySource::Missing,
        Err(e) => return HistorySource::Unopenable(format!("can't read {}: {e}", path.display())),
    };
    if file.metadata().is_ok_and(|m| m.len() == 0) {
        return HistorySource::Missing;
    }
    let mut lines = Some(BufReader::new(file).split(b'\n'));
    HistorySource::Lines(Box::new(std::iter::from_fn(move || {
        loop {
            match lines.as_mut()?.next()? {
                // A read error mid-file ends the reading, counted as one line.
                Err(_) => {
                    lines = None;
                    return Some(HistoryLine::Unreadable);
                }
                Ok(line) if line.iter().all(u8::is_ascii_whitespace) => continue,
                Ok(line) => return Some(entry_of(&line)),
            }
        }
    })))
}

fn entry_of(line: &[u8]) -> HistoryLine {
    let Ok(line) = serde_json::from_slice::<Line>(line) else {
        return HistoryLine::Unreadable;
    };
    let at = match &line.timestamp {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => iso_ms(s).or_else(|| s.parse().ok()),
        _ => None,
    };
    let Some(at) = at else {
        return HistoryLine::Unreadable;
    };
    let text = if only_pastes(&line.display) {
        PASTE_LEFT_OUT.to_owned()
    } else {
        line.display
    };
    HistoryLine::Entry(HistoryEntry {
        at,
        folder: line.project.filter(|p| !p.is_empty()),
        text,
    })
}

/// Whether `display` is nothing but paste placeholders: `[Pasted text #1 +12 lines]`, several of them.
fn only_pastes(display: &str) -> bool {
    let mut rest = display.trim();
    if rest.is_empty() {
        return false;
    }
    while !rest.is_empty() {
        let Some(after) = rest.strip_prefix("[Pasted text #") else {
            return false;
        };
        let Some(end) = after.find(']') else {
            return false;
        };
        if !after.starts_with(|c: char| c.is_ascii_digit()) {
            return false;
        }
        rest = after[end + 1..].trim_start();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_tools::history::{
        self, HistoryPage, HistoryParams, KnownProject, Since, only_a_project_agent,
    };

    /// The made-up history the tests share (T-0426): 300 entries from 2025-08-26 to 2026-10-20 in pocket-notes,
    /// pocket-api and scratch, three pastes and one broken line. Invented; never a real one.
    const FIXTURE: &str = include_str!("../../../tests/fixtures/claude-history.jsonl");

    /// 2026-10-21 12:00 UTC, the fixture's "now".
    const NOW: i64 = 1_792_584_000_000;

    /// The markers inside the fixture's `pastedContents`: none may ever be printed.
    const PASTE_MARKERS: &[&str] = &[
        "PASTE-MARKER-QUOKKA-1",
        "PASTE-MARKER-NARWHAL-2",
        "PASTE-MARKER-AXOLOTL-3",
        "SAM_TOKEN",
        "Traceback",
    ];

    fn folder(history: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), history).unwrap();
        dir
    }

    fn page(dir: &Path, params: &HistoryParams) -> HistoryPage {
        history::read(vec![("Claude Code", read(dir))], params, &[], NOW).unwrap()
    }

    fn all() -> HistoryParams {
        HistoryParams {
            since: Since::All,
            ..HistoryParams::default()
        }
    }

    #[test]
    fn the_fixture_has_what_the_tests_say() {
        assert_eq!(FIXTURE.lines().count(), 301);
        assert_eq!(
            FIXTURE.matches("\"display\":\"/security-review\"").count(),
            12
        );
        assert_eq!(FIXTURE.matches("\"display\":\"/commit\"").count(), 40);
        for marker in &PASTE_MARKERS[..3] {
            assert!(FIXTURE.contains(marker), "{marker}");
        }
    }

    #[test]
    fn totals_of_the_last_12_months_by_default() {
        let dir = folder(FIXTURE);
        let page = page(dir.path(), &HistoryParams::default());
        assert!(page.found);
        assert_eq!((page.messages, page.projects, page.skipped), (266, 3, 1));
        assert_eq!((page.page, page.pages), (1, 1));
        assert_eq!(page.entries.len(), 266);
        let text = history::text(&page);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "266 of your messages in 3 projects \u{b7} last 12 months"
        );
        assert_eq!(*lines.last().unwrap(), "page 1 of 1 \u{b7} 1 line skipped");
        // Oldest first, every one within the 12 months.
        assert!(page.entries.windows(2).all(|w| w[0].at <= w[1].at));
        assert!(page.entries[0].at >= Since::Months(12).start(NOW).unwrap());
    }

    #[test]
    fn all_of_it_and_shorter_periods() {
        let dir = folder(FIXTURE);
        let every = page(dir.path(), &all());
        assert_eq!((every.messages, every.projects), (300, 3));
        assert!(
            history::text(&every)
                .starts_with("300 of your messages in 3 projects \u{b7} all time\n")
        );
        let month = page(
            dir.path(),
            &HistoryParams {
                since: Since::Days(30),
                ..HistoryParams::default()
            },
        );
        assert_eq!(month.messages, 28);
        let year = page(
            dir.path(),
            &HistoryParams {
                since: Since::Years(1),
                ..HistoryParams::default()
            },
        );
        assert_eq!(year.messages, 266);
    }

    #[test]
    fn filters_by_project_and_command() {
        let dir = folder(FIXTURE);
        let counts = |project: Option<&str>, command: Option<&str>, since: Since| {
            page(
                dir.path(),
                &HistoryParams {
                    since,
                    project: project.map(str::to_owned),
                    command: command.map(str::to_owned),
                    page: 1,
                },
            )
            .messages
        };
        assert_eq!(counts(Some("pocket-notes"), None, Since::All), 142);
        assert_eq!(counts(Some("pocket-api"), None, Since::default()), 91);
        assert_eq!(counts(Some("scratch"), None, Since::default()), 50);
        assert_eq!(counts(Some("nowhere"), None, Since::All), 0);
        assert_eq!(counts(None, Some("/security-review"), Since::All), 12);
        assert_eq!(counts(None, Some("/security-review"), Since::default()), 8);
        assert_eq!(counts(None, Some("commit"), Since::default()), 36);
        assert_eq!(counts(Some("pocket-api"), Some("/commit"), Since::All), 9);

        let only = page(
            dir.path(),
            &HistoryParams {
                command: Some("/security-review".into()),
                ..HistoryParams::default()
            },
        );
        let text = history::text(&only);
        assert!(text.starts_with("8 of your messages in "), "{text}");
        for line in text.lines().skip(1).filter(|l| !l.starts_with("page ")) {
            assert!(line.ends_with(" \u{b7} /security-review"), "{line}");
        }
    }

    #[test]
    fn pages_of_500_with_the_next_command() {
        // The fixture three times over: 900 entries, two pages.
        let three = FIXTURE.repeat(3);
        let dir = folder(&three);
        let first = page(dir.path(), &all());
        assert_eq!(
            (first.messages, first.pages, first.entries.len()),
            (900, 2, 500)
        );
        assert_eq!(first.skipped, 3);
        let text = history::text(&first);
        assert!(
            text.starts_with("900 of your messages in 3 projects"),
            "{text}"
        );
        assert_eq!(
            text.lines().last().unwrap(),
            format!(
                "page 1 of 2 \u{b7} next: `{} history --all --page 2` \u{b7} 3 lines skipped",
                crate::naming::COMMAND_NAME
            )
        );
        let second = page(dir.path(), &HistoryParams { page: 2, ..all() });
        assert_eq!(second.entries.len(), 400);
        assert_eq!(
            history::text(&second).lines().last().unwrap(),
            "page 2 of 2 \u{b7} 3 lines skipped"
        );
        let beyond = page(dir.path(), &HistoryParams { page: 3, ..all() });
        assert!(beyond.entries.is_empty());
        assert!(
            history::text(&beyond)
                .lines()
                .last()
                .unwrap()
                .starts_with("page 3 of 2 \u{b7} the last is `")
        );
    }

    #[test]
    fn pasted_text_is_left_out() {
        let dir = folder(FIXTURE);
        let every = page(dir.path(), &all());
        let text = history::text(&every);
        let json = serde_json::to_string(&every).unwrap();
        for marker in PASTE_MARKERS {
            assert!(!text.contains(marker), "{marker} printed");
            assert!(!json.contains(marker), "{marker} in --json");
        }
        // The paste-only entry prints the placeholder; the others keep their own words.
        let left_out: Vec<_> = every
            .entries
            .iter()
            .filter(|e| e.text == PASTE_LEFT_OUT)
            .collect();
        assert_eq!(left_out.len(), 1);
        assert!(text.contains(&format!("\u{b7} {PASTE_LEFT_OUT}\n")));
        assert!(text.contains("look at this stack trace [Pasted text #1 +12 lines] and tell me"));
        assert!(text.contains(
            "compare these two configs [Pasted text #1 +5 lines] [Pasted text #2 +4 lines]"
        ));
        assert!(only_pastes(
            "[Pasted text #1 +8 lines] [Pasted text #2 +1 lines]"
        ));
        assert!(!only_pastes("see [Pasted text #1 +8 lines]"));
        assert!(!only_pastes("[Pasted text #x]"));
    }

    #[test]
    fn multi_line_entries_are_indented() {
        let dir = folder(FIXTURE);
        let text = history::text(&page(dir.path(), &all()));
        let plan = text
            .lines()
            .position(|l| l.ends_with(" \u{b7} plan for the offline mode:"))
            .expect("the multi-line plan");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[plan + 1], "    1. cache the notes locally");
        assert_eq!(lines[plan + 3], "    3. replay them on reconnect");
    }

    #[test]
    fn broken_lines_are_skipped_and_counted() {
        let dir = folder(
            "not json\n{\"display\":\"fine\",\"timestamp\":1792000000000,\"project\":\"/x/a\"}\n\n\
             {\"display\":\"no time\",\"project\":\"/x/a\"}\n\
             {\"display\":\"odd fields\",\"timestamp\":\"2026-10-01T10:00:00.000Z\",\"extra\":[1]}\n",
        );
        let every = page(dir.path(), &all());
        assert_eq!((every.messages, every.skipped), (2, 2));
        // Oldest first: the ISO time (1 October) before the milliseconds (2 October).
        let read: Vec<(&str, &str)> = every
            .entries
            .iter()
            .map(|e| (e.text.as_str(), e.project.as_str()))
            .collect();
        assert_eq!(read, [("odd fields", "no folder"), ("fine", "a")]);
    }

    #[test]
    fn no_file_or_an_empty_one() {
        let none = tempfile::tempdir().unwrap();
        assert!(matches!(read(none.path()), HistorySource::Missing));
        let text = history::text(&page(none.path(), &HistoryParams::default()));
        assert_eq!(text, "No Claude Code history on this computer");
        let empty = folder("");
        assert_eq!(
            history::text(&page(empty.path(), &HistoryParams::default())),
            "No Claude Code history on this computer"
        );
        let blank = folder("\n\n");
        assert!(!page(blank.path(), &HistoryParams::default()).found);
    }

    /// The transcripts in `projects/` are never opened: a marker there never shows.
    #[test]
    fn session_transcripts_are_not_read() {
        let dir = folder(FIXTURE);
        let transcripts = dir.path().join("projects/-Users-sam-code-pocket-notes");
        std::fs::create_dir_all(&transcripts).unwrap();
        std::fs::write(
            transcripts.join("0b6c7d1e.jsonl"),
            "{\"type\":\"user\",\"message\":{\"content\":\"TRANSCRIPT-MARKER-OKAPI\"},\"timestamp\":\"2026-10-20T10:00:00.000Z\",\"cwd\":\"/Users/sam/code/pocket-notes\"}\n",
        )
        .unwrap();
        let every = page(dir.path(), &all());
        assert_eq!(every.messages, 300);
        assert!(!history::text(&every).contains("TRANSCRIPT-MARKER-OKAPI"));
        assert!(
            !serde_json::to_string(&every)
                .unwrap()
                .contains("TRANSCRIPT-MARKER-OKAPI")
        );
    }

    #[test]
    fn otaq_projects_take_their_names() {
        let dir = folder(FIXTURE);
        let projects = [KnownProject {
            key: "NOTE".into(),
            name: "Pocket Notes".into(),
            repo: "/Users/sam/code/pocket-notes".into(),
        }];
        let params = HistoryParams {
            project: Some("NOTE".into()),
            ..all()
        };
        let named = history::read(
            vec![("Claude Code", read(dir.path()))],
            &params,
            &projects,
            NOW,
        )
        .unwrap();
        assert_eq!(named.messages, 142);
        assert!(named.entries.iter().all(|e| e.project == "Pocket Notes"));
    }

    #[test]
    fn the_refusal_names_the_command() {
        assert_eq!(
            only_a_project_agent(),
            format!(
                "`{} history` is for the Project agent; ticket agents don't read your history.",
                crate::naming::COMMAND_NAME
            )
        );
    }
}
