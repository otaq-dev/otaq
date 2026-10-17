//! Copy-on-write cloning and the clone-capable probe (T-0295, WRM-01, d0082). Every tree is a fake one written by the
//! test. Clone tests skip with a note where the temporary folder's disk can't clone (ext4, NTFS), since nothing is
//! ever copied instead.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use otaq_core::data_dir::DataDir;
use otaq_core::warm::clone::{
    CloneError, CloneProbe, CloneSupport, clone_tree, move_tree, move_tree_or_back, probe,
};

/// 2001-09-09, well before any file the test writes.
fn old_time() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000)
}

fn set_mtime(path: &Path, time: SystemTime) {
    let times = fs::FileTimes::new().set_modified(time).set_accessed(time);
    #[cfg(windows)]
    let file = {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_BACKUP_SEMANTICS, so a folder opens too.
        fs::OpenOptions::new()
            .write(true)
            .custom_flags(0x0200_0000)
            .open(path)
            .unwrap()
    };
    #[cfg(not(windows))]
    let file = fs::File::open(path).unwrap();
    file.set_times(times).unwrap();
}

fn symlink(target: &str, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    // Windows makes symlinks only with Developer Mode or as an administrator; the tree goes without one then. Its
    // links resolve only with `\` separators.
    #[cfg(windows)]
    if let Err(err) = std::os::windows::fs::symlink_file(target.replace('/', "\\"), link) {
        eprintln!("no symlink in the test tree: {err}");
    }
}

