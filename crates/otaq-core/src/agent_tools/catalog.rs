//! The models.dev catalog (AGT-02; d0086, d0088): the families and release dates behind Codex's and OpenCode's
//! "always the newest" choices ("GPT Sol · always the newest (now gpt-6-sol)").
//!
//! `https://models.dev/api.json` (about 5.8 MB, keyed by provider: `openai`, `anthropic`, …; each model with its
//! `family` and `release_date`) is read at service start when Otaq's copy is older than a day, and once a day after
//! ([`super::models::ModelLists::run`]), with `If-None-Match` and the copy's ETag. Otaq keeps a trimmed copy,
//! `provider → model id → { family, release_date }`, in `<data folder>/models-dev.json` with its ETag and when it
//! was read. A failed read keeps the old copy and is logged; with no copy at all there are no follow entries, never
//! an error (d0086: the picker still shows the tool's own list).
//!
//! Which tool takes its families from where is the adapter's [`super::AgentTool::family_source`]; the follow
//! entries themselves are made in [`super::models`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::data_dir::DataDir;

/// Where the catalog is read from.
pub const URL: &str = "https://models.dev/api.json";

/// Otaq's copy, in the data folder.
pub const FILE_NAME: &str = "models-dev.json";

/// A copy older than this is read again (d0086: "about once a day"). A little under a day, so the daily timer,
/// which ticks a day after the last read started, always finds it due.
pub const MAX_AGE: Duration = Duration::from_secs(23 * 60 * 60);

/// How long one read may take, the whole body included.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

/// Where a tool's model families come from (d0086).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FamilySource {
    /// The tool's ids are models.dev's ids of `provider` exactly: the family and release date come from the
    /// catalog (Codex: `openai`).
    ModelsDev { provider: &'static str },
    /// The tool's own list carries each model's family and release date ([`super::models::ModelEntry::family`],
    /// [`super::models::ModelEntry::released_at`]); with `models_dev_fallback`, a `provider/id` model missing either takes
    /// it from the catalog's entry for the same provider and id (OpenCode).
    ListCarriesFamily { models_dev_fallback: bool },
}

/// A model family as models.dev names it (`gpt-sol`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Family {
    pub id: String,
}

impl Family {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    /// The name people read: the id's words title-cased, known acronyms upper-cased: `gpt-sol` → "GPT Sol",
    /// `claude-opus` → "Claude Opus" (Q-L).
    pub fn display_name(&self) -> String {
        const ACRONYMS: &[&str] = &["gpt", "ai", "glm", "oss", "llm", "vl"];
        self.id
            .split(['-', '_', ' '])
            .filter(|w| !w.is_empty())
            .map(|word| {
                if ACRONYMS.contains(&word.to_ascii_lowercase().as_str()) {
                    word.to_ascii_uppercase()
                } else {
                    let mut chars = word.chars();
                    chars.next().map_or_else(String::new, |first| {
                        first.to_uppercase().chain(chars).collect()
                    })
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// What the catalog says about one model.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogModel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// As models.dev writes it: `2026-09-22`, sometimes only `2026-09`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
}

impl CatalogModel {
    /// The release date in Unix milliseconds (midnight UTC; a month alone is its first day).
    pub fn released_at(&self) -> Option<i64> {
        self.release_date.as_deref().and_then(date_ms)
    }
}

/// The catalog, trimmed to what Otaq uses: provider → model id → family and release date.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub providers: HashMap<String, HashMap<String, CatalogModel>>,
}

impl Catalog {
    /// Reads `api.json` as models.dev serves it. Models with neither a family nor a date are left out.
    pub fn parse(json: &[u8]) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Provider {
            #[serde(default)]
            models: HashMap<String, Model>,
        }
        #[derive(Deserialize)]
        struct Model {
            #[serde(default)]
            family: Option<serde_json::Value>,
            #[serde(default)]
            release_date: Option<serde_json::Value>,
        }
        let text = |v: Option<serde_json::Value>| {
            v.and_then(|v| v.as_str().map(str::trim).map(str::to_owned))
                .filter(|s| !s.is_empty())
        };
        let raw: HashMap<String, Provider> =
            serde_json::from_slice(json).map_err(|e| format!("not the catalog's shape: {e}"))?;
        let providers = raw
            .into_iter()
            .map(|(provider, p)| {
                let models = p
                    .models
                    .into_iter()
                    .filter_map(|(id, m)| {
                        let model = CatalogModel {
                            family: text(m.family),
                            release_date: text(m.release_date),
                        };
                        (model != CatalogModel::default()).then_some((id, model))
                    })
                    .collect();
                (provider, models)
            })
            .collect();
        Ok(Self { providers })
    }

    /// What the catalog says about `provider`'s model `id`.
    pub fn model(&self, provider: &str, id: &str) -> Option<&CatalogModel> {
        self.providers.get(provider)?.get(id)
    }

    /// The same for an id written `provider/id` (OpenCode's `anthropic/claude-sonnet-5-5`).
    pub fn model_by_path(&self, path: &str) -> Option<&CatalogModel> {
        let (provider, id) = path.split_once('/')?;
        self.model(provider, id)
    }
}

/// Otaq's copy as it is saved: the catalog, its ETag and when it was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogCopy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// When the catalog was last read (a 304 counts), in Unix milliseconds.
    pub read_at: i64,
    pub catalog: Catalog,
}

/// What a read did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogRead {
    /// A new catalog replaced the copy.
    Changed,
    /// models.dev answered 304: the copy stays, with a new read time.
    NotModified,
    /// The copy is younger than the age asked for: nothing was read.
    Fresh,
}

