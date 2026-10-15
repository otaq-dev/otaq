// Copying a selection (T-0255, d0078): the clipboard gets rich text (ProseMirror's HTML, unchanged) for Docs, Slack or
// TextEdit, and Markdown as `text/plain` for a terminal or Claude. The Markdown comes from the serializer that saves
// the description, so a copy reads the way the stored text does.

import type { Editor, JSONContent } from "@tiptap/core";
import { Fragment, type Node, type Slice } from "@tiptap/pm/model";

import { withoutTrailingEmptyParagraphs } from "../changes";

/**
 * The Markdown for a copied slice. A selection inside one paragraph, heading, list item or cell gives that text with its
 * formatting (`**12%** of deliveries`), not the list or table around it; inside one code block, the code itself.
 * `null` when the editor has no Markdown support (the editor's plain text is used then).
 */
export function sliceMarkdown(editor: Editor, slice: Slice): string | null {
  if (!editor.markdown) return null;
  const block = innerTextblock(slice);
  if (block?.type.spec.code === true) return block.textContent;
  const content =
    block !== null
      ? Fragment.from(block)
      : slice.content.firstChild?.isInline === true
        ? Fragment.from(editor.schema.nodes.paragraph?.create(null, slice.content))
        : slice.content;
  const doc: JSONContent = { type: "doc", content: (content.toJSON() as JSONContent[] | null) ?? [] };
  return editor.markdown.serialize(withoutTrailingEmptyParagraphs(doc)).replace(/\n+$/, "");
}

/** The one text block a selection lies inside (through the lists, quotes and tables around it), or `null`. */
function innerTextblock({ content, openStart, openEnd }: Slice): Node | null {
  let node = content.childCount === 1 ? content.firstChild : null;
  for (let depth = 0; node !== null && depth < Math.min(openStart, openEnd); depth++) {
    if (node.isTextblock) return node;
    node = node.childCount === 1 ? node.firstChild : null;
  }
  return null;
}
