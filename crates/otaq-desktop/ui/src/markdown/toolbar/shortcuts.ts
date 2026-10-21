// The toolbar's keys that Tiptap doesn't bind, or binds differently (d0078, the *Annotations* list), while the editor
// has focus. Labels for the tooltips and the "…" menu come from `shortcutLabel`.
import { Extension } from "@tiptap/core";

import type { Shortcut } from "./buttons";
import { openLink } from "./link";

/**
 * ⌘⇧X strikethrough (Tiptap's is ⌘⇧S, which the webview keeps for Save as), ⌘K the Link hook, ⌘⌥C Code block (Tiptap's
 * own key, taken here so it's always stopped: on a Mac the webview would open its inspector, Q51), and no heading
 * shortcut: StarterKit's heading keys are taken here and do nothing (d0058 gave them to the panel tabs; they stay off).
 */
export const ToolbarShortcuts = Extension.create({
  name: "toolbarShortcuts",
  // Before the formatting extensions (default 100), so their heading keys never run.
  priority: 1000,
  addKeyboardShortcuts() {
    const none = Object.fromEntries([1, 2, 3, 4, 5, 6].map((n) => [`Mod-Alt-${String(n)}`, () => true]));
    return {
      ...none,
      "Mod-Shift-x": () => this.editor.commands.toggleStrike(),
      "Mod-Alt-c": () => {
        this.editor.commands.toggleCodeBlock();
        return true;
      },
      "Mod-k": () => {
        openLink(this.editor);
        return true;
      },
    };
  },
});

/** How a shortcut is printed: `⌘⇧X` on macOS (as in the design), `Ctrl+Shift+X` on Windows and Linux. */
export function shortcutLabel(shortcut: Shortcut, mac: boolean): string {
  if (mac) return `⌘${shortcut.alt === true ? "⌥" : ""}${shortcut.shift === true ? "⇧" : ""}${shortcut.key}`;
  return [
    "Ctrl",
    ...(shortcut.alt === true ? ["Alt"] : []),
    ...(shortcut.shift === true ? ["Shift"] : []),
    shortcut.key,
  ].join("+");
}
