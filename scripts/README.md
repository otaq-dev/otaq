# Checks on macOS, Windows and Linux

> References such as "decision 0075", "d0072", "SDD §5.5", ticket numbers (T-0215), requirement IDs (UIT-02)
> and design frames (`crewboard.pen`) point to the maintainer's design notes, which are not part of this repository.

Every commit must pass the same steps on every platform (decision 0056, SDD section 23.2). First the frontend steps
of decision 0075 (point 7), from `crates/otaq-desktop`, where the page lives:

1. `node`: the Node version check, against `crates/otaq-desktop/.nvmrc` (24); the script stops with
   "Node 24 is required (found v22.3.0)" on any other version
2. `npm-ci`: `npm ci`, the locked packages
3. `types`: `cargo test -p otaq-desktop generated_types_are_fresh`, the `ts-rs` freshness check (it names the stale
   files in `ui/src/generated/`)
4. `tsc`: `npm run typecheck` (`tsc --noEmit`)
5. `eslint`: `npm run lint` (`--max-warnings 0`)
6. `prettier`: `npm run format:check`
7. `vitest`: `npm run test`
8. `page`: `npm run build:page` (Vite builds the page into `ui/dist`)

then the Rust steps:

9. `fmt`: `cargo fmt --all --check`
10. `clippy`: `cargo clippy --workspace --all-targets -- -D warnings`
11. `test`: `cargo test --workspace`

| Platform | Run | Where |
|---|---|---|
| macOS | `scripts/check.sh` | the developer's Mac |
| Linux | `scripts/check.sh` (unchanged), on a copy | the local Ubuntu VM |
| Windows | `powershell -ExecutionPolicy Bypass -File scripts\check.ps1` | the local Windows VM |

Both scripts print each step's name and stop at the first failing step with a non-zero exit. There are no GitHub
Actions yet, so these scripts are the CI; a macOS / Windows / Linux matrix that runs the same steps is planned (d0056).

**Disk space on the Mac first.** The VMs' disk images live on the Mac and grow with every build inside them (a
full check with its own target folder adds tens of GB per VM), and they don't shrink when the VM deletes files. Before
a VM run, check the Mac's free space (`df -h /System/Volumes/Data`) and keep well over 40 GB free; on 2026-10-13 two
VM checks side by side filled the Mac's disk, the Ubuntu VM froze in `cargo test --workspace` and quit. Run one VM at
a time, and delete a VM's target folder when its checks are done.

The VMs run in VMPal on the Mac and see the repository as a shared folder. Commits are made on the Mac only. Each VM
builds into a folder on its own disk (`CARGO_TARGET_DIR`), never the repository's `target/`, so its builds don't
collide with the Mac's. Each VM also runs the checks on its own copy of the repository, never in the shared folder:
`npm ci` replaces `crates/otaq-desktop/node_modules`, and the Mac's copy holds macOS builds of some packages
(esbuild, Rollup, the Tauri CLI).

## Rust

All three machines use the same toolchain as the Mac: **Rust 1.99.0** with `rustfmt` and `clippy`
(`rustup` installs it; the workspace asks for `rust-version = 1.99`).

## Node