/// A temporary folder, or `None` (with a printed note) when its disk can't clone.
fn clone_capable_temp() -> Option<tempfile::TempDir> {
    let tmp = tempfile::tempdir().unwrap();
    let support = probe(&DataDir::new(tmp.path().join("data")));
    if support.available {
        Some(tmp)
    } else {
        eprintln!(
            "skipped: the temporary folder's disk ({} on {:?}) can't clone",
            support.file_system, support.volume
        );
        None
    }
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// Every path under `root`, relative and sorted.
fn listing(root: &Path) -> Vec<PathBuf> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            out.push(path.strip_prefix(root).unwrap().to_owned());
            if fs::symlink_metadata(&path).unwrap().is_dir() {
                walk(root, &path, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// A fake build output: nested folders, an executable, a symlink, all with old times.
fn build_tree(root: &Path) {
    write(&root.join("out/deep/nested/obj.bin"), "object");
    write(&root.join("out/deep/info.txt"), "info");
    write(&root.join("tools/run.sh"), "#!/bin/sh\necho hi\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root.join("tools/run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    symlink("deep/info.txt", &root.join("out/link"));
    // Files first, then folders from the inside out, so writing a child doesn't refresh a folder's time.
    let mut paths = listing(root);
    paths.reverse();
    for rel in paths {
        if !fs::symlink_metadata(root.join(&rel)).unwrap().is_symlink() {
            set_mtime(&root.join(rel), old_time());
        }
    }
    set_mtime(root, old_time());
}

fn assert_same(from: &Path, to: &Path) {
    let from_meta = fs::symlink_metadata(from).unwrap();
    let to_meta = fs::symlink_metadata(to).unwrap();
    assert_eq!(from_meta.file_type(), to_meta.file_type(), "{to:?}");
    if from_meta.is_symlink() {
        assert_eq!(fs::read_link(from).unwrap(), fs::read_link(to).unwrap());
        return;
    }
    assert_eq!(
        from_meta.modified().unwrap(),
        to_meta.modified().unwrap(),
        "{to:?}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            from_meta.permissions().mode(),
            to_meta.permissions().mode(),
            "{to:?}"
        );
    }
    if from_meta.is_file() {
        assert_eq!(fs::read(from).unwrap(), fs::read(to).unwrap());
    }
}

#[test]
fn a_tree_clones_with_its_layout_times_modes_and_symlinks() {
    let Some(tmp) = clone_capable_temp() else {
        return;
    };
    let from = tmp.path().join("from");
    build_tree(&from);
    let to = tmp.path().join("new/parent/to");

    clone_tree(&from, &to, None).unwrap();

    assert_eq!(listing(&from), listing(&to));
    assert_same(&from, &to);
    for rel in listing(&from) {
        assert_same(&from.join(&rel), &to.join(&rel));
    }
    if fs::symlink_metadata(from.join("out/link")).is_ok() {
        assert!(
            fs::symlink_metadata(to.join("out/link"))
                .unwrap()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(to.join("out/link")).unwrap(), "info");
    }
    // The clone is its own: writing to it leaves the source as it was.
    fs::write(to.join("out/deep/info.txt"), "changed").unwrap();
    assert_eq!(
        fs::read_to_string(from.join("out/deep/info.txt")).unwrap(),
        "info"
    );
}

#[test]
fn only_clones_the_listed_paths_into_an_existing_folder() {
    let Some(tmp) = clone_capable_temp() else {
        return;
    };
    let from = tmp.path().join("from");
    build_tree(&from);
    write(&from.join("src/main.txt"), "tracked");
    write(&from.join("top.log"), "log");
    let to = tmp.path().join("worktree");
    write(&to.join("src/main.txt"), "checked out");

    let only = [
        PathBuf::from("out/deep"),
        PathBuf::from("top.log"),
        PathBuf::from("tools/run.sh"),
    ];
    clone_tree(&from, &to, Some(&only)).unwrap();

    let expected: Vec<PathBuf> = [
        "out",
        "out/deep",
        "out/deep/info.txt",
        "out/deep/nested",
        "out/deep/nested/obj.bin",
        "src",
        "src/main.txt",
        "tools",
        "tools/run.sh",
        "top.log",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    assert_eq!(listing(&to), expected);
    assert_eq!(
        fs::read_to_string(to.join("src/main.txt")).unwrap(),
        "checked out"
    );
    for rel in [
        "out/deep",
        "out/deep/nested/obj.bin",
        "top.log",
        "tools/run.sh",
    ] {
        assert_same(&from.join(rel), &to.join(rel));
    }
}

#[test]
fn a_clone_that_fails_halfway_leaves_nothing() {
    let Some(tmp) = clone_capable_temp() else {
        return;
    };
    let from = tmp.path().join("from");
    build_tree(&from);
    let to = tmp.path().join("worktree");
    write(&to.join("README"), "checked out");

    // The first path clones, then the second is missing: the first and its new parents are removed again.
    let only = [
        PathBuf::from("out/deep/nested"),
        PathBuf::from("missing/dir"),
    ];
    let err = clone_tree(&from, &to, Some(&only)).unwrap_err();
    assert!(matches!(err, CloneError::Io { .. }), "{err}");
    assert_eq!(listing(&to), vec![PathBuf::from("README")]);

    // A whole-folder clone into new parents removes the parents too.
    let missing = tmp.path().join("nothing-here");
    let to_new = tmp.path().join("a/b/c");
    assert!(clone_tree(&missing, &to_new, None).is_err());
    assert!(!tmp.path().join("a").exists());

    // A destination that exists is refused and left alone.
    let err = clone_tree(&from, &to, None).unwrap_err();
    assert!(
        matches!(&err, CloneError::Io { error, .. } if error.kind() == std::io::ErrorKind::AlreadyExists)
    );
    assert_eq!(listing(&to), vec![PathBuf::from("README")]);

    // A path outside the folder is refused.
    assert!(clone_tree(&from, &to, Some(&[PathBuf::from("../from")])).is_err());
    assert_eq!(listing(&to), vec![PathBuf::from("README")]);
}

#[test]
fn a_read_only_folder_that_was_cloned_is_removed_on_failure() {
    let Some(tmp) = clone_capable_temp() else {
        return;
    };
    let from = tmp.path().join("from");
    build_tree(&from);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            from.join("out/deep/nested"),
            fs::Permissions::from_mode(0o555),
        )
        .unwrap();
    }
    let to = tmp.path().join("to");
    let only = [PathBuf::from("out"), PathBuf::from("missing")];
    assert!(clone_tree(&from, &to, Some(&only)).is_err());
    assert!(!to.exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            from.join("out/deep/nested"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
}

#[test]
fn move_tree_renames_the_listed_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let worktree = tmp.path().join("worktree");
    build_tree(&worktree);
    write(&worktree.join("src/main.txt"), "tracked");
    let incoming = tmp.path().join("warm/shop-web.incoming");

    let only = [PathBuf::from("out/deep/nested"), PathBuf::from("tools")];
    let before = fs::symlink_metadata(worktree.join("out/deep/nested/obj.bin")).unwrap();
    move_tree(&worktree, &incoming, Some(&only)).unwrap();

    assert!(!worktree.join("out/deep/nested").exists());
    assert!(!worktree.join("tools").exists());
    assert!(worktree.join("out/deep/info.txt").exists());
    assert!(worktree.join("src/main.txt").exists());
    let moved = fs::symlink_metadata(incoming.join("out/deep/nested/obj.bin")).unwrap();
    assert_eq!(before.modified().unwrap(), moved.modified().unwrap());
    assert_eq!(
        fs::read_to_string(incoming.join("tools/run.sh")).unwrap(),
        "#!/bin/sh\necho hi\n"
    );

    // A destination that exists is an error, and nothing moves.
    write(&incoming.join("src/main.txt"), "other");
    let err = move_tree(&worktree, &incoming, Some(&[PathBuf::from("src")])).unwrap_err();
    assert!(
        matches!(&err, CloneError::Io { error, .. } if error.kind() == std::io::ErrorKind::AlreadyExists)
    );
    assert_eq!(
        fs::read_to_string(worktree.join("src/main.txt")).unwrap(),
        "tracked"
    );

    // Without `only`, the whole folder moves.
    let warm = tmp.path().join("warm/shop-web");
    move_tree(&incoming, &warm, None).unwrap();
    assert!(!incoming.exists());
    assert!(warm.join("tools/run.sh").exists());
}

#[test]
fn move_tree_or_back_moves_everything_or_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let worktree = tmp.path().join("worktree");
    build_tree(&worktree);
    let incoming = tmp.path().join("warm/shop-web.incoming");
    // The second path's destination is there already: the first, moved by then, goes back.
    write(&incoming.join("tools/run.sh"), "in the way");
    let only = [PathBuf::from("out/deep/nested"), PathBuf::from("tools")];
    let before = fs::symlink_metadata(worktree.join("out/deep/nested/obj.bin")).unwrap();

    let err = move_tree_or_back(&worktree, &incoming, Some(&only)).unwrap_err();

    assert!(
        matches!(&err, CloneError::Io { error, .. } if error.kind() == std::io::ErrorKind::AlreadyExists),
        "{err}"
    );
    let back = fs::symlink_metadata(worktree.join("out/deep/nested/obj.bin")).unwrap();
    assert_eq!(before.modified().unwrap(), back.modified().unwrap());
    assert!(worktree.join("tools/run.sh").exists());
    // The parent folders it made for the first path are gone again; what was there stays.
    assert!(!incoming.join("out").exists());
    assert_eq!(
        fs::read_to_string(incoming.join("tools/run.sh")).unwrap(),
        "in the way"
    );

    // Out of the way, everything moves.
    fs::remove_dir_all(incoming.join("tools")).unwrap();
    move_tree_or_back(&worktree, &incoming, Some(&only)).unwrap();
    assert!(!worktree.join("out/deep/nested").exists() && !worktree.join("tools").exists());
    assert!(incoming.join("out/deep/nested/obj.bin").exists());
    assert!(incoming.join("tools/run.sh").exists());
    // A path outside the folder is refused before anything moves.
    let outside = [PathBuf::from("out/deep/info.txt"), PathBuf::from("../x")];
    assert!(move_tree_or_back(&worktree, &incoming, Some(&outside)).is_err());
    assert!(worktree.join("out/deep/info.txt").exists());
}

/// Another disk: `CLONE_PROBE_DIR` when set, else the build's temporary folder, which is on another disk than the
/// system's temporary folder on some machines (the test says so and passes where it isn't).
#[test]
fn a_move_to_another_disk_is_unsupported_and_leaves_the_source() {
    let other = std::env::var_os("CLONE_PROBE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")));
    fs::create_dir_all(&other).unwrap();
    let there = tempfile::tempdir_in(&other).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let marker = tmp.path().join("same-disk");
    write(&marker, "x");
    if fs::rename(&marker, there.path().join("same-disk")).is_ok() {
        eprintln!(
            "skipped: {} and {} are on the same disk",
            tmp.path().display(),
            other.display()
        );
        return;
    }
    let from = tmp.path().join("from");
    build_tree(&from);
    let to = there.path().join("moved");
    let only = [PathBuf::from("tools"), PathBuf::from("out")];

    let moved = move_tree_or_back(&from, &to, Some(&only));

    assert!(
        matches!(moved, Err(CloneError::Unsupported { .. })),
        "{moved:?}"
    );
    assert!(from.join("tools/run.sh").exists() && from.join("out/deep/info.txt").exists());
    assert!(!to.exists(), "nothing is left on the other disk");
}

