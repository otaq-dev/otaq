# Checks on macOS, Windows and Linux

Every commit must pass the same steps on every platform (decision 0056, SDD section 23.2). First the frontend steps
of decision 0075 (point 7), from `crates/crewboard-desktop`, where the page lives:

1. `node`: the Node version check, against `crates/crewboard-desktop/.nvmrc` (24); the script stops with
   "Node 24 is required (found v22.3.0)" on any other version
2. `npm-ci`: `npm ci`, the locked packages
3. `types`: `cargo test -p crewboard-desktop generated_types_are_fresh`, the `ts-rs` freshness check (it names the stale
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
Actions while the repository has no remote; once it has one, a macOS / Windows / Linux matrix runs the same steps
(d0056).

**Disk space on the Mac first.** The VMs' disk images live on the Mac and grow with every build inside them (a
full check with its own target folder adds tens of GB per VM), and they don't shrink when the VM deletes files. Before
a VM run, check the Mac's free space (`df -h /System/Volumes/Data`) and keep well over 40 GB free; on 2026-10-13 two
VM checks side by side filled the Mac's disk, the Ubuntu VM froze in `cargo test --workspace` and quit. Run one VM at
a time, and delete a VM's target folder when its checks are done.

The VMs run in VMPal on the Mac and see the repository as a shared folder. Commits are made on the Mac only. Each VM
builds into a folder on its own disk (`CARGO_TARGET_DIR`), never the repository's `target/`, so its builds don't
collide with the Mac's. Each VM also runs the checks on its own copy of the repository, never in the shared folder:
`npm ci` replaces `crates/crewboard-desktop/node_modules`, and the Mac's copy holds macOS builds of some packages
(esbuild, Rollup, the Tauri CLI).

## Rust

All three machines use the same toolchain as the Mac: **Rust 1.99.0** with `rustfmt` and `clippy`
(`rustup` installs it; the workspace asks for `rust-version = 1.99`).

## Node

All three machines also need **Node 24** (LTS) with its npm, for the frontend steps (decision 0075; the version is
pinned in `crates/crewboard-desktop/.nvmrc` and `package.json`'s `engines`). Nothing npm-related is installed
globally: the Tauri CLI is a dev dependency. On the Mac, Node 24 comes from nvm (`nvm install 24`, then `nvm use` in
`crates/crewboard-desktop`, or put `~/.nvm/versions/node/v24.*/bin` first on `PATH` for the run).

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

To look at the app in the VM (the hand checks), build it in the copy with `cargo build --release -p crewboard-cli` (the
service the app starts sits next to it) and, in `crates\crewboard-desktop`, `npm run tauri build` (the page goes inside
the binary), then start `%USERPROFILE%\cb-target\release\crewboard-app.exe` from the signed-in desktop, with
`CREWBOARD_HOME` set to a temporary folder.
A program started by a remote command (VMPal's exec, ssh) runs off the visible desktop; a scheduled task that runs
in the signed-in session puts it on screen. On 2026-10-13 (T-0246) a `.cmd` setting `CREWBOARD_HOME` and
`CREWBOARD_KEY_LOG=1` and starting the app was run with `schtasks /create /tn cb-t0246-app /tr <the .cmd> /sc once
/st 23:59 /it /f`, then `schtasks /run /tn cb-t0246-app`. Undo: `schtasks /delete /tn cb-t0246-app /f`. After a
restart Windows waits at the sign-in screen (automatic sign-in is off), and nothing can be run in it until someone
signs in.

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