/// Otaq's copy of the catalog and how to read it again. Cheap to clone.
#[derive(Clone)]
pub struct Catalogs {
    url: String,
    path: PathBuf,
    client: reqwest::Client,
    copy: Arc<RwLock<Option<Arc<CatalogCopy>>>>,
}

impl Catalogs {
    /// The copy saved in `data`'s folder, read from models.dev.
    pub fn load(data: &DataDir) -> Self {
        Self::load_from(data.root().join(FILE_NAME), URL)
    }

    /// The copy saved at `path`, read from `url` (tests: a local server).
    pub fn load_from(path: PathBuf, url: impl Into<String>) -> Self {
        let copy = saved(&path);
        let client = reqwest::Client::builder()
            .user_agent(format!(
                "{}/{}",
                crate::naming::COMMAND_NAME,
                env!("CARGO_PKG_VERSION")
            ))
            .timeout(READ_TIMEOUT)
            .build()
            .unwrap_or_default();
        Self {
            url: url.into(),
            path,
            client,
            copy: Arc::new(RwLock::new(copy.map(Arc::new))),
        }
    }

    /// The copy, if there is one.
    pub fn copy(&self) -> Option<Arc<CatalogCopy>> {
        self.copy.read().expect("catalog").clone()
    }

    /// Reads the catalog again when the copy is missing or at least `max_age` old at `now` (Unix milliseconds).
    pub async fn refresh_if_older(
        &self,
        max_age: Duration,
        now: i64,
    ) -> Result<CatalogRead, CatalogError> {
        let age_ms = i64::try_from(max_age.as_millis()).unwrap_or(i64::MAX);
        if self
            .copy()
            .is_some_and(|c| now.saturating_sub(c.read_at) < age_ms)
        {
            return Ok(CatalogRead::Fresh);
        }
        self.refresh(now).await
    }

    /// Reads the catalog now, with the copy's ETag. A new catalog replaces the copy and is saved; a 304 keeps it
    /// with `now` as its read time; anything else keeps it as it is and is answered as an error (logged).
    pub async fn refresh(&self, now: i64) -> Result<CatalogRead, CatalogError> {
        let read = self.read(now).await;
        match &read {
            Ok(CatalogRead::Changed) => tracing::info!("models.dev catalog read"),
            Ok(_) => tracing::debug!("models.dev catalog unchanged"),
            Err(e) => {
                tracing::warn!(error = %e, "models.dev catalog not read; the last copy stays")
            }
        }
        read
    }

