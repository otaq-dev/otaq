//! A skill folder on disk (WSK-02, WSK-03; d0092): the checks at import, the copy into `skills/<id>`, and the
//! fingerprint that tells whether Otaq's copy was edited since. Blocking file work: callers run it off the
//! service's async threads.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::data_dir::DataDir;

/// The file every skill folder has.
pub const SKILL_FILE: &str = "SKILL.md";

/// A skill folder that passed the checks and was copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copied {
    /// The `name:` of its SKILL.md.
    pub name: String,
    pub fingerprint: String,
    /// Files the SKILL.md mentions that aren't in the folder, and links that leave it (amber, WSK-02).
    pub warnings: Vec<String>,
}

/// Why a folder can't be imported (WSK-02); nothing was copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub String);

/// Checks `folder` and copies it whole into `skills/<id>` (mode 0700): files, subfolders and their modes; a link
/// inside the folder is copied as what it points to, one that leaves the folder is a warning and isn't followed.
/// Refused, with nothing left behind, when the folder has no SKILL.md or its front matter can't be read.
pub fn import(data: &DataDir, folder: &Path, id: &str) -> Result<Copied, Refused> {
    if !folder.is_dir() {
        return Err(Refused(format!("{} isn't a folder", folder.display())));
    }
    let text = read_skill_file(folder)?;
    let name = skill_name(&text).map_err(Refused)?;
    let target = data
        .skill_path(id)
        .map_err(|e| Refused(format!("Can't copy the skill: {e}")))?;
    let copied = copy_whole(folder, &target).and_then(|mut warnings| {
        warnings.extend(missing_mentions(&text, folder));
        Ok((warnings, fingerprint(&target)?))
    });
    match copied {
        Ok((warnings, fingerprint)) => Ok(Copied {
            name,
            fingerprint,
            warnings,
        }),
        Err(e) => {
            remove(&target);
            Err(Refused(format!("Can't copy the skill: {e}")))
        }
    }
}

/// Why Otaq's copy at `folder` can't be used, if it can't: it is gone, has no SKILL.md, or its front matter no
/// longer reads (WSK-04: the step then uses the built-in).
pub fn copy_problem(folder: &Path) -> Option<String> {
    if !folder.is_dir() {
        return Some(copy_missing());
    }
    read_skill_file(folder)
        .and_then(|text| skill_name(&text).map_err(Refused))
        .err()
        .map(|Refused(why)| why)
}

/// Deletes a copy (or anything at `path`); a missing one is fine.
pub fn remove(path: &Path) {
    let removed = match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    };
    if let Err(e) = removed {
        tracing::warn!(path = %path.display(), error = %e, "can't delete a workflow skill copy");
    }
}

/// "Otaq's copy is missing".
pub fn copy_missing() -> String {
    format!("{}'s copy is missing", crate::naming::PRODUCT_NAME)
}

fn read_skill_file(folder: &Path) -> Result<String, Refused> {
    let file = folder.join(SKILL_FILE);
    if !file.is_file() {
        return Err(Refused(format!("The folder has no {SKILL_FILE}")));
    }
    fs::read_to_string(&file).map_err(|e| Refused(format!("{SKILL_FILE} can't be read: {e}")))
}

/// The `name:` of a SKILL.md's front matter: the lines between a first `---` line and the next one. Each line is
/// `key: value`, a comment, or an indented (or `- `) continuation of the key above; anything else, or no closing
/// line, means the front matter doesn't read.
pub fn skill_name(text: &str) -> Result<String, String> {
    let unreadable = |why: &str| format!("{SKILL_FILE}'s front matter can't be read: {why}");
    let mut lines = text.trim_start_matches('\u{feff}').lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return Err(unreadable("it must start with a --- line"));
    }
    let mut name = None;
    let mut keyed = false;
    for (n, line) in lines.enumerate() {
        let number = n + 2;
        let trimmed = line.trim();
        if line.trim_end() == "---" {
            return match name {
                Some(name) => Ok(name),
                None => Err(format!("{SKILL_FILE}'s front matter has no name:")),
            };
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if line.starts_with([' ', '\t', '-']) {
            if !keyed {
                return Err(unreadable(&format!("line {number} continues no key")));
            }
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(unreadable(&format!("line {number} isn't key: value")));
        };
        let key_ok = !key.is_empty()
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'));
        if !key_ok {
            return Err(unreadable(&format!("line {number} isn't key: value")));
        }
        keyed = true;
        if key == "name" {
            let value = unquote(value.trim())
                .ok_or_else(|| unreadable(&format!("line {number} has an unclosed quote")))?;
            if !value.is_empty() && !matches!(value, "|" | ">") {
                name = Some(value.to_owned());
            }
        }
    }
    Err(unreadable("it has no closing --- line"))
}