All three machines also need **Node 24** (LTS) with its npm, for the frontend steps (decision 0075; the version is
pinned in `crates/otaq-desktop/.nvmrc` and `package.json`'s `engines`). Nothing npm-related is installed
globally: the Tauri CLI is a dev dependency. On the Mac, Node 24 comes from nvm (`nvm install 24`, then `nvm use` in
`crates/otaq-desktop`, or put `~/.nvm/versions/node/v24.*/bin` first on `PATH` for the run).

## Linux VM (Ubuntu 26.04, aarch64)

Set up on 2026-10-11, starting from a fresh install with nothing added; updated on 2026-10-13 (T-0246, no snapshot:
a running Ubuntu VM can't be snapshotted, and the user approved working without one). Each step says how to undo it.

1. Build tools and the system libraries Tauri builds against (WebKitGTK 4.1, GTK 3, the tray library, D-Bus), as
   root:

   ```sh
   sudo apt-get update
   sudo apt-get install -y build-essential pkg-config git curl wget file \
     libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev \
     libssl-dev libdbus-1-dev libnotify-bin
   ```

   `libnotify-bin` gives `notify-send`, which Otaq's OS notifications use on Linux (Ubuntu's desktop has it
   already).

   Done on 2026-10-13 with these packages, without errors (WebKitGTK 2.52.6). Undo: `sudo apt-get remove` the
   packages of the second line, then `sudo apt-get autoremove`. The GPUI app's system libraries installed in M11
   (the list in `git show b7498ca:scripts/README.md`) are no longer needed and can be removed the same way; they were
   left in place.
2. Rust, as the normal user:

   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
     | sh -s -- -y --default-toolchain 1.99.0 --profile minimal -c rustfmt -c clippy
   ```
3. Node 24, as the normal user, with nvm (it installs into the home folder):

   ```sh
   curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
   . ~/.nvm/nvm.sh && nvm install 24
   ```

   Done on 2026-10-13 (nvm 0.40.3, Node v24.21.0). Undo: `rm -rf ~/.nvm` and delete the nvm lines the installer
   added to the end of `~/.bashrc`. The test build's leftovers in the home folder (`~/node24`, `~/m14-sysroot`,
   `~/m14-debs`, `~/m14tb`, `~/m14tb-target`) aren't used any more and can be deleted.
4. Run the checks on a copy in the home folder, with a target folder there too (the app itself needs a desktop
   session to open its window: Wayland or X11):

   ```sh
   rsync -a --delete --exclude target --exclude .claude --exclude node_modules --exclude ui/dist \
     /media/VMPal/software-factory/ $HOME/cb-src/
   cd $HOME/cb-src
   . ~/.nvm/nvm.sh && nvm use 24
   export CARGO_TARGET_DIR=$HOME/cb-target PATH=$HOME/.cargo/bin:$PATH CARGO_BUILD_JOBS=4
   scripts/check.sh
   ```

   Run `rsync` again after every change on the Mac. Never commit from the copy. `CARGO_BUILD_JOBS=4` keeps the
   builds within the VM's 16 GB. To check a committed branch instead of the Mac's working tree, clone it from the
   shared folder: `git clone --branch <branch> /media/VMPal/software-factory ~/cb-src`. Undo: delete `~/cb-src` and
   `~/cb-target`.

## Windows VM (Windows 11 Pro 24H2, ARM64)

Set up on 2026-10-11, starting from a fresh install with only Git for Windows added; Node added on 2026-10-13
(T-0246), after a snapshot ("Before T-0246 (Node 24 via winget)").

1. **Git for Windows** (already there): `winget install Git.Git`. Besides `git`, the tests use its Unix tools
   (`sh`, `cat`) in place of `claude`, as on macOS and Linux; `check.ps1` adds Git's `usr\bin` to the end of PATH for
   the run. Claude Code on Windows needs Git for Windows too.
2. **Visual Studio 2022 Build Tools** with the C++ tools for ARM64 and the Windows 11 SDK, from an elevated
   PowerShell:

   ```powershell
   $override = '--quiet --wait --norestart --nocache --add Microsoft.VisualStudio.Workload.VCTools ' +
     '--add Microsoft.VisualStudio.Component.VC.Tools.ARM64 ' +
     '--add Microsoft.VisualStudio.Component.Windows11SDK.26100 --includeRecommended'
   winget install --id Microsoft.VisualStudio.2022.BuildTools --exact --silent `
     --accept-source-agreements --accept-package-agreements --disable-interactivity --override $override
   ```

   (On an x64 machine use `Microsoft.VisualStudio.Component.VC.Tools.x86.x64` instead of the ARM64 component.)
3. **Rust**, as the normal user (no elevation):

   ```powershell
   Invoke-WebRequest -UseBasicParsing `
     https://static.rust-lang.org/rustup/dist/aarch64-pc-windows-msvc/rustup-init.exe -OutFile $env:TEMP\rustup-init.exe
   & $env:TEMP\rustup-init.exe -y --default-toolchain 1.99.0 --profile minimal -c rustfmt -c clippy
   ```
4. **Node 24**, the Windows ARM64 installer (`node-v24.*-arm64.msi`) from nodejs.org, or with winget:
   `winget install --id OpenJS.NodeJS.LTS --exact` (check that `node --version` prints `v24.*`; once a newer LTS
   is out, take the v24 installer instead). Open a new PowerShell afterwards so `node` and `npm` are on PATH.
   Done on 2026-10-13 from an elevated PowerShell, `winget install --id OpenJS.NodeJS.LTS --exact --scope machine`:
   winget's LTS package was still v24 (v24.20.0, arm64). Undo: `winget uninstall --id OpenJS.NodeJS.LTS --exact`.
   Git for Windows checks text files out with CRLF line endings by default (`core.autocrlf`), which Prettier's
   format check rejects in every file; the repository's `.gitattributes` keeps LF in every checkout.
5. **Run the checks on a copy on the VM's own disk.** The VMPal shared folder (`Z:\software-factory`) can't be
   canonicalized by Windows ("The volume does not contain a recognized file system", os error 1005), which
   `sqlx::migrate!` needs at build time, so the repository is mirrored to the VM's disk first (without the Mac's
   `node_modules`, which `npm ci` installs anew):

   ```powershell
   robocopy Z:\software-factory $env:USERPROFILE\cb-src /MIR /XD target .claude node_modules dist /XF .DS_Store
   $env:CARGO_TARGET_DIR = "$env:USERPROFILE\cb-target"
   cd $env:USERPROFILE\cb-src
   powershell -ExecutionPolicy Bypass -File scripts\check.ps1
   ```

   Run `robocopy` again after every change on the Mac. Never commit from the copy. To check a committed branch
   instead of the Mac's working tree, fetch it into the copy: `git fetch Z:/software-factory <branch>`, then
   `git checkout -f --detach FETCH_HEAD`. Undo: delete `%USERPROFILE%\cb-src` and `%USERPROFILE%\cb-target`.

PowerShell's default execution policy blocks unsigned scripts, hence `-ExecutionPolicy Bypass` for this one run
(the machine's policy stays as it is).

Optional, done in the VM: some tests listen on all network interfaces, and Windows then asks once per test
program whether to allow it through the firewall (the tests pass either way; the prompts only pile up on the
screen). An elevated `Set-NetFirewallProfile -All -NotifyOnListen False` turns those prompts off.

To look at the app in the VM (the hand checks), build it in the copy with `cargo build --release -p otaq-cli` (the
service the app starts sits next to it) and, in `crates\otaq-desktop`, `npm run tauri build` (the page goes inside
the binary), then start `%USERPROFILE%\cb-target\release\otaq-app.exe` from the signed-in desktop, with
`OTAQ_HOME` set to a temporary folder.
A program started by a remote command (VMPal's exec, ssh) runs off the visible desktop; a scheduled task that runs
in the signed-in session puts it on screen. On 2026-10-13 (T-0246) a `.cmd` setting `OTAQ_HOME` and
`OTAQ_KEY_LOG=1` and starting the app was run with `schtasks /create /tn cb-t0246-app /tr <the .cmd> /sc once
/st 23:59 /it /f`, then `schtasks /run /tn cb-t0246-app`. Undo: `schtasks /delete /tn cb-t0246-app /f`. After a
restart Windows waits at the sign-in screen (automatic sign-in is off), and nothing can be run in it until someone
signs in.

## Warm builds and project variables (for people using Otaq)

Decision 0082. This section is about using Otaq on any project, so it names no language or build tool.

**Warm builds.** Each ticket still gets its own worktree, but a new worktree doesn't have to build from nothing.
When a ticket reaches Done, Otaq moves its worktree's gitignored files (what your setup installed and your builds
made, but not the project's env files) into the project's warm build, `<data folder>/warm/<repo-name>` (for example
`~/.otaq/warm/shop-web`, next to `wt/`), with the commit they were built at and the modified time of each tracked
file. A newer build replaces an older one; an older one never replaces a newer one. The next new worktree is cloned
from it; the tracked files unchanged since that commit get back the times they had where the build was made, and the
ones changed since get the current time, so your first build only redoes what differs (also a file committed after
the last build). With no warm build yet, the first worktree is cloned from your own checkout's gitignored files, if
it has any and is on the same disk, with the checkout's own file times.
The setup log of each ticket says where its worktree started from.

- **The switch:** project Settings → "Start new worktrees from a warm build", on by default. Turn it off for a
  project whose build output doesn't survive being moved to another folder; turning it off also clears the warm
  build (after a confirmation).
- **Clear:** Cleanup lists each project's warm build under the leftovers, with its size, commit and age, and a
  **Clear…** button. Sizes say "up to": a clone shares its disk space with what it was cloned from. Nothing clears
  warm builds automatically and there is no size limit, so clear one that has grown too big; the next Done keeps a
  new one.
- **It needs a disk that can clone** (copy-on-write): APFS on macOS, btrfs or XFS on Linux, a Dev Drive on Windows.
  What counts is the disk under Otaq's data folder (where `wt/` and `warm/` live). On any other disk (NTFS, ext4,
  exFAT…) the switch shows as unavailable with the reason, and worktrees start as before. Otaq never falls back to a
  plain copy, which would cost a whole build's disk space per ticket.
- **On Windows, put the data folder on a Dev Drive** to get warm builds. Create one in Settings → System → Storage →
  Advanced storage settings → Disks & volumes → Create Dev Drive (a new partition or a virtual disk file). Then move
  Otaq's data folder there while no ticket has a worktree (nothing in Dev or Ready for Review, and Cleanup empty):
  quit Otaq, stop its service (`otaq daemon stop`), move the folder `%USERPROFILE%\.otaq` to the Dev Drive (for
  example to `D:\otaq`) and delete the `scratch` folder inside it (Otaq makes it again at the next Local merge), run
  `git worktree prune` in each project's checkout, set `OTAQ_HOME` for your account (`setx OTAQ_HOME D:\otaq`) and
  start Otaq again. With `OTAQ_HOME` set, Otaq uses that folder instead of `~/.otaq` (on every platform).
- **A setup command that deletes its output before installing** (a "clean install", or a step that empties the
  build folder first) throws the warm build away on every new worktree, and the first build starts from nothing
  again. Use the form of your install step that updates what is already there.

**Project variables.** Project Settings → Environment variables lists `NAME = value` rows that Otaq sets in
everything it starts in that project: ticket agents, the validator, Project agents and epic leads, the Project
terminal, the setup command, Run and Shell, and the git commands it runs in the project's repository and worktrees
(so your git hooks see them too). It is empty by default, and Otaq adds nothing of its own. For example `CI = true`,
so tools never wait for input or start watch mode. Another one you may want when several agents build at once: each
build takes every core by default, so give each one a share, written for your stack, such as a variable your build
tool reads for its job count set to your cores divided by the number of agents that build at the same time. A
change applies to what starts or resumes after it; running sessions keep the values they started with. A row with
an empty name, a name containing `=`, or a name repeating an earlier row's (on Windows, also one differing only in
case) is marked and not set; Otaq's own variables (`OTAQ_*`, `PORT`) win over a row of the same name.

## Not covered by the checks

The hand checks of each milestone (`tickets/review-M*.md`), for example the window chrome, keyboard shortcuts and
notifications on Windows and Linux, and anything with the real `claude`, `gh` or `az`.

## Third-party licenses

`THIRD-PARTY-LICENSES.md` lists the notices of everything Otaq ships (decision 0071). After changing dependencies, run
`scripts/licenses.sh` on the Mac and commit the regenerated file with the change; `scripts/licenses.sh --check` says
whether it is up to date. It covers the Rust crates (through `cargo-about`) and the page's production npm packages
(`scripts/licenses/npm.mjs`, from `package-lock.json` and the license files of the installed packages, so `npm ci`
first). It needs `cargo-about` (`cargo install cargo-about --locked --features cli`), so it isn't one of the check
steps.
