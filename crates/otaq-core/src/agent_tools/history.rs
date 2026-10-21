//! `otaq history` (ONB-01, d0108): what the user typed into their agent tool, in every project, for the Project agent
//! that proposes a first flow. Each tool's adapter reads its own file ([`super::AgentTool::history`]; only Claude
//! Code keeps one Otaq reads); this module filters the entries, counts them and cuts them into pages, in one pass
//! over the lines and without keeping more than the page asked for. Nothing is written to disk.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::local_time;
use crate::naming;

/// Entries on one page.
pub const PAGE_SIZE: usize = 500;

/// What an entry that held only pasted text prints: the paste itself is never read.
pub const PASTE_LEFT_OUT: &str = "[pasted text left out]";

/// The refusal for every caller but a Project agent and the user (ONB-02): "`otaq history` is for the Project
/// agent; ticket agents don't read your history."
pub fn only_a_project_agent() -> String {
    format!(
        "`{} history` is for the Project agent; ticket agents don't read your history.",
        naming::COMMAND_NAME
    )
}

/// One thing the user typed (a message or a slash command line), as the tool wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Unix milliseconds.
    pub at: i64,
    /// The folder the tool ran in; `None` when the line names none.
    pub folder: Option<String>,
    /// The text; [`PASTE_LEFT_OUT`] when it was only pasted text.
    pub text: String,
}

/// One line of a tool's history file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryLine {
    Entry(HistoryEntry),
    /// A line that couldn't be read: skipped and counted.
    Unreadable,
}

/// A tool's history, line by line.
pub type HistoryLines = Box<dyn Iterator<Item = HistoryLine> + Send>;

/// What a tool's adapter found.
pub enum HistorySource {
    /// No history file (or an empty one): "No Claude Code history on this computer".
    Missing,
    /// The file is there but can't be opened; the reason is shown as it is.
    Unopenable(String),
    Lines(HistoryLines),
}

/// How far back: `30d`, `12m`, `2y`, or all of it. Written and read as that text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Since {
    Days(u32),
    Months(u32),
    Years(u32),
    All,
}

impl Default for Since {
    /// The last 12 months (d0108).
    fn default() -> Self {
        Self::Months(12)
    }
}

impl std::str::FromStr for Since {
    type Err = String;

    /// `30d`, `12m`, `2y` (a whole number above 0, then the unit), or `all`.
    fn from_str(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if text == "all" {
            return Ok(Self::All);
        }
        let wrong =
            || format!("{text:?} isn't a period: give a number and d, m or y, like 30d, 12m or 2y");
        let unit = text.chars().last().ok_or_else(wrong)?;
        let count: u32 = text[..text.len() - unit.len_utf8()]
            .parse()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(wrong)?;
        match unit {
            'd' => Ok(Self::Days(count)),
            'm' => Ok(Self::Months(count)),
            'y' => Ok(Self::Years(count)),
            _ => Err(wrong()),
        }
    }
}

impl TryFrom<String> for Since {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        text.parse()
    }
}

impl From<Since> for String {
    fn from(since: Since) -> Self {
        match since {
            Since::Days(n) => format!("{n}d"),
            Since::Months(n) => format!("{n}m"),
            Since::Years(n) => format!("{n}y"),
            Since::All => "all".into(),
        }
    }
}

impl Since {
    /// The first moment counted, for `now` (Unix milliseconds); `None` for all of it. Months and years are
    /// calendar ones (12 months before 21 October is 21 October), counted in UTC.
    pub fn start(self, now: i64) -> Option<i64> {
        const DAY: i64 = 86_400_000;
        match self {
            Self::Days(n) => Some(now - i64::from(n) * DAY),
            Self::Months(n) => Some(months_before(now, n)),
            Self::Years(n) => Some(months_before(now, n.saturating_mul(12))),
            Self::All => None,
        }
    }

    /// "last 12 months", "last 30 days", "last month", "all time".
    pub fn label(self) -> String {
        let last = |n: u32, one: &str, many: &str| match n {
            1 => format!("last {one}"),
            n => format!("last {n} {many}"),
        };
        match self {
            Self::Days(n) => last(n, "day", "days"),
            Self::Months(n) => last(n, "month", "months"),
            Self::Years(n) => last(n, "year", "years"),
            Self::All => "all time".into(),
        }
    }
}

/// `otaq history`'s request (ONB-01). Every field may be left out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryParams {
    #[serde(default)]
    pub since: Since,
    /// Only this project: its name as the lines print it, or an Otaq project's key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Only this slash command: `/security-review` (the slash may be left out).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// From 1; 0 and a missing page are the first.
    #[serde(default)]
    pub page: u32,
}

/// An Otaq project, for the names the lines print: entries made in its folder carry its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownProject {
    pub key: String,
    pub name: String,
    pub repo: String,
}