    async fn read(&self, now: i64) -> Result<CatalogRead, CatalogError> {
        let old = self.copy();
        let mut request = self.client.get(&self.url);
        if let Some(etag) = old.as_ref().and_then(|c| c.etag.as_deref()) {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        let response = request
            .send()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_MODIFIED {
            let Some(old) = old else {
                return Err(CatalogError("304 without a copy".into()));
            };
            self.keep(CatalogCopy {
                read_at: now,
                ..(*old).clone()
            })?;
            return Ok(CatalogRead::NotModified);
        }
        if !status.is_success() {
            return Err(CatalogError(format!("models.dev answered {status}")));
        }
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = response
            .bytes()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        let catalog = Catalog::parse(&body).map_err(CatalogError)?;
        self.keep(CatalogCopy {
            etag,
            read_at: now,
            catalog,
        })?;
        Ok(CatalogRead::Changed)
    }

    /// Saves `copy` (a whole new file put in place) and holds it in memory.
    fn keep(&self, copy: CatalogCopy) -> Result<(), CatalogError> {
        let json = serde_json::to_vec(&copy).expect("a catalog serialises");
        write_replacing(&self.path, &json)
            .map_err(|e| CatalogError(format!("can't save {}: {e}", self.path.display())))?;
        *self.copy.write().expect("catalog") = Some(Arc::new(copy));
        Ok(())
    }
}

/// Why the catalog wasn't read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogError(pub String);

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CatalogError {}

/// The copy saved at `path`; a file that doesn't read counts as none (logged).
fn saved(path: &Path) -> Option<CatalogCopy> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "can't read the models.dev copy");
            return None;
        }
    };
    serde_json::from_slice(&bytes)
        .inspect_err(|e| tracing::warn!(error = %e, "the models.dev copy doesn't read"))
        .ok()
}

