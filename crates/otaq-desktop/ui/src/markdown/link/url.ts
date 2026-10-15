// What a description link may point at, and how it opens (T-0252, d0078): web links only (Q52), always in the system
// browser, never inside Otaq.
import { request } from "../../tauri";

/** The schemes a link may have; the popover refuses every other one. */
export const LINK_SCHEMES: readonly string[] = ["https:", "http:"];

/** The popover's inline errors. */
export const LINK_ERRORS = {
  empty: "Enter a URL.",
  scheme: "Only https: and http: links.",
  invalid: "That isn't a URL.",
} as const;

export type LinkCheck = { ok: true; href: string } | { ok: false; error: string };

const HAS_SCHEME = /^[a-z][a-z0-9+.-]*:/i;

/**
 * The URL typed in the popover, checked: `https:` and `http:` pass (as typed, trimmed); a URL with no scheme gets
 * `https://` (`example.com/report`); any other scheme (`mailto:`, `javascript:`, `file:`, `otaq:`) is refused (Q52).
 */
export function checkLinkUrl(typed: string): LinkCheck {
  const text = typed.trim();
  if (text === "") return { ok: false, error: LINK_ERRORS.empty };
  const href = HAS_SCHEME.test(text) ? text : `https://${text}`;
  let url: URL;
  try {
    url = new URL(href);
  } catch {
    return { ok: false, error: LINK_ERRORS.invalid };
  }
  if (!LINK_SCHEMES.includes(url.protocol)) return { ok: false, error: LINK_ERRORS.scheme };
  if (url.hostname === "") return { ok: false, error: LINK_ERRORS.invalid };
  if (/\s/.test(href)) return { ok: false, error: LINK_ERRORS.invalid };
  return { ok: true, href };
}

/**
 * Whether a link can be opened from Otaq: web links only, as `terminal.open_link` (the one way out to the browser)
 * accepts. A link that isn't one but is already in a stored description (`mailto:`, written by an agent) is kept and
 * shown unchanged, and doesn't open (Q52).
 */
export function opensInBrowser(href: string): boolean {
  return /^https?:\/\//i.test(href.trim());
}

/** The URL as the bubble shows it: without `https://` or `http://` and a trailing slash. */
export function shortUrl(href: string): string {
  return href
    .trim()
    .replace(/^https?:\/\//i, "")
    .replace(/\/$/, "");
}

/** Opens a web link in the system browser, through the terminal's link request and its safety rules. */
export function openInBrowser(href: string): void {
  if (!opensInBrowser(href)) return;
  request("terminal.open_link", { url: href.trim() }).catch((e: unknown) => {
    console.error("couldn't open the link", e);
  });
}
