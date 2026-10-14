// What every Settings section reads: the open page from the Rust view and the calls back to it. The Settings overlay
// provides it; the section components (registered per id) take their rows from it.
import { createContext, useContext } from "react";

import type { SettingsButtonAction } from "../../generated/SettingsButtonAction";
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

/** The section in view on the scrolling global page: the last one whose top is at or above the page's top; at the
 * very bottom the last one (the same rule as `settings::section_in_view` in the GPUI app's view model). */
export function sectionInView(tops: readonly number[], scrollTop: number, atBottom: boolean): number {
  if (tops.length === 0) return 0;
  if (atBottom) return tops.length - 1;
  let found = 0;
  tops.forEach((top, i) => {
    if (top <= scrollTop + 1) found = i;
  });
  return found;
}
