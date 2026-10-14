// The Open-source licenses dialog's filter (d0071; *15 → Open-source licenses · groups & filter*): which bundled rows
// and which crates a query keeps, and the wording templates the Rust view sends, filled in. Matching is by crate
// name or license name, case-insensitive; a license that matches keeps its whole group.
import type { LicensesBundledRow } from "../../generated/LicensesBundledRow";
import type { LicensesDialog } from "../../generated/LicensesDialog";
import type { LicensesGroup } from "../../generated/LicensesGroup";
import type { LicensesItem } from "../../generated/LicensesItem";

export interface ShownGroup {
  group: LicensesGroup;
  /** The items the query keeps; all of them without a query. */
  items: readonly LicensesItem[];
}

export interface Shown {
  bundled: readonly LicensesBundledRow[];
  rust: readonly ShownGroup[];
  npm: readonly ShownGroup[];
  /** How many crates and packages the query kept; 0 without a query. */
  matches: number;
  /** Whether a query is set (after trimming). */
  filtering: boolean;
}

function filterGroups(groups: readonly LicensesGroup[], q: string): ShownGroup[] {
  return groups.flatMap((group) => {
    if (q === "" || group.license.toLowerCase().includes(q)) return [{ group, items: group.items }];
    const items = group.items.filter((i) => i.name.toLowerCase().includes(q));
    return items.length > 0 ? [{ group, items }] : [];
  });
}

/** What the dialog shows for `query`. */
export function filterLicenses(view: LicensesDialog, query: string): Shown {
  const q = query.trim().toLowerCase();
  const bundled =
    q === ""
      ? view.bundled
      : view.bundled.filter(
          (b) => b.project.toLowerCase().includes(q) || b.license.toLowerCase().includes(q),
        );
  const rust = filterGroups(view.rust, q);
  const npm = filterGroups(view.npm, q);
  const matches = q === "" ? 0 : [...rust, ...npm].reduce((n, g) => n + g.items.length, 0);
  return { bundled, rust, npm, matches, filtering: q !== "" };
}

/** A Rust template with `{count}`, `{total}` and `{query}` filled in. */
export function fill(template: string, values: { count?: number; total?: number; query?: string }): string {
  return template
    .replaceAll("{count}", String(values.count ?? ""))
    .replaceAll("{total}", String(values.total ?? ""))
    .replaceAll("{query}", values.query ?? "");
}

/** The line above the results while filtering: "4 crates match “tokio”". */
export function resultLine(view: LicensesDialog, shown: Shown, query: string): string {
  const template = shown.matches === 1 ? view.result_one : view.result_many;
  return fill(template, { count: shown.matches, query: query.trim() });
}

/** A group's count: its size, or "4 of 542" while a query keeps only some. */
export function groupCount(view: LicensesDialog, shown: ShownGroup): string {
  const total = shown.group.items.length;
  return shown.items.length === total
    ? String(total)
    : fill(view.count_of, { count: shown.items.length, total });
}

/** A group's key in the open set: its section and license. */
export function groupKey(section: "rust" | "npm", group: LicensesGroup): string {
  return `${section}:${group.license}`;
}