#[test]
fn the_probe_answers_for_the_test_disk_and_cleans_up() {
    let tmp = tempfile::tempdir().unwrap();
    let data = DataDir::new(tmp.path().join("data"));
    let support = probe(&data);
    eprintln!("probe: {support:?}");

    assert!(!data.root().join("warm/.probe").exists());
    assert!(data.root().join("warm").is_dir());
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    {
        assert!(!support.file_system.is_empty(), "{support:?}");
        assert!(!support.volume.is_empty(), "{support:?}");
    }
    // The probe agrees with a real clone on the same disk.
    let file = tmp.path().join("file");
    fs::write(&file, "x").unwrap();
    assert_eq!(
        support.available,
        clone_tree(&file, &tmp.path().join("copy"), None).is_ok()
    );
    if cfg!(target_os = "macos") && support.file_system == "apfs" {
        assert!(support.available);
    }
}

#[test]
fn a_forced_probe_answers_without_probing() {
    let tmp = tempfile::tempdir().unwrap();
    let data = DataDir::new(tmp.path().join("data"));
    let exfat = CloneSupport {
        available: false,
        file_system: "exfat".to_owned(),
        volume: "/Volumes/USB".to_owned(),
    };
    let forced = CloneProbe::forced(exfat.clone());
    assert_eq!(forced.support(&data), &exfat);
    assert!(!data.root().exists());

    // An unforced one probes once and keeps the answer.
    let kept = CloneProbe::new();
    let first = kept.support(&data).clone();
    assert_eq!(
        kept.support(&DataDir::new(tmp.path().join("elsewhere"))),
        &first
    );
    assert!(!tmp.path().join("elsewhere").exists());
}

