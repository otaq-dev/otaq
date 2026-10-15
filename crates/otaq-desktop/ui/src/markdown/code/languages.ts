// The code block's language picker (T-0254, *08 → g · Code block · language picker*): a short list with a filter. The
// chosen name is what Markdown keeps after the opening fence (```ts); Plain text keeps nothing (```).

import type { Editor } from "@tiptap/core";

/** One row of the picker. */
export interface CodeLanguage {
  /** What goes after the fence, or `null` for Plain text. */
  id: string | null;
  /** The row's text. */
  label: string;
}

export const PLAIN_TEXT: CodeLanguage = { id: null, label: "Plain text" };

/** The picker's rows, in the drawn order. Only these languages are coloured (highlight.ts). */
export const CODE_LANGUAGES: readonly CodeLanguage[] = [
  PLAIN_TEXT,
  ...["bash", "diff", "json", "rust", "ts", "tsx", "yaml", "toml"].map((id) => ({ id, label: id })),
];

/** The rows whose name contains `query` (any case); Plain text also answers to "none". */
export function filterLanguages(query: string): readonly CodeLanguage[] {
  const q = query.trim().toLowerCase();
  if (q === "") return CODE_LANGUAGES;
  return CODE_LANGUAGES.filter(
    (language) => language.label.toLowerCase().includes(q) || (language.id === null && "none".includes(q)),
  );
}

/** The block's label at its top right: its language as written after the fence, or "Plain text". */
export function languageLabel(language: string | null | undefined): string {
  return language === null || language === undefined || language === "" ? PLAIN_TEXT.label : language;
}

/** Whether a row is the block's language (an empty language is Plain text). */
export function isCurrentLanguage(row: CodeLanguage, language: string | null | undefined): boolean {
  return row.id === (language === "" || language === undefined ? null : language);
}

/** Sets the language of the code block at `pos`; it is saved after the opening fence. */
export function setCodeLanguage(editor: Editor, pos: number, language: string | null): boolean {
  return editor
    .chain()
    .command(({ tr }) => {
      const node = tr.doc.nodeAt(pos);
      if (node?.type.name !== "codeBlock") return false;
      tr.setNodeAttribute(pos, "language", language);
      return true;
    })
    .run();
}
