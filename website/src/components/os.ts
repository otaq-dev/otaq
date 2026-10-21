// The visitor's operating system and what the page says for each one: the `OS` table of the canvas's script
// (Main.dc.html), with the `otaq-io` organisation (d0076). The install commands are d0072's.

export type OsKey = "mac" | "linux" | "win";

export interface OsInfo {
  /** The Install tab's label. */
  label: string;
  /** The Download button's label. */
  download: string;
  /** The note under the hero's buttons. */
  note: string;
  /** The one-line install command (hero and Install card). */
  command: string;
  intro: string;
  path: string;
  archive: string;
  foot: string;
}

export const REPO_URL = "https://github.com/otaq-io/otaq";
export const DOWNLOAD_URL = `${REPO_URL}/releases/latest`;

const UNIX_COMMAND = `curl -fsSL ${DOWNLOAD_URL}/download/install.sh | sh`;
const WINDOWS_COMMAND = `irm ${DOWNLOAD_URL}/download/install.ps1 | iex`;

/** The Install tabs, in the drawn order. */
export const OS_KEYS: readonly OsKey[] = ["mac", "linux", "win"];

export const OS: Readonly<Record<OsKey, OsInfo>> = {
  mac: {
    label: "macOS",
    download: "Download for macOS",
    note: "Apple silicon and Intel · also for Windows and Linux",
    command: UNIX_COMMAND,
    intro: "Paste this into Terminal. It picks the right build for your Mac and checks it before installing.",
    path: "~/Applications/Otaq.app",
    archive: "otaq-<version>-aarch64-apple-darwin.tar.gz (or x86_64)",
    foot: "Installed this way, macOS opens Otaq without a warning. A manual download needs a one-time “Open Anyway”.",
  },
  linux: {
    label: "Linux",
    download: "Download for Linux",
    note: "x64 and arm64 · also for macOS and Windows",
    command: UNIX_COMMAND,
    intro: "Paste this into your terminal. It picks the right build, checks it, and adds a desktop entry.",
    path: "~/.local/share/otaq · ~/.local/bin/otaq",
    archive: "otaq-<version>-x86_64-unknown-linux-gnu.tar.gz (or aarch64)",
    foot: "Built against an older glibc, so it runs on the distributions people actually use.",
  },
  win: {
    label: "Windows",
    download: "Download for Windows",
    note: "Windows 10 and 11, x64 and arm64 · also for macOS and Linux",
    command: WINDOWS_COMMAND,
    intro: "Paste this into PowerShell. It picks the right build, checks it, and adds a Start menu shortcut.",
    path: "%LOCALAPPDATA%\\Programs\\Otaq",
    archive: "otaq-<version>-x86_64-pc-windows-msvc.zip (or aarch64)",
    foot: "Windows may show SmartScreen once: More info → Run anyway.",
  },
};

/** The page's detection: Windows, then Linux (not Android), else macOS, also when nothing is known. */
export function pickOs(userAgent: string, platform = ""): OsKey {
  const ua = `${userAgent} ${platform}`;
  if (/Win/i.test(ua)) return "win";
  if (/Linux|X11/i.test(ua) && !/Android/i.test(ua)) return "linux";
  return "mac";
}