/// One printed entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageEntry {
    /// Unix milliseconds.
    pub at: i64,
    /// `2026-10-19 14:02`, local time.
    pub time: String,
    pub project: String,
    pub text: String,
}

/// One page of `otaq history`, with the totals of every entry it was cut from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryPage {
    /// The tools read: "Claude Code".
    pub tools: Vec<String>,
    /// Whether any tool had a history at all; when none did, the page says so and nothing else.
    pub found: bool,
    pub since: Since,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Every entry that matches, on every page.
    pub messages: usize,
    /// The projects they were typed in.
    pub projects: usize,
    pub page: u32,
    pub pages: u32,
    /// Lines that couldn't be read.
    pub skipped: usize,
    /// This page's entries, oldest first.
    pub entries: Vec<PageEntry>,
}

/// Reads `sources` (each tool's display name and history) for `params`, at `now` (Unix milliseconds), naming
/// entries after `projects`.
pub fn read(
    sources: Vec<(&str, HistorySource)>,
    params: &HistoryParams,
    projects: &[KnownProject],
    now: i64,
) -> Result<HistoryPage, String> {
    read_pages(sources, params, projects, now, PAGE_SIZE)
}

fn read_pages(
    sources: Vec<(&str, HistorySource)>,
    params: &HistoryParams,
    projects: &[KnownProject],
    now: i64,
    page_size: usize,
) -> Result<HistoryPage, String> {
    let page = params.page.max(1);
    let start = params.since.start(now);
    let command = params.command.as_deref().map(|c| {
        let c = c.trim();
        if c.starts_with('/') {
            c.to_owned()
        } else {
            format!("/{c}")
        }
    });
    let wanted_project = params.project.as_deref().map(str::trim);
    let first = (page as usize - 1) * page_size;
    let mut result = HistoryPage {
        tools: sources.iter().map(|(name, _)| (*name).to_owned()).collect(),
        found: false,
        since: params.since,
        project: params.project.clone(),
        command: command.clone(),
        messages: 0,
        projects: 0,
        page,
        pages: 1,
        skipped: 0,
        entries: Vec::new(),
    };
    let mut names = Names::new(projects);
    let mut typed_in: HashSet<String> = HashSet::new();
    for (_, source) in sources {
        let lines = match source {
            HistorySource::Missing => continue,
            HistorySource::Unopenable(reason) => return Err(reason),
            HistorySource::Lines(lines) => lines,
        };
        for line in lines {
            let entry = match line {
                HistoryLine::Entry(entry) => entry,
                HistoryLine::Unreadable => {
                    result.found = true;
                    result.skipped += 1;
                    continue;
                }
            };
            result.found = true;
            if start.is_some_and(|start| entry.at < start) {
                continue;
            }
            if let Some(command) = &command
                && !runs(&entry.text, command)
            {
                continue;
            }
            let (project, key) = names.of(entry.folder.as_deref());
            if let Some(wanted) = wanted_project
                && !project.eq_ignore_ascii_case(wanted)
                && !key.is_some_and(|k| k.eq_ignore_ascii_case(wanted))
            {
                continue;
            }
            if (first..first + page_size).contains(&result.messages) {
                result.entries.push(PageEntry {
                    at: entry.at,
                    time: time_text(entry.at),
                    project: project.clone(),
                    text: entry.text,
                });
            }
            result.messages += 1;
            typed_in.insert(project);
        }
    }
    result.projects = typed_in.len();
    result.pages = u32::try_from(result.messages.div_ceil(page_size).max(1)).unwrap_or(u32::MAX);
    result.entries.sort_by_key(|e| e.at);
    Ok(result)
}

/// Whether `text` runs the slash command `command` (`/commit`, or `/commit fix the typo`).
fn runs(text: &str, command: &str) -> bool {
    text.trim_start()
        .strip_prefix(command)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
}

/// The project names of folders, worked out once per folder.
struct Names<'a> {
    projects: &'a [KnownProject],
    seen: HashMap<String, (String, Option<&'a str>)>,
}

impl<'a> Names<'a> {
    fn new(projects: &'a [KnownProject]) -> Self {
        Self {
            projects,
            seen: HashMap::new(),
        }
    }

    /// The Otaq project's name (and key) when `folder` is its repository, else the folder's own name.
    fn of(&mut self, folder: Option<&str>) -> (String, Option<&'a str>) {
        let Some(folder) = folder else {
            return ("no folder".into(), None);
        };
        if let Some(known) = self.seen.get(folder) {
            return known.clone();
        }
        let trimmed = |p: &str| p.trim_end_matches(['/', '\\']).to_owned();
        let named = self
            .projects
            .iter()
            .find(|p| trimmed(&p.repo) == trimmed(folder))
            .map(|p| (p.name.clone(), Some(p.key.as_str())))
            .unwrap_or_else(|| {
                let base = Path::new(&trimmed(folder))
                    .file_name()
                    .map_or_else(|| folder.to_owned(), |n| n.to_string_lossy().into_owned());
                (base, None)
            });
        self.seen.insert(folder.to_owned(), named.clone());
        named
    }
}

