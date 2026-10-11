# Checks on macOS, Windows and Linux

Every commit must pass the same three steps on every platform (decision 0056, SDD section 23.2):

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`

| Platform | Run | Where |
|---|---|---|
| macOS | `scripts/check.sh` | the developer's Mac |
| Linux | `scripts/check.sh` (unchanged) | the local Ubuntu VM |
| Windows | `powershell -ExecutionPolicy Bypass -File scripts\check.ps1` | the local Windows VM |

Both scripts stop at the first failing step and exit non-zero. There are no GitHub Actions while the repository has
no remote; once it has one, a macOS / Windows / Linux matrix runs the same steps (d0056).

The VMs run in VMPal on the Mac and see the repository as a shared folder. Commits are made on the Mac only. Each VM
builds into a folder on its own disk (`CARGO_TARGET_DIR`), never the repository's `target/`, so its builds don't
collide with the Mac's.

## Rust

All three machines use the same toolchain as the Mac: **Rust 1.99.0** with `rustfmt` and `clippy`
(`rustup` installs it; the workspace asks for `rust-version = 1.99`).

## Linux VM (Ubuntu 26.04, aarch64)

Set up on 2026-10-11, starting from a fresh install with nothing added.

1. Build tools and the GPUI (Zed) system libraries, as root:

   ```sh
   sudo apt-get update
   sudo apt-get install -y build-essential pkg-config git curl clang cmake \
     libfontconfig-dev libfreetype-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
     libx11-xcb-dev libxcb1-dev libvulkan1 libvulkan-dev mesa-vulkan-drivers libasound2-dev \
     libssl-dev libzstd-dev libgit2-dev libdbus-1-dev libglib2.0-dev libxcursor-dev libxrandr-dev \
     libxi-dev libgl-dev libegl-dev libnotify-bin
   ```

   `libnotify-bin` gives `notify-send`, which Crewboard's OS notifications use on Linux (Ubuntu's desktop has it
   already).
2. Rust, as the normal user:

   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
     | sh -s -- -y --default-toolchain 1.99.0 --profile minimal -c rustfmt -c clippy
   ```
3. Run the checks from the shared folder with a target folder in the home folder (the app itself needs a desktop
   session to open its window: Wayland or X11):

   ```sh
   cd /media/VMPal/software-factory
   export CARGO_TARGET_DIR=$HOME/cb-target PATH=$HOME/.cargo/bin:$PATH
   scripts/check.sh
   ```

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
4. **Run the checks on a copy on the VM's own disk.** The VMPal shared folder (`Z:\software-factory`) can't be
   canonicalized by Windows ("The volume does not contain a recognized file system", os error 1005), which
   `sqlx::migrate!` needs at build time, so the repository is mirrored to the VM's disk first:

   ```powershell
   robocopy Z:\software-factory $env:USERPROFILE\cb-src /MIR /XD target .claude /XF .DS_Store
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

To look at the app in the VM (the hand checks), build it with `cargo build -p crewboard-app -p crewboard-cli` in the
copy and start `%USERPROFILE%\cb-target\debug\crewboard-app.exe` from the signed-in desktop, with
`CREWBOARD_HOME` set to a temporary folder.

## Not covered by the checks

The hand checks of each milestone (`tickets/review-M*.md`), for example the window chrome, keyboard shortcuts and
notifications on Windows and Linux, and anything with the real `claude`, `gh` or `az`.

## Third-party licenses

`THIRD-PARTY-LICENSES.md` lists the notices of everything Crewboard ships (decision 0071). After changing dependencies, run
`scripts/licenses.sh` on the Mac and commit the regenerated file with the change; `scripts/licenses.sh --check` says
whether it is up to date. It needs `cargo-about` (`cargo install cargo-about --locked --features cli`), so it isn't one of
the three check steps.
