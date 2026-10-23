//! Comment reports (CMT-03, d0109): long text attached to one ticket comment, kept as a file in Otaq's data folder
//! ([`DataDir::report_path`]), never in the repository (d0023). A copied file keeps its own name
//! (`reports/<repo>/<KEY>/<comment id>/test-run-login.md`), which the Attachments column and `prime` show (RPT-02);
//! pasted text is `reports/<repo>/<KEY>/<comment id>.txt` and shows as "Pasted text".
//!
//! A report comes from a file an agent's tool wrote (`cargo test > "$OTAQ_RESOURCE_DIR/test-run.txt"`, then
//! `--report-file`) or from text the user pasted in the app. Either way it is **copied** (the original stays where it
//! is), **streamed** (never read into memory whole) and sized and line-counted on the way. Over [`REPORT_LIMIT_BYTES`]
//! it is refused and nothing is left behind. It outlives its comment's delete and goes with its ticket (d0060).

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{Board, BoardError, Refusal};
use crate::data_dir::DataDir;
use crate::domain::TicketKey;
use crate::store::new_id;

/// The largest report Otaq keeps: 20 MB (20 × 1024 × 1024 bytes). Exactly this size is accepted (CMT-03).
pub const REPORT_LIMIT_BYTES: u64 = 20 * 1024 * 1024;

/// What the Attachments column and `prime` call a report without a name of its own: text the user pasted.
pub const PASTED_TEXT: &str = "Pasted text";

/// Where a new report's text comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportSource {
    /// A file to copy (`--report-file PATH`), as an absolute path; the CLI makes a relative one absolute.
    Path(PathBuf),
    /// Text the user pasted into the comment box (the app's chip, CMT-11).
    Text(String),
}

/// A report's size and line count, as the thread, `prime` and History show it ("Report · 48 KB", "2,310 lines").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportInfo {
    pub bytes: u64,
    pub lines: u64,
}

/// A report copied into the data folder for a comment that is about to be posted ([`Board::prepare_report`]). The
/// comment takes its id. If the comment isn't posted after all (its transaction failed), [`Self::discard`] removes
/// the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredReport {
    /// The id the comment gets: the file is named after it.
    pub comment_id: String,
    /// The file, relative to the data folder (`reports/shop-web/SHOP-34/<id>/test-run.txt`), as the comment stores
    /// it.
    pub relative: String,
    /// The file's full path.
    pub path: PathBuf,
    pub info: ReportInfo,
}

/// The name a report's source file gives it (`test-run-login.md`); pasted text has none. A `\` (a folder separator
/// on Windows, a plain character elsewhere) becomes `_` so the name is one path part everywhere.
fn source_name(source: &ReportSource) -> Option<String> {
    match source {
        ReportSource::Path(path) => path
            .file_name()
            .map(|n| n.to_string_lossy().replace('\\', "_")),
        ReportSource::Text(_) => None,
    }
}

/// A stored report's own name, from the path its comment stores: the file's name when it sits in its comment's folder
/// (`reports/shop-web/SHOP-34/<id>/test-run-login.md`), `None` for pasted text (`…/<id>.txt`).
pub fn stored_name(relative: &str, comment_id: &str) -> Option<String> {
    let path = Path::new(relative);
    let folder = path.parent()?.file_name()?;
    (folder == comment_id)
        .then(|| path.file_name())
        .flatten()
        .map(|n| n.to_string_lossy().into_owned())
}

impl StoredReport {
    /// Removes the copied file, and its comment's folder for a named one: its comment wasn't posted.
    pub fn discard(&self) {
        if let Err(e) = std::fs::remove_file(&self.path) {
            tracing::warn!(path = %self.path.display(), error = %e, "can't remove an unposted report");
        }
        if stored_name(&self.relative, &self.comment_id).is_some()
            && let Some(folder) = self.path.parent()
        {
            let _ = std::fs::remove_dir(folder);
        }
    }
}

/// A size as people read it: "312 bytes", "48 KB", "1.5 MB", "212 MB" (1 KB = 1024 bytes). The thread's chip, `prime`'s
/// "Report: 48 KB" line and the refusals all use it.
pub fn size_label(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    let scaled = |unit: u64, name: &str| {
        // Under 10 of a unit, one decimal ("1.5 MB"); a whole number otherwise ("212 MB").
        let tenths = (bytes * 10 + unit / 2) / unit;
        if tenths < 100 && !tenths.is_multiple_of(10) {
            format!("{}.{} {name}", tenths / 10, tenths % 10)
        } else {
            format!("{} {name}", (bytes + unit / 2) / unit)
        }
    };
    match bytes {
        1 => "1 byte".to_owned(),
        b if b < KB => format!("{b} bytes"),
        b if b < MB - KB / 2 => format!("{} KB", ((b + KB / 2) / KB).max(1)),
        b if b < GB - MB / 2 => scaled(MB, "MB"),
        _ => scaled(GB, "GB"),
    }
}