/// `2026-10-19 14:02` in local time.
fn time_text(at: i64) -> String {
    local_time::local(at).map_or_else(String::new, |t| {
        format!(
            "{}-{:02}-{:02} {:02}:{:02}",
            t.year, t.month, t.day, t.hour, t.minute
        )
    })
}

/// What `otaq history` prints: the totals, the page's entries and where the next page is.
pub fn text(page: &HistoryPage) -> String {
    if !page.found {
        return format!("No {} history on this computer", page.tools.join(" or "));
    }
    let mut out = format!(
        "{} of your messages in {} \u{b7} {}\n",
        thousands(page.messages),
        match page.projects {
            1 => "1 project".to_owned(),
            n => format!("{} projects", thousands(n)),
        },
        page.since.label()
    );
    for entry in &page.entries {
        let mut lines = entry.text.lines();
        let first = lines.next().unwrap_or_default();
        out.push_str(&format!(
            "{} \u{b7} {} \u{b7} {first}\n",
            entry.time, entry.project
        ));
        for line in lines {
            let line = line.trim_end();
            if line.is_empty() {
                out.push('\n');
            } else {
                out.push_str(&format!("    {line}\n"));
            }
        }
    }
    let mut last = vec![format!("page {} of {}", page.page, page.pages)];
    if page.page < page.pages {
        last.push(format!("next: `{}`", page_command(page, page.page + 1)));
    } else if page.page > page.pages {
        last.push(format!("the last is `{}`", page_command(page, page.pages)));
    }
    match page.skipped {
        0 => {}
        1 => last.push("1 line skipped".into()),
        n => last.push(format!("{} lines skipped", thousands(n))),
    }
    out.push_str(&last.join(" \u{b7} "));
    out
}

/// The command that prints page `n` with the same filters: `otaq history --command /commit --page 2`.
fn page_command(page: &HistoryPage, n: u32) -> String {
    let mut words = vec![naming::COMMAND_NAME.to_owned(), "history".into()];
    match page.since {
        Since::All => words.push("--all".into()),
        since if since == Since::default() => {}
        since => words.extend(["--since".into(), String::from(since)]),
    }
    if let Some(project) = &page.project {
        words.extend(["--project".into(), project.clone()]);
    }
    if let Some(command) = &page.command {
        words.extend(["--command".into(), command.clone()]);
    }
    if n > 1 {
        words.extend(["--page".into(), n.to_string()]);
    }
    words.join(" ")
}

