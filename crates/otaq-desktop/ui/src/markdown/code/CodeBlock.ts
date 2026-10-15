// The description's code block (T-0254, d0078): Tiptap's lowlight code block with the Diff tab's colours
// (highlight.ts), a fence that fits the code (fence.ts, Q48), the language label and Copy (CodeBlockView.tsx), and
// ``` on an empty last line to leave it. ↓ at the end leaves it too (Tiptap's own). Tab and ⇧Tab indent and outdent
// the line, or every selected line, by two spaces (Tiptap's own, T-0259, d0078 D3); Esc then Tab leaves the editor.

import type { Editor } from "@tiptap/core";
import { CodeBlockLowlight } from "@tiptap/extension-code-block-lowlight";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { ReactNodeViewRenderer } from "@tiptap/react";

import { CodeBlockView } from "./CodeBlockView";
import { codeBlockMarkdown } from "./fence";
import { codeLowlight } from "./highlight";

/**
 * Typing the third backtick of ``` on the block's last line, with nothing else on it, removes that line and moves the
 * caret into a new paragraph below the block, as closing the fence would in Markdown. Returns whether it did.
 */
export function leaveOnClosingFence(editor: Editor, from: number, to: number, text: string): boolean {
  const { $from } = editor.state.selection;
  if (text !== "`" || from !== to || $from.pos !== from || $from.parent.type.name !== "codeBlock")
    return false;
  const code = $from.parent.textContent;
  const offset = $from.parentOffset;
  if (offset !== code.length) return false;
  const lineStart = code.lastIndexOf("\n", offset - 1) + 1;
  if (code.slice(lineStart, offset) !== "``") return false;
  // The line goes, with the line break before it when there is one.
  const deleteFrom = $from.start() + Math.max(0, lineStart - 1);
  return editor
    .chain()
    .command(({ tr }) => {
      tr.delete(deleteFrom, from);
      return true;
    })
    .exitCode()
    .run();
}

export const CodeBlock = CodeBlockLowlight.extend({
  renderMarkdown: (node, helpers) =>
    codeBlockMarkdown(
      (node.attrs?.language as string | null | undefined) || null,
      node.content ? helpers.renderChildren(node.content) : "",
    ),

  addNodeView() {
    return ReactNodeViewRenderer(CodeBlockView, {
      contentDOMElementTag: "span",
      // The label, picker and Copy are the block's own controls: the editor leaves their presses and keys alone.
      stopEvent: ({ event }) =>
        event.target instanceof Element && event.target.closest(".md-code-tools") !== null,
    });
  },

  addProseMirrorPlugins() {
    const editor = this.editor;
    return [
      ...(this.parent?.() ?? []),
      new Plugin({
        key: new PluginKey("codeBlockClosingFence"),
        props: {
          handleTextInput: (_view, from, to, text) => leaveOnClosingFence(editor, from, to, text),
        },
      }),
    ];
  },
}).configure({ lowlight: codeLowlight, enableTabIndentation: true, tabSize: 2 });