/// A count with thousands separators: "2,000", "48,210".
pub fn count_label(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The refusal of a report over the limit (CMT-03): "The report is 212 MB; the limit is 20 MB. Keep the part that
/// matters, for example the failing tests." `bytes` is `None` when the copy stopped at the limit without knowing the
/// whole size; a size that reads the same as the limit says "more than 20 MB".
pub fn too_big(bytes: Option<u64>) -> String {
    let limit = size_label(REPORT_LIMIT_BYTES);
    let size = bytes
        .map(size_label)
        .filter(|s| *s != limit)
        .unwrap_or_else(|| format!("more than {limit}"));
    format!(
        "The report is {size}; the limit is {limit}. Keep the part that matters, for example the failing tests."
    )
}

fn refused(message: String) -> BoardError {
    BoardError::refused(Refusal::BadReport, message)
}

/// Why the report at `path` can't be read: "Can't read the report /tmp/run.txt: it doesn't exist".
fn unreadable(path: &Path, e: &io::Error) -> BoardError {
    let why = match e.kind() {
        io::ErrorKind::NotFound => "it doesn't exist".to_owned(),
        io::ErrorKind::PermissionDenied => "permission denied".to_owned(),
        _ => e.to_string(),
    };
    refused(format!("Can't read the report {}: {why}", path.display()))
}

/// Opens the report file at `path` (absolute) for its copy, or the refusal: missing, unreadable, not a file, or over
/// the limit. The CLI calls it before it posts anything (CMT-06).
pub fn open_report(path: &Path) -> Result<File, BoardError> {
    if !path.is_absolute() {
        return Err(refused(format!(
            "The report path must be absolute: {}",
            path.display()
        )));
    }
    let file = File::open(path).map_err(|e| unreadable(path, &e))?;
    let meta = file.metadata().map_err(|e| unreadable(path, &e))?;
    if !meta.is_file() {
        return Err(refused(format!(
            "Can't read the report {}: it isn't a file",
            path.display()
        )));
    }
    if meta.len() > REPORT_LIMIT_BYTES {
        return Err(refused(too_big(Some(meta.len()))));
    }
    Ok(file)
}

/// Counts bytes and lines as they pass to `inner`.
struct Counting<W> {
    inner: W,
    bytes: u64,
    newlines: u64,
    last: Option<u8>,
}

impl<W: Write> Counting<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            bytes: 0,
            newlines: 0,
            last: None,
        }
    }

    /// Lines: every newline, and a last line without one.
    fn info(&self) -> ReportInfo {
        ReportInfo {
            bytes: self.bytes,
            lines: self.newlines + u64::from(self.last.is_some_and(|b| b != b'\n')),
        }
    }
}

impl<W: Write> Write for Counting<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        let done = &buf[..n];
        self.bytes += n as u64;
        self.newlines += done.iter().filter(|b| **b == b'\n').count() as u64;
        if let Some(&last) = done.last() {
            self.last = Some(last);
        }
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Copies `source` to `dest` (its folder created), streaming, and returns its size and lines. Refused when the
/// source can't be read or is over the limit; nothing is left at `dest` then (the copy goes to `<dest>.part` and is
/// renamed only once whole).
pub(crate) fn copy_report(source: &ReportSource, dest: &Path) -> Result<ReportInfo, BoardError> {
    // Everything is checked before anything is written.
    let mut input: Box<dyn Read> = match source {
        ReportSource::Path(path) => Box::new(open_report(path)?),
        ReportSource::Text(text) => {
            if text.len() as u64 > REPORT_LIMIT_BYTES {
                return Err(refused(too_big(Some(text.len() as u64))));
            }
            Box::new(io::Cursor::new(text.as_bytes()))
        }
    };
    let store_error =
        |e: io::Error| refused(format!("Can't store the report in {}: {e}", dest.display()));
    if let Some(folder) = dest.parent() {
        DataDir::ensure_dir(folder).map_err(store_error)?;
    }
    // `<name>.part`, next to it: never the name itself (a report called `run.part`).
    let mut part = dest.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    let copied = (|| {
        let mut out = Counting::new(File::create(&part).map_err(store_error)?);
        // One byte past the limit is enough to know it is over (a file that grew since it was checked).
        let read = io::copy(&mut (&mut input).take(REPORT_LIMIT_BYTES + 1), &mut out);
        if let Err(e) = read {
            return Err(match source {
                ReportSource::Path(path) => unreadable(path, &e),
                ReportSource::Text(_) => store_error(e),
            });
        }
        out.flush().map_err(store_error)?;
        let info = out.info();
        if info.bytes > REPORT_LIMIT_BYTES {
            return Err(refused(too_big(None)));
        }
        std::fs::rename(&part, dest).map_err(store_error)?;
        Ok(info)
    })();
    if copied.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    copied
}

