// The ? list's one fixed id: the Settings topic its link opens (T-0395's Keyboard shortcuts page, d0089).
import type { GlobalSection } from "../../app/registry/settings";

/** Settings → Keyboard shortcuts. Until T-0395 adds the topic to `GlobalSectionId`, Settings opens on its first page.
 * Once it has: `export const KEYBOARD_SECTION: GlobalSection = "keyboard_shortcuts";`. */
const KEYBOARD_SECTION_ID: string = "keyboard_shortcuts";
export const KEYBOARD_SECTION = KEYBOARD_SECTION_ID as GlobalSection;
