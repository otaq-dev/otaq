//! Canonical paths that every program understands.
//!
//! On Windows `std::fs::canonicalize` answers with a verbatim path (`\\?\C:\Users\…`), which git and many other
//! programs refuse ("cannot mkdir … Invalid argument"). [`canonical`] gives the ordinary form (`C:\Users\…`)
//! whenever one exists; on macOS and Linux it is `canonicalize` itself.

use std::io;
use std::path::{Path, PathBuf};

/// The absolute path of `path` with links resolved, in its ordinary form.
pub fn canonical(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    std::fs::canonicalize(path).map(|p| ordinary(&p))
}

/// `\\?\C:\x` → `C:\x` and `\\?\UNC\server\share` → `\\server\share`; anything else unchanged.
pub fn ordinary(path: &Path) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path.to_path_buf();
    };
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_prefixes_go() {
        assert_eq!(
            ordinary(Path::new(r"\\?\C:\Users\me")),
            Path::new(r"C:\Users\me")
        );
        assert_eq!(
            ordinary(Path::new(r"\\?\UNC\srv\share\x")),
            Path::new(r"\\srv\share\x")
        );
        assert_eq!(
            ordinary(Path::new(r"\\?\Volume{1}\x")),
            Path::new(r"\\?\Volume{1}\x")
        );
        assert_eq!(ordinary(Path::new("/tmp/x")), Path::new("/tmp/x"));
    }

    #[test]
    fn canonical_resolves_existing_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let p = canonical(tmp.path()).unwrap();
        assert!(p.is_absolute());
        assert!(
            !p.display().to_string().starts_with(r"\\?\"),
            "{}",
            p.display()
        );
    }
}