impl Board {
    /// Copies a report into the data folder for a comment on the ticket `key` of the repository `repo`, before the
    /// comment is posted: a missing, unreadable or oversized report is refused before anything is posted (CMT-06).
    /// The copy runs off the async threads.
    pub async fn prepare_report(
        &self,
        repo: &str,
        key: &TicketKey,
        source: ReportSource,
    ) -> Result<StoredReport, BoardError> {
        let comment_id = new_id();
        let name = source_name(&source);
        let path = self
            .data()
            .report_path(repo, key, &comment_id, name.as_deref())
            .map_err(|e| refused(format!("Can't store the report: {e}")))?;
        let relative = path
            .strip_prefix(self.data().root())
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        let dest = path.clone();
        let info = tokio::task::spawn_blocking(move || copy_report(&source, &dest))
            .await
            .map_err(|e| refused(format!("Can't store the report: {e}")))??;
        Ok(StoredReport {
            comment_id,
            relative,
            path,
            info,
        })
    }

    /// A report's full path from what its comment stores.
    pub fn report_file(&self, relative: &str) -> PathBuf {
        self.data().root().join(relative)
    }

    /// Removes the reports of deleted tickets (d0060), and their tickets' folders once empty. Best effort: a file
    /// that can't be removed is logged.
    pub(crate) fn remove_reports(&self, relative: &[String]) {
        for file in relative {
            let path = self.report_file(file);
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "can't remove a deleted ticket's report");
                }
            }
            // The comment's folder (a named report), the ticket's, then the repository's, when nothing else is in them;
            // never the reports folder itself.
            let mut folder = path.parent();
            let stop = self.data().reports_dir();
            for _ in 0..3 {
                let Some(dir) = folder else { break };
                if dir == stop || std::fs::remove_dir(dir).is_err() {
                    break;
                }
                folder = dir.parent();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_like_the_canvas() {
        assert_eq!(size_label(0), "0 bytes");
        assert_eq!(size_label(1), "1 byte");
        assert_eq!(size_label(312), "312 bytes");
        assert_eq!(size_label(1024), "1 KB");
        assert_eq!(size_label(48 * 1024 + 100), "48 KB");
        assert_eq!(size_label(1024 * 1024 - 1), "1 MB");
        assert_eq!(size_label(1536 * 1024), "1.5 MB");
        assert_eq!(size_label(2 * 1024 * 1024), "2 MB");
        assert_eq!(size_label(REPORT_LIMIT_BYTES), "20 MB");
        assert_eq!(size_label(212 * 1024 * 1024), "212 MB");
        assert_eq!(size_label(3 * 1024 * 1024 * 1024 / 2), "1.5 GB");
    }

    #[test]
    fn counts_have_thousands_separators() {
        assert_eq!(count_label(0), "0");
        assert_eq!(count_label(999), "999");
        assert_eq!(count_label(2_000), "2,000");
        assert_eq!(count_label(48_210), "48,210");
        assert_eq!(count_label(1_234_567), "1,234,567");
    }

    #[test]
    fn the_refusal_names_the_size() {
        assert_eq!(
            too_big(Some(212 * 1024 * 1024)),
            "The report is 212 MB; the limit is 20 MB. Keep the part that matters, for example the failing tests."
        );
        assert_eq!(
            too_big(Some(REPORT_LIMIT_BYTES + 1)),
            "The report is more than 20 MB; the limit is 20 MB. Keep the part that matters, for example the failing \
             tests."
        );
    }

    #[test]
    fn a_copied_file_keeps_its_name_and_pasted_text_has_none() {
        let path = ReportSource::Path("/tmp/runs/test-run-login.md".into());
        assert_eq!(source_name(&path).as_deref(), Some("test-run-login.md"));
        // On Windows `\` separates folders already.
        #[cfg(unix)]
        assert_eq!(
            source_name(&ReportSource::Path("/tmp/a\\b.txt".into())).as_deref(),
            Some("a_b.txt")
        );
        assert_eq!(source_name(&ReportSource::Text("x".into())), None);
        assert_eq!(
            stored_name("reports/shop-web/SHOP-34/c1/test-run-login.md", "c1").as_deref(),
            Some("test-run-login.md")
        );
        assert_eq!(stored_name("reports/shop-web/SHOP-34/c1.txt", "c1"), None);
    }

    #[test]
    fn a_report_named_like_its_part_file_is_copied_whole() {
        let dir = tempfile::TempDir::new().unwrap();
        let dest = dir.path().join("c1").join("run.part");
        let info = copy_report(&ReportSource::Text("one\n".into()), &dest).unwrap();
        assert_eq!(info.bytes, 4);
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "one\n");
        assert!(!dir.path().join("c1").join("run.part.part").exists());
    }

    #[test]
    fn lines_count_a_last_line_without_a_newline() {
        let dir = tempfile::TempDir::new().unwrap();
        let dest = dir.path().join("r").join("a.txt");
        let info = copy_report(&ReportSource::Text("one\ntwo\nthree".into()), &dest).unwrap();
        assert_eq!(
            info,
            ReportInfo {
                bytes: 13,
                lines: 3
            }
        );
        let info = copy_report(&ReportSource::Text("one\ntwo\n".into()), &dest).unwrap();
        assert_eq!(info, ReportInfo { bytes: 8, lines: 2 });
        let info = copy_report(&ReportSource::Text(String::new()), &dest).unwrap();
        assert_eq!(info, ReportInfo { bytes: 0, lines: 0 });
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "");
    }
}
