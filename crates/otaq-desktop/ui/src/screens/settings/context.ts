// What every Settings section reads: the open page from the Rust view and the calls back to it. The Settings overlay
// provides it; the section components (registered per id) take their rows from it.
import { createContext, useContext } from "react";

import type { SettingsButtonAction } from "../../generated/SettingsButtonAction";
import type { SettingsNavRow } from "../../generated/SettingsNavRow";
import type { SettingsPage } from "../../generated/SettingsPage";
import type { SettingsSection } from "../../generated/SettingsSection";
import type { SettingsValue } from "../../generated/SettingsValue";

export interface SettingsContextValue {
  page: SettingsPage;
  /** The project whose page is open; `null` on the global page. */
  project: string | null;
  /** Saves one field (`settings.edit`); the fresh view replaces the page. */
  edit: (field: string, value: SettingsValue) => void;
  act: (action: SettingsButtonAction) => void;
}

export const SettingsContext = createContext<SettingsContextValue | null>(null);

export function useSettings(): SettingsContextValue {
  const value = useContext(SettingsContext);
  if (value === null) throw new Error("a settings section outside Settings");
  return value;
}

/** The section of that id on the open page, if it has one. */
export function sectionOf(page: SettingsPage, id: string): SettingsSection<string> | undefined {
  const sections: readonly SettingsSection<string>[] = page.sections;
  return sections.find((s) => s.id === id);
}

/** The DOM id of a field's input, so "use your own Open PR skill" can focus the Open PR field. */
export function fieldId(field: string): string {
  return `settings-field-${field.replace(/[^a-z0-9_]/gi, "-")}`;
}

/** The open global topic's nav row: the one holding `topic` (any of its sections), else the first. */
export function topicRow(rows: readonly SettingsNavRow[], topic: string | null): SettingsNavRow | undefined {
  const sections = (row: SettingsNavRow): readonly string[] =>
    row.target.page === "global" ? row.target.sections : [];
  return rows.find((r) => topic !== null && sections(r).includes(topic)) ?? rows[0];
}