/// Writes `bytes` to a file beside `path`, then moves it over `path`, so a reader never sees half a file.
fn write_replacing(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        DataDir::ensure_dir(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// `2026-09-22` (or `2026-09`) as midnight UTC in Unix milliseconds.
fn date_ms(date: &str) -> Option<i64> {
    let mut parts = date.trim().splitn(3, '-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next().map_or(Some(1), |d| d.parse().ok())?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Days from 1970-01-01 in the proleptic Gregorian calendar (Howard Hinnant's days_from_civil).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * 86_400_000)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// A trimmed `api.json`, shaped from models.dev as read on 2026-10-17 (other fields and providers left out).
    pub(crate) const FIXTURE: &str = include_str!("../../tests/fixtures/models-dev/api.json");

    pub(crate) fn fixture() -> Catalog {
        Catalog::parse(FIXTURE.as_bytes()).unwrap()
    }

    pub(crate) fn date(text: &str) -> Option<i64> {
        date_ms(text)
    }

    #[test]
    fn the_catalog_reads_families_and_dates_by_provider() {
        let c = fixture();
        let sol = c.model("openai", "gpt-6-sol").unwrap();
        assert_eq!(sol.family.as_deref(), Some("gpt-sol"));
        assert_eq!(sol.release_date.as_deref(), Some("2026-09-22"));
        assert!(sol.released_at() > c.model("openai", "gpt-5.6-sol").unwrap().released_at());
        assert_eq!(
            c.model_by_path("anthropic/claude-opus-5-5")
                .and_then(|m| m.family.as_deref()),
            Some("claude-opus")
        );
        assert_eq!(c.model("openai", "gpt-9"), None);
        assert!(Catalog::parse(b"[1, 2]").is_err());
    }

    #[test]
    fn dates_and_family_names() {
        assert_eq!(date_ms("1970-01-02"), Some(86_400_000));
        assert_eq!(date_ms("2026-09"), date_ms("2026-09-01"));
        assert_eq!(date_ms("soon"), None);
        assert_eq!(Family::new("gpt-sol").display_name(), "GPT Sol");
        assert_eq!(Family::new("claude-opus").display_name(), "Claude Opus");
        assert_eq!(Family::new("gpt-oss").display_name(), "GPT OSS");
    }

    /// One canned answer: status, headers, body.
    type Answer = (u16, Vec<(&'static str, &'static str)>, &'static str);

    /// A local server answering each request with the next of `answers` (status, headers, body), and telling
    /// what each request's `If-None-Match` was.
    async fn server(
        answers: Vec<Answer>,
    ) -> (String, tokio::sync::mpsc::UnboundedReceiver<Option<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/api.json", listener.local_addr().unwrap());
        let (seen, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            for (status, headers, body) in answers {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buf = [0u8; 4096];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = socket.read(&mut buf).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    request.extend_from_slice(&buf[..n]);
                }
                let text = String::from_utf8_lossy(&request).to_string();
                let etag = text.lines().find_map(|l| {
                    let (name, value) = l.split_once(':')?;
                    name.eq_ignore_ascii_case("if-none-match")
                        .then(|| value.trim().to_owned())
                });
                seen.send(etag).unwrap();
                let mut answer =
                    format!("HTTP/1.1 {status} X\r\ncontent-length: {}\r\n", body.len());
                for (name, value) in headers {
                    answer.push_str(&format!("{name}: {value}\r\n"));
                }
                answer.push_str("connection: close\r\n\r\n");
                answer.push_str(body);
                socket.write_all(answer.as_bytes()).await.unwrap();
                socket.shutdown().await.ok();
            }
        });
        (url, rx)
    }

    /// The copy is saved with its ETag; the next read sends it, a 304 keeps the copy (with a new read time), a
    /// failure keeps it too, and the next service reads it back from the file.
    #[tokio::test]
    async fn the_copy_is_saved_with_its_etag_and_kept_on_304_and_failure() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join(FILE_NAME);
        let (url, mut seen) = server(vec![
            (200, vec![("etag", "\"abc\"")], FIXTURE),
            (304, vec![("etag", "\"abc\"")], ""),
            (500, vec![], "oops"),
            (200, vec![], "not json"),
        ])
        .await;
        let catalogs = Catalogs::load_from(path.clone(), url.clone());
        assert_eq!(catalogs.copy(), None);

        assert_eq!(catalogs.refresh(1_000).await, Ok(CatalogRead::Changed));
        assert_eq!(seen.recv().await.unwrap(), None);
        let saved: CatalogCopy = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.etag.as_deref(), Some("\"abc\""));
        assert_eq!(saved.read_at, 1_000);
        assert_eq!(saved.catalog, fixture());

        assert_eq!(catalogs.refresh(2_000).await, Ok(CatalogRead::NotModified));
        assert_eq!(seen.recv().await.unwrap().as_deref(), Some("\"abc\""));
        let kept = catalogs.copy().unwrap();
        assert_eq!((kept.read_at, &kept.catalog), (2_000, &fixture()));

        assert!(catalogs.refresh(3_000).await.is_err());
        assert!(catalogs.refresh(4_000).await.is_err());
        assert_eq!(catalogs.copy(), Some(kept.clone()));

        // The next service starts from the saved file.
        let again = Catalogs::load_from(path, url);
        assert_eq!(again.copy(), Some(kept));
    }

    /// A young copy isn't read again; no server at all (a failure) leaves no copy and no error beyond the log.
    #[tokio::test]
    async fn a_young_copy_is_not_read_and_no_copy_stays_none() {
        let home = tempfile::tempdir().unwrap();
        let (url, _seen) = server(vec![(200, vec![], FIXTURE)]).await;
        let catalogs = Catalogs::load_from(home.path().join(FILE_NAME), url);
        let day = 24 * 60 * 60 * 1000;
        assert_eq!(
            catalogs.refresh_if_older(MAX_AGE, day).await,
            Ok(CatalogRead::Changed)
        );
        assert_eq!(
            catalogs.refresh_if_older(MAX_AGE, day + 1_000).await,
            Ok(CatalogRead::Fresh)
        );

        let none = Catalogs::load_from(home.path().join("other.json"), "http://127.0.0.1:9/");
        assert!(none.refresh(1).await.is_err());
        assert_eq!(none.copy(), None);
    }
}
