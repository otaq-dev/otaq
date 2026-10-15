// Copy and paste in the description editor (T-0255, TKT-11, d0078 D6): the extension list's `CopyPaste` line.
// - Pasted HTML keeps what Markdown can hold and leaves out the rest (`cleanPastedHtml`).
// - Pasted plain text that looks like Markdown becomes formatting; other plain text is pasted line by line.
// - ⌘⇧V / Ctrl+Shift+V pastes plain text, with no formatting and no Markdown reading.
// - Images and embeds left out are counted for the note under the content, which stays until the next edit.
// - A copied selection carries Markdown as its plain text, next to ProseMirror's HTML.
// None of it changes the document by itself: the note's count rides on the paste's own transaction, or on one that
// changes nothing (an image-only paste), so a description nobody edited is never re-saved.

import { type Editor, Extension } from "@tiptap/core";
import { Slice } from "@tiptap/pm/model";
import { type EditorState, Plugin, PluginKey } from "@tiptap/pm/state";

import { wrapLooseInline } from "../changes";
import { sliceMarkdown } from "./copy";
import { type Dropped, droppedAny, NOTHING_DROPPED } from "./dropped";
import { cleanPastedHtml } from "./html";
import { looksLikeMarkdown, stripMarkdownImages } from "./markdown";

const key = new PluginKey<Dropped | null>("copyPaste");

/** What the last paste left out, while the note shows (until the next edit); `null` when there's no note. */
export function droppedByPaste(state: EditorState): Dropped | null {
  return key.getState(state) ?? null;
}

/**
 * Pasted Markdown as a slice of the editor's document. ProseMirror then fits it in as it does pasted HTML: a one-line
 * paste joins the paragraph at the caret. `null` without Markdown support.
 */
export function markdownSlice(editor: Editor, markdown: string): Slice | null {
  if (!editor.markdown) return null;
  const json = wrapLooseInline(editor.markdown.parse(markdown), editor.schema);
  return new Slice(editor.schema.nodeFromJSON(json).content, 0, 0);
}

/** Pastes `text` as plain text at the selection (⌘⇧V): no formatting, no Markdown; one paragraph per line. */
export function pastePlainText(editor: Editor, text: string): void {
  editor.view.pasteText(text);
}

export const CopyPaste = Extension.create({
  name: "copyPaste",
  // Before Tiptap's own clipboard text (default 100), so a copy's plain text is Markdown.
  priority: 1000,

  // Tiptap's paste rules read Markdown signs in any pasted text (`2 * 3 * 4` turns italic), even with ⌘⇧V. Here a
  // paste reads Markdown only when the text looks like Markdown, so they're off. The editor builds its plugins after
  // this, when it's mounted.
  onBeforeCreate() {
    this.editor.options.enablePasteRules = false;
  },

  addKeyboardShortcuts() {
    return {
      "Mod-Shift-v": () => {
        const { editor } = this;
        if (!editor.isEditable) return false;
        // The webview has no "paste as plain text" of its own on macOS, and Chromium's (Windows) would bring the
        // HTML's text: read the clipboard's text and paste that. Taking the key stops the webview's own paste.
        void navigator.clipboard.readText().then(
          (text) => {
            if (!editor.isDestroyed && text !== "") pastePlainText(editor, text);
          },
          (err: unknown) => {
            console.warn("paste as plain text failed", err);
          },
        );
        return true;
      },
    };
  },

  addProseMirrorPlugins() {
    const { editor } = this;
    // What the paste being read leaves out; taken by its transaction.
    let pending: Dropped = NOTHING_DROPPED;
    const readMarkdown = (text: string): Slice | null => {
      const stripped = stripMarkdownImages(text);
      pending = stripped.dropped;
      return markdownSlice(editor, stripped.markdown);
    };
    return [
      new Plugin<Dropped | null>({
        key,
        state: {
          init: () => null,
          apply(tr, note) {
            const own = tr.getMeta(key) as Dropped | undefined;
            if (own !== undefined) return droppedAny(own) ? own : null;
            if (tr.getMeta("uiEvent") === "paste") {
              const dropped = pending;
              pending = NOTHING_DROPPED;
              return droppedAny(dropped) ? dropped : null;
            }
            // The editor tidying up after the paste (a table fixed, the trailing paragraph) isn't the next edit.
            if (tr.getMeta("appendedTransaction") !== undefined) return note;
            return tr.docChanged ? null : note;
          },
        },
        props: {
          // A paste reads either text (here first) or HTML: each sets what it leaves out.
          transformPastedText(text) {
            pending = NOTHING_DROPPED;
            return text;
          },
          transformPastedHTML(html) {
            const cleaned = cleanPastedHtml(html);
            pending = cleaned.dropped;
            return cleaned.html;
          },
          // `null` leaves the text to ProseMirror (one paragraph per line); its type doesn't say it may.
          clipboardTextParser(text, _context, plain) {
            const slice = plain || !looksLikeMarkdown(text) ? null : readMarkdown(text);
            return slice as Slice;
          },
          handlePaste(view, event) {
            // Only files on the clipboard (a screenshot, an image copied in Preview): nothing to paste, but the note
            // says the image was left out. The transaction carries the count and changes nothing.
            const data = event.clipboardData;
            if (data === null || data.getData("text/plain") !== "" || data.getData("text/html") !== "")
              return false;
            const images = [...data.files].filter((file) => file.type.startsWith("image/")).length;
            if (images === 0) return false;
            view.dispatch(view.state.tr.setMeta(key, { images, embeds: 0 }));
            return true;
          },
          clipboardTextSerializer: (slice) => sliceMarkdown(editor, slice) ?? "",
        },
      }),
    ];
  },
});
