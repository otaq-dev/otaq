import { describe, expect, it } from "vitest";

import type { LicensesDialog } from "../../generated/LicensesDialog";
import type { LicensesGroup } from "../../generated/LicensesGroup";
import { fill, filterLicenses, groupCount, groupKey, resultLine } from "./licenses";

/** The first element, which the test expects to be there. */
function first<T>(xs: readonly T[]): T {
  const x = xs[0];
  if (x === undefined) throw new Error("empty");
  return x;
}

const item = (name: string) => ({
  name,
  version: "1.0.0",
  source: `https://x/${name}`,
  source_text: `x/${name}`,
});
const group = (license: string, names: string[]): LicensesGroup => ({
  license,
  items: names.map(item),
  texts: [`${license} text`],
  note: null,
});

const view: LicensesDialog = {
  title: "Open-source licenses",
  description: "",
  filter_placeholder: "Filter by name or license",
  bundled_heading: "Bundled with the app",
  bundled: [
    {
      project: "Lucide icons",
      source_text: "github.com/lucide-icons/lucide",
      source: "https://l",
      license: "ISC",
    },
    { project: "Theme palettes", source_text: "9 projects", source: null, license: "MIT" },
  ],
  rust_heading: "Rust crates · 5",
  rust: [group("MIT License", ["tokio", "tokio-util", "serde"]), group("Apache License 2.0", ["tao", "dpi"])],
  npm_heading: "npm packages · 1",
  npm: [group("MIT", ["react"])],
  source_label: "Open the source",
  credits_label: "See Theme credits",
  show_text_label: "Show license text",
  hide_text_label: "Hide license text",
  result_one: "1 crate matches “{query}”",
  result_many: "{count} crates match “{query}”",
  count_of: "{count} of {total}",
  no_match: "No crate or license matches “{query}”",
  clear_label: "Clear filter",
  filter_keys: "⌘F",
  filter_hint: "filter",
  close_hint: "close",
  copy_label: "Copy all notices",
  copied_label: "Copied",
  done_label: "Done",
  notices: "",
};

describe("filterLicenses", () => {
  it("shows everything without a query", () => {
    const shown = filterLicenses(view, "  ");
    expect(shown.filtering).toBe(false);
    expect(shown.bundled).toHaveLength(2);
    expect(shown.rust.map((g) => g.items.length)).toEqual([3, 2]);
    expect(shown.matches).toBe(0);
  });

  it("keeps the crates whose name matches, and only their groups", () => {
    const shown = filterLicenses(view, "Tokio");
    expect(shown.rust.map((g) => g.group.license)).toEqual(["MIT License"]);
    expect(shown.rust[0]?.items.map((i) => i.name)).toEqual(["tokio", "tokio-util"]);
    expect(shown.npm).toEqual([]);
    expect(shown.bundled).toEqual([]);
    expect(shown.matches).toBe(2);
    expect(resultLine(view, shown, " Tokio ")).toBe("2 crates match “Tokio”");
    expect(groupCount(view, first(shown.rust))).toBe("2 of 3");
  });

  it("keeps a whole group when its license matches", () => {
    const shown = filterLicenses(view, "apache");
    expect(shown.rust).toHaveLength(1);
    expect(groupCount(view, first(shown.rust))).toBe("2");
  });

  it("matches bundled rows by name or license", () => {
    expect(filterLicenses(view, "isc").bundled.map((b) => b.project)).toEqual(["Lucide icons"]);
    expect(filterLicenses(view, "palettes").bundled.map((b) => b.project)).toEqual(["Theme palettes"]);
  });

  it("says when one or nothing matches", () => {
    const one = filterLicenses(view, "react");
    expect(resultLine(view, one, "react")).toBe("1 crate matches “react”");
    const none = filterLicenses(view, "foo");
    expect(none.matches).toBe(0);
    expect(none.rust).toEqual([]);
    expect(fill(view.no_match, { query: "foo" })).toBe("No crate or license matches “foo”");
  });

  it("keys groups by section and license", () => {
    expect(groupKey("npm", first(view.npm))).toBe("npm:MIT");
  });
});
