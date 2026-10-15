// The Link hook: the one place the Link button and ⌘K go. It opens the link popover (`markdown/link/`, T-0252).
import type { Editor } from "@tiptap/core";

import { openLinkPopover } from "../link/links";

/**
 * The Link button and ⌘K (d0078 D2): opens the popover on the link around the caret, on the selection, or at the
 * caret, where Apply types the URL as the link's text.
 */
export function openLink(editor: Editor): void {
  openLinkPopover(editor);
}
