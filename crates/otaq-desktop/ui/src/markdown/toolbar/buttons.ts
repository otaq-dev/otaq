// The toolbar's buttons (d0078, *MD Toolbar · component*): one entry per button, in the order they're drawn, so each
// ticket adds its own line. T-0250 fills the list and draws it; Expand (T-0260) and the Markdown toggle (T-0251) are
// slots at the toolbar's right end, not entries here.

import type { Editor } from "@tiptap/core";

import type { IconName } from "../../generated/IconName";

/** One toolbar button. */
export interface ToolbarButton {
  /** A stable name, for tests and the overflow order. */
  id: string;
  /** The tooltip's name ("Bold"). */
  label: string;
  icon: IconName;
  /** Applies the format at the caret or the selection. */
  run: (editor: Editor) => void;
  /** Whether the caret is in this format (the button's active state). */
  isActive?: (editor: Editor) => boolean;
}

export const TOOLBAR_BUTTONS: readonly ToolbarButton[] = [];