/// 2140 → "2,140".
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The same moment `months` calendar months earlier, in UTC (the day kept, or the month's last day).
fn months_before(now: i64, months: u32) -> i64 {
    const DAY: i64 = 86_400_000;
    let (days, time) = (now.div_euclid(DAY), now.rem_euclid(DAY));
    let (year, month, day) = civil_from_days(days);
    let index = year * 12 + i64::from(month) - 1 - i64::from(months);
    let (year, month) = (
        index.div_euclid(12),
        u32::try_from(index.rem_euclid(12)).unwrap_or(0) + 1,
    );
    let day = day.min(days_in_month(year, month));
    days_from_civil(year, month, day) * DAY + time
}

fn days_in_month(year: i64, month: u32) -> u32 {
    let next = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    u32::try_from(days_from_civil(next.0, next.1, 1) - days_from_civil(year, month, 1))
        .unwrap_or(28)
}

/// Howard Hinnant's `days_from_civil`: days since 1970-01-01.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let (month, day) = (i64::from(month), i64::from(day));
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Howard Hinnant's `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400_000;

    fn entry(at: i64, folder: &str, text: &str) -> HistoryLine {
        HistoryLine::Entry(HistoryEntry {
            at,
            folder: Some(folder.into()),
            text: text.into(),
        })
    }

    fn source(lines: Vec<HistoryLine>) -> Vec<(&'static str, HistorySource)> {
        vec![(
            "Claude Code",
            HistorySource::Lines(Box::new(lines.into_iter())),
        )]
    }

    #[test]
    fn periods_read_and_write_as_text() {
        assert_eq!("30d".parse(), Ok(Since::Days(30)));
        assert_eq!("12m".parse(), Ok(Since::Months(12)));
        assert_eq!("2y".parse(), Ok(Since::Years(2)));
        assert_eq!("all".parse(), Ok(Since::All));
        for wrong in ["", "m", "0m", "12", "12w", "-1d", "1.5y"] {
            assert!(wrong.parse::<Since>().is_err(), "{wrong}");
        }
        assert_eq!(String::from(Since::Months(12)), "12m");
        assert_eq!(Since::default(), Since::Months(12));
        assert_eq!(Since::Months(12).label(), "last 12 months");
        assert_eq!(Since::Months(1).label(), "last month");
        assert_eq!(Since::Days(30).label(), "last 30 days");
        assert_eq!(Since::All.label(), "all time");
    }

    #[test]
    fn months_are_calendar_months() {
        // 2026-10-21 12:00 UTC.
        let now = 1_792_584_000_000;
        assert_eq!(Since::Months(12).start(now), Some(now - 365 * DAY));
        assert_eq!(Since::Years(1).start(now), Some(now - 365 * DAY));
        assert_eq!(Since::Days(30).start(now), Some(now - 30 * DAY));
        assert_eq!(Since::All.start(now), None);
        // 2026-03-31 → 2026-02-28.
        let march_31 = days_from_civil(2026, 3, 31) * DAY;
        assert_eq!(
            months_before(march_31, 1),
            days_from_civil(2026, 2, 28) * DAY
        );
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
    }

    #[test]
    fn numbers_have_a_thousands_separator() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(2140), "2,140");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn slash_commands_match_whole_words() {
        assert!(runs("/commit", "/commit"));
        assert!(runs("/commit fix the typo", "/commit"));
        assert!(!runs("/commit-all", "/commit"));
        assert!(!runs("please /commit", "/commit"));
    }

    #[test]
    fn otaq_projects_name_their_folders() {
        let projects = [KnownProject {
            key: "NOTE".into(),
            name: "Pocket Notes".into(),
            repo: "/Users/sam/code/pocket-notes".into(),
        }];
        let lines = vec![
            entry(2 * DAY, "/Users/sam/code/pocket-notes/", "one"),
            entry(3 * DAY, "/Users/sam/scratch", "two"),
        ];
        let params = HistoryParams {
            since: Since::All,
            ..HistoryParams::default()
        };
        let page = read(source(lines.clone()), &params, &projects, 10 * DAY).unwrap();
        let named: Vec<&str> = page.entries.iter().map(|e| e.project.as_str()).collect();
        assert_eq!(named, ["Pocket Notes", "scratch"]);
        // By the printed name or the key.
        for wanted in ["pocket notes", "NOTE"] {
            let only = HistoryParams {
                project: Some(wanted.into()),
                ..params.clone()
            };
            let page = read(source(lines.clone()), &only, &projects, 10 * DAY).unwrap();
            assert_eq!(page.messages, 1, "{wanted}");
        }
    }

    #[test]
    fn the_text_has_totals_lines_and_the_next_page() {
        let lines = vec![
            entry(DAY, "/Users/sam/code/pocket-api", "/commit"),
            HistoryLine::Unreadable,
            entry(
                2 * DAY,
                "/Users/sam/code/pocket-api",
                "plan:\n1. cache\n\n2. queue",
            ),
            entry(3 * DAY, "/Users/sam/scratch", PASTE_LEFT_OUT),
            HistoryLine::Unreadable,
        ];
        let params = HistoryParams {
            since: Since::All,
            page: 1,
            ..HistoryParams::default()
        };
        let page = read_pages(source(lines), &params, &[], 10 * DAY, 2).unwrap();
        assert_eq!(
            (page.messages, page.projects, page.pages, page.skipped),
            (3, 2, 2, 2)
        );
        let (one, two) = (time_text(DAY), time_text(2 * DAY));
        assert_eq!(
            text(&page),
            format!(
                "3 of your messages in 2 projects \u{b7} all time\n\
                 {one} \u{b7} pocket-api \u{b7} /commit\n\
                 {two} \u{b7} pocket-api \u{b7} plan:\n    1. cache\n\n    2. queue\n\
                 page 1 of 2 \u{b7} next: `{cmd} history --all --page 2` \u{b7} 2 lines skipped",
                cmd = naming::COMMAND_NAME
            )
        );
        assert!(one.len() == 16 && one.as_bytes()[4] == b'-' && one.as_bytes()[10] == b' ');
    }

    #[test]
    fn no_history_says_so() {
        let page = read(
            vec![("Claude Code", HistorySource::Missing)],
            &HistoryParams::default(),
            &[],
            DAY,
        )
        .unwrap();
        assert!(!page.found);
        assert_eq!(text(&page), "No Claude Code history on this computer");
        let empty = read(source(Vec::new()), &HistoryParams::default(), &[], DAY).unwrap();
        assert_eq!(text(&empty), "No Claude Code history on this computer");
    }
}
