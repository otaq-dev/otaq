//! The session made before launch (ADP-04, ADP-09; T-0354): `opencode api --standalone POST /api/session` with a
//! `ses_…` id, the title and the model, as a [`crate::agent_tools::PrepareStep`] the service runs in the working
//! folder before it starts OpenCode with `--session <id>`.
//!
//! Checked with OpenCode 2.0.17 (its own home folders pointed at a scratch folder, no provider, no model call): the
//! call answers `{"data":{"id":"ses_…","title":…,"model":{"id","providerID","variant"}…}}` and exits 0; an id that
//! doesn't start with `ses` answers `InvalidRequestError … Expected a string starting with "ses"`, `HTTP 400 Bad
//! Request` and exits 1. The id has OpenCode's own form (its `SessionID.descending`): `ses_`, 12 hex digits of the
//! time in milliseconds times 4096 plus a counter, bit-inverted, then 14 random base-62 characters, so Otaq's
//! sessions sort among the user's own by time in OpenCode's session list.

use std::path::Path;
use std::sync::atomic::{AtomicU16, Ordering};

use serde_json::{Value, json};

use super::COMMAND_NAME;
use crate::agent_tools::PrepareStep;

/// OpenCode's session id prefix.
const SESSION_PREFIX: &str = "ses_";

/// The random part's characters (OpenCode's own).
const BASE62: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Keeps two ids made in the same millisecond apart.
static COUNTER: AtomicU16 = AtomicU16::new(0);

/// A new session id in OpenCode's form, from the time `now_ms` (Unix milliseconds).
pub(super) fn new_session_id(now_ms: i64) -> String {
    let count = u64::from(COUNTER.fetch_add(1, Ordering::Relaxed) % 0x1000);
    let time = u64::try_from(now_ms).unwrap_or(0);
    let descending = !(time.wrapping_mul(0x1000).wrapping_add(count)) & 0xffff_ffff_ffff;
    // The random halves of two v7 UUIDs (the crate's only generator here).
    let random: Vec<u8> = (0..2)
        .flat_map(|_| uuid::Uuid::now_v7().as_bytes()[8..].to_vec())
        .collect();
    let tail: String = random[..14]
        .iter()
        .map(|b| char::from(BASE62[usize::from(*b) % BASE62.len()]))
        .collect();
    format!("{SESSION_PREFIX}{descending:012x}{tail}")
}

/// `{"providerID": …, "id": …[, "variant": …]}` of a model named `provider/model[#variant]` as OpenCode's list
/// names it; `None` for a name without a provider.
pub(super) fn model_ref(model: &str) -> Option<Value> {
    let (provider, rest) = model.split_once('/')?;
    let (id, variant) = match rest.split_once('#') {
        Some((id, variant)) => (id, Some(variant)),
        None => (rest, None),
    };
    if provider.is_empty() || id.is_empty() || variant.is_some_and(str::is_empty) {
        return None;
    }
    let mut value = json!({ "providerID": provider, "id": id });
    if let Some(variant) = variant {
        value["variant"] = json!(variant);
    }
    Some(value)
}

/// The step that creates session `id` titled `title` in `cwd`, on `model` (a [`model_ref`]) when one is given;
/// OpenCode found on `path`.
pub(super) fn create_session(
    id: &str,
    title: &str,
    model: Option<Value>,
    cwd: &Path,
    path: String,
) -> PrepareStep {
    let mut body = json!({ "id": id, "title": title });
    if let Some(model) = model {
        body["model"] = model;
    }
    PrepareStep {
        program: COMMAND_NAME.to_owned(),
        args: vec![
            "api".to_owned(),
            "--standalone".to_owned(),
            "POST".to_owned(),
            "/api/session".to_owned(),
            "-d".to_owned(),
            body.to_string(),
        ],
        cwd: cwd.to_path_buf(),
        env: vec![("PATH".to_owned(), path)],
    }
}
