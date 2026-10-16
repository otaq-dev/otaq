# Filesystem- and container-level isolation for parallel coding agents

Research date: 2026-10-16. Scope: cheap per-task copies of a repo *including build output* (node_modules, Rust target/, venvs) on macOS / Windows / Linux, zero-config, no admin ideally.

## 1. Copy-on-write file clones (APFS clonefile, Linux reflinks, Windows ReFS block cloning / Dev Drive)

### Takeaway
CoW cloning is the most practical "instant copy of repo + build output" technique, but only where the filesystem supports it. macOS: yes by default (APFS), no admin. Linux: only on btrfs/XFS (Fedora, openSUSE, RHEL family); Ubuntu/Debian users on ext4 get a full copy. Windows: only on a Dev Drive (ReFS) on Windows 11 24H2+, which the user must create with admin rights; the C: drive (NTFS) can never be one. Clone time scales with file *count*, not bytes, so a tree with hundreds of thousands of files still takes seconds to minutes.

### Cited Findings
**macOS / APFS**
- `cp -Rc` clones a directory tree on APFS; each clone uses extra disk space only once written. Used by people to give each git worktree its own `node_modules` — [Atomic Object, "Using Git Worktrees with Many Untracked Files"](https://spin.atomicobject.com/git-worktrees-untracked-files/)
- Clones only share data within the same volume; copying to another volume copies all data — [Eclectic Light, 2025-04-07](https://eclecticlight.co/2025/04/07/how-robust-are-apfs-clone-and-sparse-files/)
- `cp -c -R` and Finder clone files one by one (a folder full of file clones, not a directory clone). A custom script calling `clonefile(2)` directly on a directory was reported faster — [Michael Tsai blog, 2026-05-14](https://mjtsai.com/blog/?p=51918)
- The `clonefile(2)` man page "strongly discourages" cloning directories and recommends `copyfile(3)`. Apple engineer Kevin Elliott explains that a directory clone runs the whole copy in the kernel and blocks changes to the source tree during the operation. If that stalls critical locations long enough, the kernel may give up and panic — [Apple Developer Forums thread 784446](https://developer.apple.com/forums/thread/784446), summarized at [mjtsai.com](https://mjtsai.com/blog/?p=51918)
- Data point (old, ~2016): cloning about 77,000 files (under 2 GB) took just under 2 minutes on APFS. That is faster than an HFS+ copy but far from instant. Shows per-file metadata cost dominates — [Apple Developer Forums thread 49199](https://developer.apple.com/forums/thread/49199)
- Prior art: **Enki** (multi-agent tool) gives each worker a full copy of the project, including build artifacts, node_modules, gitignored files and .git, at `.enki/copies/<task_id>`. It uses `cp -Rc` (clonefile) on macOS, `cp --reflink=auto -a` on Linux, and plain `cp -a` elsewhere. Reported cost on APFS/btrfs is about 1 MB of metadata at creation plus changed blocks; on ext4, a full copy (10 GB example). Copies come from the live working tree, then a task branch is created inside the copy. Its docs do not address absolute paths baked into build output — [Enki docs: Worker isolation](https://mintlify.wiki/korbindeman/enki/concepts/worker-isolation)
- Other tools doing the same: `cow-cli` (wraps `cp -rc`, clonefile per file) — [docs.rs cow-cli](https://docs.rs/cow-cli); `wt-cli` (symlinks heavy dirs like node_modules *or* reflink-copies per-worktree files on APFS/btrfs/XFS) — [github.com/bkildow/wt-cli](https://github.com/bkildow/wt-cli)

**Linux**
- Fedora desktop editions default to Btrfs since Fedora 33. openSUSE uses Btrfs for `/` and XFS for extra disks. RHEL 8 family (Rocky, Alma) default to XFS. Red Hat removed Btrfs from RHEL 8 — [Phoronix](https://www.phoronix.com/news/Fedora-33-Btrfs-Desktop-Approve), [Wikipedia: List of default file systems](https://en.wikipedia.org/wiki/List_of_default_file_systems)
- With `cp --reflink=auto`, ext4 silently falls back to a full copy — [Enki docs](https://mintlify.wiki/korbindeman/enki/concepts/worker-isolation)

**Windows**
- Dev Drive = a ReFS volume tuned for dev work. Requirements: Windows 11 build 22621.2338+, **local administrator permissions**, minimum 50 GB, 8 GB RAM (16 recommended). It can live in a VHD/VHDX file (dynamically expanding allowed), a resized partition, or unallocated space. **The C: drive cannot be a Dev Drive**, and existing volumes cannot be converted. Enterprises may need a Group Policy to allow it — [Microsoft Learn: Set up a Dev Drive (updated 2026-06-01)](https://learn.microsoft.com/en-us/windows/dev-drive/)
- "Beginning in Windows 11 24H2 & Windows Server 2025, Block cloning is now supported on Dev Drive ... free performance benefits whenever you copy a file using Dev Drive" — [Microsoft Learn: Dev Drive](https://learn.microsoft.com/en-us/windows/dev-drive/)
- Microsoft engineering blog: CoW was added to the Win32 **CopyFile** API family for Dev Drive and ReFS, so build tools need no changes. It is "on by default for Dev Drive in the 24H2" release wave. Measured on a large C# repo: build time −32% from Dev Drive alone, −42% with CoW — [Engineering@Microsoft, Nov 2023](https://devblogs.microsoft.com/engineering-at-microsoft/copy-on-write-in-win32-api-early-access/), [May 2024](https://devblogs.microsoft.com/engineering-at-microsoft/copy-on-write-performance-and-debugging/)
- Microsoft's CopyOnWrite .NET library warns that if cloned files are opened for append or random-access write, filling in their original content later can cause out-of-disk-space errors — [NuGet CopyOnWrite](https://www.nuget.org/packages/CopyOnWrite)
- Microsoft-reported numbers (May 2024, as relayed by press): a 10 GB file copy took 7.96 s on NTFS vs 0.64 s with block clone. An 18 GB folder of ~5,800 files took 30.9 s vs 6.3 s — [Landian news](https://landian.news/article/1912.html), [Gigazine](https://gigazine.net/gsc_news/en/20241004-windows-11-block-cloning). Secondary sources; small-file gains are much smaller (~18% for 1 MB files per Gigazine).
- Dev Drive filters: by default only antivirus filters attach. ProjFS (`PrjFlt`) and Docker (`bindFlt`, `wcifs`) must be allow-listed with `fsutil devdrv setfiltersallowed` (admin) — [Microsoft Learn: Dev Drive](https://learn.microsoft.com/en-us/windows/dev-drive/)

### Inferences
- CoW cloning on Windows only helps users who already have a Dev Drive. Otaq could detect one and use it (clone via CopyFile2 / `FSCTL_DUPLICATE_EXTENTS_TO_FILE`) but cannot create one without admin, and it cannot be the default location (C:). Most Windows users will fall back to full copies or a shared-cache approach.
- Disk cost of CoW clones starts near zero, but **grows as each copy rebuilds**. For example, a Rust `cargo build` in a cloned `target/` rewrites the workspace crates' artifacts (often GBs), so 15 tickets in Review each hold their own diverged blocks. CoW saves the shared dependency artifacts, not the rebuilt ones.
- For a 36 GB Rust target/ with hundreds of thousands of files, expect a clone to take tens of seconds to minutes (it scales with file count, not bytes). That is still far better than a rebuild but not "instant." No modern benchmark at that scale was found.
- A clone of a *working tree* also clones `.git` if it's a full checkout. Cloning a worktree's ignored dirs on top of `git worktree add` is the hybrid most tools use.

### Gaps
- No 2025–2026 benchmark of `cp -c` or clonefile on a node_modules or large Rust target tree (hundreds of thousands of files) was found.
- Not confirmed from a primary source whether `robocopy` uses block cloning on Dev Drive. CopyFile/CopyFile2 do, per Microsoft's blog.
- bcachefs reflink status not researched. It was marked "externally maintained" and removed from the mainline kernel in 2025 (from memory, unverified here).
- Ubuntu's default (ext4) is not confirmed by a fetched source in this session; the Wikipedia list covers it.

## 2. Overlay / union filesystems (OverlayFS, fuse-overlayfs, FUSE-T/macFUSE/FSKit, ProjFS, WinFsp)

### Takeaway
A per-task overlay on top of the main checkout plus its build output is feasible on Linux without root, through user namespaces and kernel ≥5.11. Ubuntu 23.10+ AppArmor restrictions complicate this, and the agent processes would have to run inside that namespace. On macOS and Windows there is no built-in union filesystem. Every option needs a third-party driver or extension (macFUSE, FUSE-T, WinFsp) or a custom ProjFS/FSKit provider that Otaq would have to write and maintain, and several of these need admin or user approval. Not zero-config.

### Cited Findings
- The Linux 5.11 overlayfs update allowed unprivileged mounting inside a user namespace — [LKML pull request for 5.11](https://lkml.iu.edu/2012.2/01416.html). As a result, fuse-overlayfs is not required for rootless Podman on kernels ≥5.11 — [Arch bug FS#71358](https://bugs.archlinux.org/task/71358)
- fuse-overlayfs can be used by unprivileged users in a user namespace since Linux 4.18 — [fuse-overlayfs man page](https://man.archlinux.org/man/fuse-overlayfs.1.en). Apptainer recommends kernel ≥5.11 for unprivileged overlay and falls back to fuse-overlayfs otherwise — [Apptainer admin docs](https://apptainer.org/docs/admin/1.4/user_namespace.html)
- Ubuntu 24.04 restricts unprivileged user namespaces by default through AppArmor (introduced in 23.10). Unconfined apps may create a userns, but capabilities inside it are denied unless an AppArmor profile grants `userns` — [Ubuntu 24.04 release notes](https://documentation.ubuntu.com/release-notes/24.04/), [Ubuntu Discourse](https://discourse.ubuntu.com/t/understanding-apparmor-user-namespace-restriction/58007)
- **FSKit** (Apple's user-space filesystem framework) is available from macOS 15.4. Modules ship as app extensions — [Apple FSKit docs](https://developer.apple.com/documentation/fskit)
- macFUSE's FSKit backend runs file systems fully in user space on macOS 26. It is opt-in with `-obackend=fskit`; the kernel extension stays the default because FSKit is "not on par" with the kernel VFS API — [MacRumors forum on macFUSE](https://forums.macrumors.com/threads/macfuse-veracrypt.2485302/)
- FUSE-T avoids kernel extensions by translating FUSE into a local NFSv4 server mounted by macOS's NFS client. Its FSKit backend needs macOS 26+ and is enabled in System Settings — [FUSE-T GitHub](https://github.com/macos-fuse-t/fuse-t), [rclone forum](https://forum.rclone.org/t/macos-rclone-mount-with-fuse-t-via-fskit/53608)
- FSKit gaps per developers: no extended-attribute ops, no inode-forget notifications, no readdirplus. Filesystem extensions still require interactive user approval before mounting, even on CI runners — [Apple forum thread 838060](https://developer.apple.com/forums/thread/838060), [thread 831338](https://origin-devforums.apple.com/forums/thread/831338)
- **ProjFS** (Projected File System, used by VFS for Git) is an optional Windows component (Windows 10 1809+). It is enabled with `Enable-WindowsOptionalFeature -Online -FeatureName Client-ProjFS` from an **elevated** prompt — [Microsoft.Windows.ProjFS NuGet / docs](https://www.nuget.org/packages/Microsoft.Windows.ProjFS), [Huntress blog](https://huntress.com/blog/windows-projected-file-system-mechanics). Rust bindings exist: [`projfs` crate](https://docs.rs/crate/projfs/latest), [`windows-projfs`](https://www.lib.rs/crates/windows-projfs)
- On a Dev Drive, ProjFS's filter `PrjFlt` must be explicitly allowed (admin) — [Microsoft Learn: Dev Drive](https://learn.microsoft.com/en-us/windows/dev-drive/)

### Inferences
- ProjFS is not a union filesystem. It projects a virtual tree from a provider process and *hydrates* files on first access into real NTFS files, so a "copy" of target/ fills up with real files as the build reads them. A CoW-like overlay would have to be built by Otaq as a provider. That is a substantial project, needs admin once to enable the feature, and any crash of the provider breaks the tree.
- Linux overlay has a semantic problem for builds: the lower layer (main checkout + its target/) must not change while overlays sit on it for days. With 10–15 tickets in Review for days, that means freezing a base snapshot per epoch, which in turn needs a copy anyway.
- Overlays still don't fix path-baked artifacts (section 5): each overlay is mounted at a different path unless mount namespaces are used to mount it at the *same* path per agent. That trick (same absolute path per task, different content) is the strongest argument for namespaces/containers on Linux.
- On macOS a FUSE-T/FSKit overlay would be NFS- or extension-backed: slower metadata ops (bad for builds with many small files) and a system-extension approval prompt. Not zero-config.

### Gaps
- No dev tool was found that ships per-task overlays on macOS/Windows for coding agents. `bindfs`, Nix and Bazel sandboxing were not researched in depth. Bazel's sandbox uses symlink forests/hardlinks into a per-action execroot (from memory, not verified here).
- WinFsp was not researched in this session. It is a third-party driver install that needs admin (unverified).
- Performance of fuse-overlayfs or kernel overlayfs for large Rust builds is not measured.

## 3. Filesystem / volume snapshots (APFS, btrfs, ZFS)

### Takeaway
btrfs subvolume snapshots are the only snapshot mechanism ordinary users can use without root: creating them is allowed, but deleting needs the `user_subvol_rm_allowed` mount option, or rmdir on kernel ≥4.18. APFS snapshots are volume-wide and need privileges or Time Machine-style APIs, so they can't copy a single directory. ZFS is rare on laptops.

### Cited Findings
- On btrfs, normal users may create subvolumes and snapshots. Deleting them requires root, or a mount with `user_subvol_rm_allowed` — [btrfs-subvolume(8)](https://dokk.org/manpages/debian/12/btrfs-progs/btrfs-subvolume.8.en), [btrfs home dir snapshots](https://git.tdem.in/joergmlpts/btrfs-home-directory-snapshots)
- Since kernel 4.18 an unprivileged user can delete an (empty) subvolume via rmdir; containers/storage uses this — [Moby commit](https://git.causa-arcana.com/kotovalexarian-likes-github/moby--moby/commit/62b5194f627571c019907edb11bb7bc2edd59933)
- Read-only snapshots owned by a user may still not be removable without extra steps — [UrBackup forum](https://forums.urbackup.org/t/podman-rootless-urbackup-server/16804)
- Tsai doubts that APFS supports directory-level clones except at the snapshot level, which is volume-wide — [mjtsai.com](https://mjtsai.com/blog/?p=51918)

### Inferences
- btrfs snapshots only work if the repo dir is itself a subvolume. Otaq would need to convert it (move + `btrfs subvolume create` + copy back) or ask the user to. They are constant-time regardless of file count, much faster than per-file reflinks for huge target dirs. Fedora/openSUSE users only.
- APFS snapshots are not useful for per-directory copies by an unprivileged app. clonefile is the macOS answer.

### Gaps
- No primary source fetched on APFS snapshot privileges (tmutil / `fs_snapshot_create` entitlement). Stated from general knowledge.
- ZFS-on-laptop prevalence not researched.

## 4. Containers / devcontainers with shared cache volumes

### Takeaway
Containers solve the "same path in every copy" problem: every task can see its repo at `/workspace`. But on macOS and Windows they run in a Linux VM, so bind-mounted source trees with many small files are slow. They need Docker Desktop/Podman/Apple container installed. Builds target Linux, not the host OS (a macOS Tauri app can't be built or run in one). Stack-specific images go against zero-config. Dagger's container-use is a working agent-focused example.

### Cited Findings
- Dagger **container-use**: an MCP server that gives each agent its own container plus git branch/worktree ("environments"). It works with Claude Code via `claude mcp add container-use -- cu stdio`; you review work by checking out the agent's branch. It was described as early development — [Dagger blog](https://dagger.io/blog/agent-container-use), [InfoQ Aug 2025](https://www.infoq.com/news/2025/08/container-use), [container-use.com](https://container-use.com)
- Apple `container`: runs each Linux container in its own lightweight VM. Apple silicon only; built for macOS 26 (one source says macOS 15 with reduced networking). Announced WWDC June 2025; 1.0.0 reported released 2026-06-09 — [The Register 2025-06-10](https://www.theregister.com/2025/06/10/apple_tries_to_contain_itself/), [InfoQ](https://www.infoq.com/news/2025/06/apple-container-linux), [itdaily](https://itdaily.com/news/network/apple-brings-linux-containers-to-macos-26-on-apple-silicon/), [pasqualepillitteri.it (1.0 claim)](https://pasqualepillitteri.it/en/news/6007/apple-container-linux-containers-mac-no-docker-desktop)
- Docker Desktop on macOS: every bind-mount file op crosses the host↔VM sharing layer. For node_modules with ~100k files that adds up to seconds or minutes. VirtioFS has been the default since Docker Desktop 4.22 and is faster, but the usual advice is still to keep node_modules in a named volume — [techearl](https://techearl.com/docker-bind-mounts-slow-mac-windows), [Jeff Geerling on VirtioFS](https://jeffgeerling.com/node/3193)
- Docker on a Dev Drive needs `bindFlt`, `wcifs` filters allow-listed — [Microsoft Learn: Dev Drive](https://learn.microsoft.com/en-us/windows/dev-drive/)

### Inferences
- For Otaq: containers fit Linux-targeted web/backend projects but not desktop apps or iOS/macOS/Windows-native builds, and they require a heavy third-party install. Shared cache volumes (cargo registry, npm store, pip cache) are a good fit, but per-task build output still lives per container.
- Docker Desktop requires a paid subscription for larger companies (from memory: >250 employees or >$10M revenue; not verified in this session).

### Gaps
- Docker Desktop licensing terms for 2025–2026 not verified.
- No numbers on disk/RAM cost of 10–15 idle containers kept "runnable" for days.

## 5. Path-baked artifacts: what breaks when a build dir is copied to another path

### Takeaway
Copying build output to a new path mostly works for dependency artifacts, but partly breaks or forces rebuilds for things that hold absolute paths. Python venvs break outright unless created relocatable. Cargo reuses registry-dependency artifacts, but stores absolute paths in fingerprints/dep-info and depends on mtimes, so workspace crates (and possibly more) rebuild once. CMake build dirs cannot be moved. node_modules is mostly relative, with pnpm's links pointing into an absolute global store.

### Cited Findings
**Python venv**
- Moving a venv breaks it: shebangs, `pyvenv.cfg`, and activation scripts keep the old absolute path — [venv-relocate on PyPI](https://pypi.org/project/venv-relocate/), [discuss.python.org: What stops a venv from being relocatable?](https://discuss.python.org/t/q-what-stops-a-venv-from-being-relocatable/57166)
- uv user report: after moving a project, a stale shebang in `.venv/bin/pytest` silently ran a *system* pytest. `uv sync` did not detect it; deleting `.venv` fixed it — [uv issue #10895 via goodfirstissue](https://www.goodfirstissue.org/astral-sh/uv/issues/10895)
- `uv venv --relocatable` writes `relocatable = true` to pyvenv.cfg so entry points and activation scripts use relative paths. The guarantee covers only standard console/GUI scripts; other binaries are left as-is — [uv commit](https://app.semanticdiff.com/gh/astral-sh/uv/commit/cb47aed9dea568811c6c5dd526574eb3cae739b0), [uv CLI docs mirror](https://mintlify.com/astral-sh/uv/cli/venv)

**Rust / cargo**
- Cargo's fingerprint code computes the absolute target dir and rewrites stored paths *relative to it*. Build-script output dirs include a `root-output` file "to handle the target directory being moved" — [cargo fingerprint::target_root docs](https://doc.rust-lang.org/stable/nightly-rustc/cargo/core/compiler/fingerprint/fn.target_root.html), [cargo layout docs](https://doc.rust-lang.org/stable/nightly-rustc/cargo/core/compiler/layout/index.html)
- The metadata (filename) hash uses `stable_hash(ws.root())`, which strips the workspace-root prefix from path package IDs, so artifact *names* don't change when the workspace moves. In PR #14951 cargo deliberately keeps an absolute path in the *fingerprint* (not metadata) for sysroot crates "so moving the rustc installation triggers a rebuild" — [cargo compilation_files.rs](https://doc.rust-lang.org/stable/nightly-rustc/src/cargo/compiler/build_runner/compilation_files.rs.html), [cargo PR #14951](https://github.com/rust-lang/cargo/pull/14951)
- Cargo relies on file mtimes for invalidation. Moving or copying sources to another directory "will also trigger large rebuilds ... due to changing paths and updated mtimes, even with a cache being available" — [cargo-mtime README](https://rust-digger.code-maven.com/crates/cargo-mtime); see also [Rust internals: less dependency on correct mtimes](https://internals.rust-lang.org/t/pr-less-dependency-on-correct-mtimes/12976)
- rustc has a known class of incremental-compilation ICEs when the source dir moves (test `moved-src-dir-fingerprint-ice`) — [rust tests](https://rust.googlesource.com/rust/+/refs/heads/perf-tmp/tests/run-make/moved-src-dir-fingerprint-ice/rmake.rs)

**CMake**
- CMake build directories are not relocatable. There is a long-standing feature request to make CMake survive the build dir moving — [Kitware issue #20473](https://gitlab.kitware.com/cmake/cmake/-/issues/20473), [CMakeHelpers](https://github.com/etcwilde/CMakeHelpers)

**node_modules**
- pnpm: every file in node_modules is a hard link into a global content-addressable store, and the tree structure is built with symlinks. Across filesystems pnpm falls back to copying — [pnpm: symlinked node_modules structure](https://pnpm.io/symlinked-node-modules-structure), [pnpm motivation](https://pnpm.io/motivation)

### Inferences
- **Cargo:** a CoW clone preserves mtimes (`cp -a`/clonefile keep them), so registry dependencies should stay fresh. Workspace crates likely rebuild because their dep-info lists absolute source paths under the old checkout, and absolute paths also land in debuginfo, panic messages, and `env!("CARGO_MANIFEST_DIR")`. Net effect: one incremental-ish rebuild of workspace crates per copy, not a full rebuild. Verify empirically: clone a target/ and run `CARGO_LOG=cargo::core::compiler::fingerprint=info cargo build`. `--remap-path-prefix` only changes paths embedded in outputs, not cargo's freshness checks. A shared `CARGO_TARGET_DIR` across tasks avoids copying but serialises builds on cargo's lock and makes tasks invalidate each other.
- **node_modules (npm/yarn classic):** mostly relative; `.bin` entries are relative symlinks (POSIX) or shim scripts (Windows) — needs verification. pnpm's hard links into an absolute store survive a copy (a clone of a hard link becomes an independent file, losing the dedupe). Native addons with RPATHs or postinstall-baked absolute paths may break.
- **Next.js `.next`, Gradle, file watchers:** not researched. Watchers (Vite/Next dev servers, cargo-watch) will treat a freshly cloned tree as new and do a full initial scan, which is fine.
- **Same-path trick:** if every task saw its copy at an identical absolute path (mount namespace on Linux, container, or a swap like `/otaq/active` symlink), all path-baked issues disappear. That is only cheap on Linux and in containers.

### Gaps
- No primary-source confirmation of exactly which cargo units rebuild after copying a target/ to another checkout path. Needs a local experiment.
- Next.js `.next` cache relocatability, Gradle build dirs/configuration cache, and npm `.bin` on Windows were not researched (tool-call budget).
- Measured disk growth per task after rebuild not found.
