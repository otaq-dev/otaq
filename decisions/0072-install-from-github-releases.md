# 0072. Installing: GitHub Releases archives and a one-line install script

- **Status:** Accepted
- **Date:** 2026-10-11
- **Related:** 0021, 0022, 0025, 0055, 0062, 0071

## Context
0021 left the package channels open, and 0062 left open how the personal build is installed (the `crewboard` command on the
PATH, the plugin folder, an app bundle for notifications). There is no paid code signing (0021), so a file downloaded in a
browser is marked as "from the internet" and macOS and Windows warn before the first run. The app already finds its own
pieces next to its executable: the `crewboard` command, `crewboard-app` and the plugin (`<bin>/plugin` or
`<prefix>/share/crewboard/plugin`), so one unpacked folder works without an installer.

## Decision
**GitHub Releases is the one source.** Each version publishes one archive per platform plus `SHA256SUMS`:

| Platform | Archive | Contents |
|---|---|---|
| macOS arm64, x64 | `crewboard-<version>-<target>.tar.gz` | `Crewboard.app` (both binaries in `Contents/MacOS`, the plugin in `Contents/Resources/plugin`, ad-hoc signed), `LICENSE`, `THIRD-PARTY-LICENSES.md` |
| Linux x64, arm64 | `.tar.gz` | `crewboard`, `crewboard-app`, `plugin/`, a `.desktop` file and icon, the license files |
| Windows x64, arm64 | `.zip` | `crewboard.exe`, `crewboard-app.exe`, `plugin\`, the license files |

The Linux build targets an older glibc than the newest Ubuntu, so it runs on the distributions people actually use. Once the
repository is on GitHub, a GitHub Actions matrix builds the archives (0056's checks run first).

**The recommended route is a one-line install script** per OS, published with each release:
- macOS and Linux: `curl -fsSL https://github.com/<owner>/crewboard/releases/latest/download/install.sh | sh`
- Windows: `irm https://github.com/<owner>/crewboard/releases/latest/download/install.ps1 | iex`

It picks the archive for the machine, checks it against `SHA256SUMS`, and installs into the user's own folders, **without
admin rights**:
- macOS: `~/Applications/Crewboard.app`, and `~/.local/bin/crewboard` pointing into it.
- Linux: `~/.local/share/crewboard/`, `~/.local/bin/crewboard`, and a `.desktop` entry with the icon.
- Windows: `%LOCALAPPDATA%\Programs\Crewboard\`, that folder on the user PATH, and a Start-menu shortcut that carries
  Crewboard's app identity (so toasts say "Crewboard", Q34).

It tells the user when `~/.local/bin` isn't on their PATH instead of editing shell files. A download with `curl` or
`Invoke-WebRequest` isn't marked "from the internet", so macOS shows no Gatekeeper prompt (the way rustup, uv and bun ship
unsigned builds); Windows may still show SmartScreen once.

**Manual download stays possible:** unpack and run. The README explains the one-time "Open Anyway" step (or
`xattr -dr com.apple.quarantine Crewboard.app`) on macOS and "More info → Run anyway" on Windows.

**The personal build uses the same layout:** `scripts/install-local.sh` installs from the local `target/release` into the
same places, so the author runs what users get. This answers 0062's open item.

**Later, as wrappers around the same archives:** a Crewboard Homebrew tap, a Scoop bucket, the AUR. Not
`cargo install crewboard`: the app needs its web frontend built and bundled (Tauri, 0074), which `cargo install` doesn't do.

## Alternatives considered
- **Archives only, no script:** every macOS user would hit the "unverified developer" block, and nobody would get the
  Dock icon, PATH entry or Start-menu identity without manual steps.
- **Native installers (`.dmg`, `.msi`, `.deb`):** more formats to build and test, and without signing they show the same
  warnings. They can come later if people ask.
- **Package managers first:** Homebrew's main repository likely refuses unsigned apps (0021), and each manager is its own
  upkeep. They wrap the archives later.

## Consequences
- `Crewboard.app` needs a bundle ID and an icon: placeholders (`io.github.<owner>.crewboard`, a placeholder icon) until
  the publisher name and the logo are decided.
- Code changes: `find_plugin_dir` also looks in `<bin>/../Resources/plugin` (the macOS bundle); macOS notifications move to
  the bundle so a click opens the ticket (Q22); the Windows shortcut's app identity lets toast clicks reach Crewboard (Q34).
- New work, as tickets once the repository has a remote: the release build matrix, `install.sh`, `install.ps1`,
  `install-local.sh`, the macOS bundle, the Linux `.desktop` file.
- Updates: 0073 (side-by-side version folders under the install folder, `current` pointing at the newest).

## Changelog
- 2026-10-11: created (user decision)
- 2026-10-11: updates decided in 0073.
- 2026-10-11: the reason for no `cargo install` updated after the move to Tauri (0074).
- 2026-10-15: names follow 0076: repository `<owner>/otaq`, `otaq-<version>-<target>` archives, `Otaq.app`, bundle ID `io.github.owner.otaq`, `~/.local/bin/otaq`; the icon is the Command tent logo.