/// A hand check on another disk, e.g. an exFAT disk image: run this ignored test with `CLONE_PROBE_DIR=/Volumes/EXFAT`.
/// Prints the probe there, and checks that cloning and moving from the temporary folder into it are refused, never
/// copied.
#[test]
#[ignore = "needs CLONE_PROBE_DIR pointing at a folder on another disk"]
fn probe_and_refusals_on_another_disk() {
    let dir = PathBuf::from(std::env::var_os("CLONE_PROBE_DIR").expect("CLONE_PROBE_DIR"));
    let support = probe(&DataDir::new(dir.join("data")));
    println!("probe: {support:?}");

    let tmp = tempfile::tempdir().unwrap();
    let from = tmp.path().join("from");
    build_tree(&from);
    let to = dir.join("cloned");
    let cloned = clone_tree(&from, &to, None);
    println!("clone into it: {cloned:?}");
    assert!(matches!(cloned, Err(CloneError::Unsupported { .. })));
    assert!(!to.exists());
    let moved = move_tree(&from, &dir.join("moved"), Some(&[PathBuf::from("out")]));
    println!("move into it: {moved:?}");
    assert!(matches!(moved, Err(CloneError::Unsupported { .. })));
    assert!(from.join("out").exists());
    let moved = move_tree_or_back(
        &from,
        &dir.join("moved"),
        Some(&[PathBuf::from("tools"), PathBuf::from("out")]),
    );
    println!("move all or nothing into it: {moved:?}");
    assert!(matches!(moved, Err(CloneError::Unsupported { .. })));
    assert!(from.join("tools").exists() && from.join("out").exists());
    let _ = fs::remove_dir_all(dir.join("data"));
}
