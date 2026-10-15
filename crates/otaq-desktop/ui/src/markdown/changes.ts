// What the description editor reports as changed (T-0249, d0078). The rule every M15 ticket keeps: a description nobody
// edited is never re-saved, because Tiptap spells some Markdown differently on a save (Q48, tickets/m15-roundtrip.md).
// So `onChange` is called only after a change the user made, and a document that comes back to what was loaded (typed,
// then undone) reports the original string, byte for byte.

import type { Editor, JSONContent } from "@tiptap/core";
import type { Schema } from "@tiptap/pm/model";

/**
 * The parsed document with any loose inline content wrapped in paragraphs. Tiptap's Markdown parser leaves a paragraph
 * that holds only an image (`![logo](…)`, kept as its alt text) as bare text where a block belongs, and such a document
 * breaks on the first click; wrapped, it reads and saves the same.
 */
export function wrapLooseInline(node: JSONContent, schema: Schema): JSONContent {
  if (node.content === undefined) return node;
  const children = node.content.map((child) => wrapLooseInline(child, schema));
  const type = node.type === undefined ? undefined : schema.nodes[node.type];
  if (type === undefined || type.inlineContent) return { ...node, content: children };
  const content: JSONContent[] = [];
  let run: JSONContent[] | null = null;
  for (const child of children) {
    const inline = child.type === "text" || (child.type !== undefined && schema.nodes[child.type]?.isInline);
    if (!inline) {
      run = null;
      content.push(child);
    } else if (run === null) {
      run = [child];
      content.push({ type: "paragraph", content: run });
    } else {
      run.push(child);
    }
  }
  return { ...node, content };
}

/** Loads a stored description into the editor, without an undo step and without counting as a change. */
export function loadMarkdown(editor: Editor, markdown: string): void {
  const content = editor.markdown
    ? wrapLooseInline(editor.markdown.parse(markdown), editor.schema)
    : markdown;
  editor
    .chain()
    .setMeta("addToHistory", false)
    .setContent(content, { contentType: editor.markdown ? "json" : "markdown", emitUpdate: false })
    .run();
}

/**
 * The document without the empty paragraphs at its end. The editor adds one after a last table, list or code block so
 * the caret can get below it (StarterKit's trailing node); it carries no text, so it's left out of the saved Markdown.
 */
export function withoutTrailingEmptyParagraphs(doc: JSONContent): JSONContent {
  const content = [...(doc.content ?? [])];
  while (content.length > 0) {
    const last = content[content.length - 1];
    if (last?.type !== "paragraph" || (last.content?.length ?? 0) > 0) break;
    content.pop();
  }
  return { ...doc, content };
}

/** The editor's document as the Markdown that gets saved. */
export function serializeMarkdown(editor: Editor): string {
  const doc = withoutTrailingEmptyParagraphs(editor.getJSON());
  return editor.markdown ? editor.markdown.serialize(doc) : editor.getMarkdown();
}

/**
 * Keeps what the parent holds in step with the editor. It starts at the loaded string; `report` turns the editor's
 * Markdown after a user change into the text to send up, or `null` when the parent already has it.
 */
export class ChangeTracker {
  private original: string;
  private baseline: string;
  private reported: string;

  /** `original` is the stored description; `baseline` is how the editor saves it untouched. */
  constructor(original: string, baseline: string) {
    this.original = original;
    this.baseline = baseline;
    this.reported = original;
  }

  /** The text the parent holds now: the original until the user changes something. */
  get value(): string {
    return this.reported;
  }

  /** A new stored description was loaded into the editor (not a user change): start over from it. */
  reload(original: string, baseline: string): void {
    this.original = original;
    this.baseline = baseline;
    this.reported = original;
  }

  /** The text to report for the editor's Markdown after a user change, or `null` if nothing new is to be reported. */
  report(markdown: string): string | null {
    const next = markdown === this.baseline ? this.original : markdown;
    if (next === this.reported) return null;
    this.reported = next;
    return next;
  }
}