/// A value without its matching quotes; `None` for an opening quote that isn't closed.
fn unquote(value: &str) -> Option<&str> {
    for quote in ['"', '\''] {
        if let Some(rest) = value.strip_prefix(quote) {
            return rest.strip_suffix(quote);
        }
    }
    Some(value)
}

/// Copies `from` into a new folder `to` (0700), returning the warnings for links it didn't follow.
fn copy_whole(from: &Path, to: &Path) -> io::Result<Vec<String>> {
    if let Some(parent) = to.parent() {
        DataDir::ensure_dir(parent)?;
    }
    remove(to);
    DataDir::ensure_dir(to)?;
    let root = crate::paths::canonical(from)?;
    let mut warnings = Vec::new();
    let mut walking = vec![root.clone()];
    copy_dir(&root, from, to, Path::new(""), &mut walking, &mut warnings)?;
    Ok(warnings)
}

fn copy_dir(
    root: &Path,
    from: &Path,
    to: &Path,
    rel: &Path,
    walking: &mut Vec<PathBuf>,
    warnings: &mut Vec<String>,
) -> io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(from)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let rel = rel.join(entry.file_name());
        let shown = rel.display();
        let mut source = path.clone();
        if fs::symlink_metadata(&path)?.is_symlink() {
            match crate::paths::canonical(&path) {
                Ok(target) if target.starts_with(root) => source = target,
                Ok(_) => {
                    warnings.push(format!("{shown} links outside the folder; not copied"));
                    continue;
                }
                Err(_) => {
                    warnings.push(format!("{shown} is a broken link; not copied"));
                    continue;
                }
            }
        }
        let meta = fs::metadata(&source)?;
        let dest = to.join(entry.file_name());
        if meta.is_dir() {
            let real = crate::paths::canonical(&source)?;
            if walking.contains(&real) {
                warnings.push(format!("{shown} links to a folder it is in; not copied"));
                continue;
            }
            fs::create_dir(&dest)?;
            walking.push(real);
            copy_dir(root, &source, &dest, &rel, walking, warnings)?;
            walking.pop();
            fs::set_permissions(&dest, meta.permissions())?;
        } else if meta.is_file() {
            // Copies the mode too.
            fs::copy(&source, &dest)?;
        }
    }
    Ok(())
}

