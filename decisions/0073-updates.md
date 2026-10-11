# 0073. Updates: a daily check, "Update now" runs the install script, side-by-side versions

- **Status:** Accepted
- **Date:** 2026-10-11
- **Related:** 0003, 0021, 0022, 0045, 0071, 0072

## Context
0072 installs Crewboard from GitHub Releases with a one-line script and left updates open. Crewboard's background service
runs the agents (0022), and the agents call the `crewboard` command and read the plugin's skills and hooks all the time, so an
update can't simply overwrite the installed files: two versions would mix inside one session, and Windows refuses to overwrite
a running `.exe`. 0022 already lets the user pick when the service restarts ("Service update ready · Restart now").

## Decision
**Knowing about an update.**
- The app asks GitHub for the latest release **once a day**, **on by default**. It sends one plain request to
  `api.github.com` and nothing about the user or their projects.
- Settings → About has the switch **"Check for updates automatically"** and a **Check for updates** button, with the
  results "You're up to date · 0.1.0 · checked 2 min ago", "Crewboard 0.2.0 is available" and "Couldn't reach GitHub".
- A newer version shows a quiet notice (a dot on Settings and a line in About): **"Crewboard 0.2.0 is available · What's
  new · Update now · Not now"**. What's new opens the GitHub release notes. Not now hides the notice until the next version.
  No pop-ups, no OS notification.

**Installing it.** **Update now** runs the same `install.sh` / `install.ps1` as a first install (0072), with its output
visible; running the one-liner by hand does the same. Each version is installed **side by side**:

```
<install folder>/versions/0.1.0/   ← the running service and its agents keep using this
<install folder>/versions/0.2.0/   ← the update unpacks here
<install folder>/current → 0.2.0   ← the app, the `crewboard` command, PATH and shortcuts start from here
```

- An update **never changes or deletes files of the version in use**. `current` is a symlink on macOS and Linux and a
  directory junction on Windows (no admin rights; the layout Scoop uses). PATH, the `~/.local/bin/crewboard` link, the
  Start-menu shortcut and the Linux `.desktop` entry point at `current`, so updates never change them.
- At start, the service resolves its own folder to the real version folder (not `current`), so its agents keep calling the
  same version's `crewboard` until it restarts.
- The previous version is kept; older ones are deleted on the next update. A folder still in use (Windows locks it) is
  skipped and tried again next time.
- macOS keeps `Crewboard.app` where the Dock and notifications expect it (`~/Applications`); how the bundle maps onto the
  versions is settled in the ticket, keeping the rule above.

**Switching over.** The running app notices a newer installed version and shows the 0022 banner, worded **"Crewboard 0.2.0
is installed · Restart now (3 agents working, 2 waiting)"**. Restart now stops the old service (agents are interrupted, as on
a reboot, with Resume all, 0045), then starts the new service and reopens the window from `current`. Nothing restarts by
itself.

**Database.** Before a new version changes the database layout, the service copies it to `crewboard.db.before-<version>`,
so going back to the kept version stays possible (0003).

**Other installs.** A manual zip or tarball install updates by hand: stop Crewboard (`crewboard daemon stop`), replace the
folder. Installs from a package manager (later, 0072) show its command instead of Update now, for example "Update with
`brew upgrade crewboard`"; the install script records how Crewboard was installed.

## Alternatives considered
- **No check; people re-run the script:** most people would never learn that updates exist.
- **A silent auto-updater:** conflicts with "the user is in control" and "the service never restarts itself" (0022).
- **Overwriting the installed files in place:** mixes versions inside running sessions, and fails on Windows while the
  service runs.
- **An update framework such as Velopack:** it applies updates only when every process has exited, which with the
  background service means stopping the agents to install; the side-by-side folders let people install now and restart
  later.

## Consequences
- The About section (0071) gains the update switch, the Check for updates button and its results; the 0022 banner gains the
  "installed" wording and restarts the window too. Both need a design update.
- New work: the daily check, the version-folder layout in `install.sh` / `install.ps1` / `install-local.sh`, the service
  resolving its real folder, the database copy before migrations, the install-source record.
- Defender may scan or flag a new unsigned Windows `.exe` for a while after a release; false positives are reported to
  Microsoft (no paid signing, 0021).

## Changelog
- 2026-10-11: created (user decision: check on by default, a switch and a Check for updates button in Settings)
