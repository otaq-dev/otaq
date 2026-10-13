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

Set up on 2026-10-11, starting from a fresh install with nothing added.

1. Build tools and the system libraries Tauri builds against (WebKitGTK 4.1, GTK 3, the tray library, D-Bus), as
   root:

   ```sh
   sudo apt-get update
   sudo apt-get install -y build-essential pkg-config git curl wget file \
     libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev \
     libssl-dev libdbus-1-dev libnotify-bin
   ```

   `libnotify-bin` gives `notify-send`, which Crewboard's OS notifications use on Linux (Ubuntu's desktop has it
   already).
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
4. Run the checks on a copy in the home folder, with a target folder there too (the app itself needs a desktop
   session to open its window: Wayland or X11):

   ```sh
   rsync -a --delete --exclude target --exclude .claude --exclude node_modules --exclude ui/dist \
     /media/VMPal/software-factory/ $HOME/cb-src/
   cd $HOME/cb-src
   . ~/.nvm/nvm.sh && nvm use 24
   export CARGO_TARGET_DIR=$HOME/cb-target PATH=$HOME/.cargo/bin:$PATH
   scripts/check.sh
   ```

   Run `rsync` again after every change on the Mac. Never commit from the copy.

## Windows VM (Windows 11 Pro 24H2, ARM64)

Set up on 2026-10-11, starting from a fresh install with only Git for Windows added.

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

   Run `robocopy` again after every change on the Mac. Never commit from the copy.

PowerShell's default execution policy blocks unsigned scripts, hence `-ExecutionPolicy Bypass` for this one run
(the machine's policy stays as it is).

Optional, done in the VM: some tests listen on all network interfaces, and Windows then asks once per test
program whether to allow it through the firewall (the tests pass either way; the prompts only pile up on the
screen). An elevated `Set-NetFirewallProfile -All -NotifyOnListen False` turns those prompts off.

To look at the app in the VM (the hand checks), build it in the copy with `cargo build --release -p crewboard-cli` (the
service the app starts sits next to it) and, in `crates\crewboard-desktop`, `npm run tauri build` (the page goes inside
the binary), then start `%USERPROFILE%\cb-target\release\crewboard-app.exe` from the signed-in desktop, with
`CREWBOARD_HOME` set to a temporary folder.

## Not covered by the checks

The hand checks of each milestone (`tickets/review-M*.md`), for example the window chrome, keyboard shortcuts and
notifications on Windows and Linux, and anything with the real `claude`, `gh` or `az`.

## Third-party licenses

`THIRD-PARTY-LICENSES.md` lists the notices of everything Crewboard ships (decision 0071). After changing dependencies, run
`scripts/licenses.sh` on the Mac and commit the regenerated file with the change; `scripts/licenses.sh --check` says
whether it is up to date. It covers the Rust crates (through `cargo-about`) and the page's production npm packages
(`scripts/licenses/npm.mjs`, from `package-lock.json` and the license files of the installed packages, so `npm ci`
first). It needs `cargo-about` (`cargo install cargo-about --locked --features cli`), so it isn't one of the check
steps.