/// A hash of the folder's files: their paths, modes and contents (and any link's target), in path order.
pub fn fingerprint(folder: &Path) -> io::Result<String> {
    let mut hash = Sha256::new();
    hash_dir(folder, Path::new(""), &mut hash)?;
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn hash_dir(folder: &Path, rel: &Path, hash: &mut Sha256) -> io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(folder)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let rel = rel.join(entry.file_name());
        let meta = fs::symlink_metadata(&path)?;
        let mut head = |kind: &str| {
            hash.update(kind.as_bytes());
            hash.update(rel.to_string_lossy().as_bytes());
            hash.update([0]);
            hash.update(mode(&meta).to_le_bytes());
        };
        if meta.is_symlink() {
            head("L");
            hash.update(fs::read_link(&path)?.to_string_lossy().as_bytes());
        } else if meta.is_dir() {
            head("D");
            hash_dir(&path, &rel, hash)?;
        } else {
            head("F");
            let contents = fs::read(&path)?;
            hash.update((contents.len() as u64).to_le_bytes());
            hash.update(&contents);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn mode(meta: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn mode(meta: &fs::Metadata) -> u32 {
    u32::from(meta.permissions().readonly())
}

/// Files the SKILL.md mentions that aren't in `folder` (WSK-02, a warning): relative paths in its code spans and
/// Markdown links, such as `scripts/check-staged.sh` or `[the style](reference.md)`.
fn missing_mentions(text: &str, folder: &Path) -> Vec<String> {
    mentions(body(text))
        .into_iter()
        .filter(|path| !folder.join(path).exists())
        .map(|path| format!("{SKILL_FILE} mentions {path}, which isn't in the folder"))
        .collect()
}

/// The SKILL.md after its front matter.
fn body(text: &str) -> &str {
    let text = text.trim_start_matches('\u{feff}');
    let Some(rest) = text.strip_prefix("---") else {
        return text;
    };
    let mut offset = text.len() - rest.len();
    for (n, line) in rest.split_inclusive('\n').enumerate() {
        offset += line.len();
        if n > 0 && line.trim_end() == "---" {
            return &text[offset..];
        }
    }
    text
}

/// The paths a text mentions, in order of first mention.
fn mentions(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    // Code spans (and fenced blocks): every other piece between backticks.
    for code in text.split('`').skip(1).step_by(2) {
        found.extend(code.split_whitespace().filter_map(as_path));
    }
    // Markdown links: `](target)`.
    for (_, rest) in text.match_indices("](").map(|(i, _)| text.split_at(i + 2)) {
        let target = rest.split([')', ' ', '\n']).next().unwrap_or_default();
        let target = target.split('#').next().unwrap_or_default();
        found.extend(as_path(target));
    }
    let mut seen = BTreeSet::new();
    found.retain(|path| seen.insert(path.clone()));
    found
}

/// A token that looks like a relative file path: it has a folder (`scripts/x`) or an extension of 2–6 characters
/// (`reference.md`, not `e.g`),
/// and is no URL, option, variable or absolute path.
fn as_path(token: &str) -> Option<String> {
    let token = token
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '(' | ')' | ',' | ';' | ':' | '!' | '?'))
        .trim_end_matches('.');
    let token = token.strip_prefix("./").unwrap_or(token);
    let plain = !token.is_empty()
        && !token.contains("://")
        && !token.starts_with(['/', '~', '-', '#', '.'])
        && !token.contains(['*', '<', '>', '{', '}', '$', '=', '|', '\\', '[', ']', '@'])
        && Path::new(token)
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
    if !plain {
        return None;
    }
    let file = token.rsplit('/').next().unwrap_or(token);
    let extension = file.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty()
            && (2..=6).contains(&ext.len())
            && ext.starts_with(|c: char| c.is_ascii_alphabetic())
            && ext.chars().all(|c| c.is_ascii_alphanumeric())
    });
    (token.contains('/') || extension).then(|| token.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_comes_from_the_front_matter() {
        let ok = "---\nname: my-commit\ndescription: >\n  Commits.\ntags:\n  - a\n---\nBody\n";
        assert_eq!(skill_name(ok).unwrap(), "my-commit");
        assert_eq!(skill_name("---\nname: \"my pr\"\n---\n").unwrap(), "my pr");
        for (bad, why) in [
            ("No front matter", "it must start with a --- line"),
            ("---\nname: x\n", "it has no closing --- line"),
            ("---\nname my-commit\n---\n", "line 2 isn't key: value"),
            (
                "---\n  indented: x\nname: x\n---\n",
                "line 2 continues no key",
            ),
            ("---\nname: \"x\n---\n", "line 2 has an unclosed quote"),
        ] {
            assert_eq!(
                skill_name(bad).unwrap_err(),
                format!("SKILL.md's front matter can't be read: {why}"),
                "{bad}"
            );
        }
        assert_eq!(
            skill_name("---\ndescription: x\n---\n").unwrap_err(),
            "SKILL.md's front matter has no name:"
        );
    }

    #[test]
    fn mentions_are_paths_in_code_spans_and_links() {
        let text = "Read [the style](reference.md#top) first. Run `bash scripts/check-staged.sh`, \
                    then `git commit -m \"<area>: <what changed>\"`. See `notes: sort by date`, \
                    https://example.com/a.md, `$HOME/x.sh`, `/abs/x.sh`, `../up.md`, `e.g.`.\n\
                    ```sh\n./scripts/check-staged.sh\n```\n";
        assert_eq!(mentions(text), ["scripts/check-staged.sh", "reference.md"]);
    }

    #[test]
    fn the_body_starts_after_the_front_matter() {
        assert_eq!(body("---\nname: `a.md`\n---\nRun `b.sh`\n"), "Run `b.sh`\n");
    }
}
