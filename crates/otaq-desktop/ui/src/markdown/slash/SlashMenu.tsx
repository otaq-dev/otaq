// The `/` list (T-0465; `slash.ts`): drawn inside `MarkdownSurface` like the table bar, fixed under the caret so the
// field's clipping never cuts it. While it shows, ↑ ↓ ↵ and Esc are its own: they are taken before the editor sees
// them (a capture listener on the text), so Esc closes the list first, as every popover does (d0078 D4). A press
// on a row doesn't take the caret out of the text.
import type { Editor } from "@tiptap/core";
import { useCurrentEditor, useEditorState } from "@tiptap/react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { Icon } from "../../theme/Icon";
import { applyItem, filterItems, type SlashQuery, slashAt, stepItem } from "./slash";
import "./slash.css";

/** Drawn inside `MarkdownSurface`; shows only in edit mode, while the text has focus and a `/` query is open. */
export function SlashMenu() {
  const { editor } = useCurrentEditor();
  return editor ? <Menu editor={editor} /> : null;
}

function sameQuery(a: SlashQuery | null, b: SlashQuery | null): boolean {
  return a?.from === b?.from && a?.to === b?.to && a?.query === b?.query;
}

function Menu({ editor }: { editor: Editor }) {
  const slash = useEditorState({
    editor,
    selector: ({ editor: e }) => (e.isEditable && e.isFocused ? slashAt(e.state) : null),
    equalityFn: sameQuery,
  });
  // Esc closes the list for this `/`; it opens again once a new `/` is typed.
  const [closedAt, setClosedAt] = useState<number | null>(null);
  const [at, setAt] = useState(0);
  const [lastQuery, setLastQuery] = useState<string | null>(null);
  const shown = slash !== null && slash.from !== closedAt ? slash : null;
  const items = shown ? filterItems(shown.query) : [];
  if ((shown?.query ?? null) !== lastQuery) {
    setLastQuery(shown?.query ?? null);
    setAt(0);
  }
  if (slash === null && closedAt !== null) setClosedAt(null);
  const list = useRef<HTMLDivElement>(null);

  const latest = useRef({ shown, items, at });
  useLayoutEffect(() => {
    latest.current = { shown, items, at };
  });

  useEffect(() => {
    const dom = editor.view.dom;
    const onKey = (e: KeyboardEvent) => {
      const { shown: open, items: rows, at: row } = latest.current;
      if (open === null || e.isComposing) return;
      let taken = true;
      if (e.key === "ArrowDown") setAt(stepItem(row, 1, rows.length));
      else if (e.key === "ArrowUp") setAt(stepItem(row, -1, rows.length));
      else if (e.key === "Enter" && rows[row]) applyItem(editor, open, rows[row]);
      else if (e.key === "Escape") setClosedAt(open.from);
      else taken = false;
      if (!taken) return;
      e.preventDefault();
      e.stopImmediatePropagation();
    };
    dom.addEventListener("keydown", onKey, true);
    return () => {
      dom.removeEventListener("keydown", onKey, true);
    };
  }, [editor]);

  // Under the caret, kept inside the window.
  useLayoutEffect(() => {
    const el = list.current;
    if (shown === null || !el) return;
    const caret = editor.view.coordsAtPos(shown.to);
    const top = Math.min(caret.bottom + 4, window.innerHeight - el.offsetHeight - 8);
    const left = Math.min(caret.left, window.innerWidth - el.offsetWidth - 8);
    el.style.top = `${String(Math.max(top, 8))}px`;
    el.style.left = `${String(Math.max(left, 8))}px`;
    el.style.visibility = "visible";
  });

  if (shown === null || items.length === 0) return null;
  return (
    <div
      ref={list}
      className="md-slash"
      role="listbox"
      aria-label="Blocks"
      data-layer
      style={{ visibility: "hidden" }}
    >
      {items.map((item, i) => (
        // eslint-disable-next-line jsx-a11y/click-events-have-key-events -- ↑ ↓ ↵ reach the rows through the text's own keys (the caret never leaves it)
        <div
          key={item.id}
          role="option"
          tabIndex={-1}
          aria-selected={i === at}
          className="md-slash-item"
          onMouseDown={(e) => {
            // Keeps the caret in the text.
            e.preventDefault();
          }}
          onMouseEnter={() => {
            setAt(i);
          }}
          onClick={() => {
            applyItem(editor, shown, item);
          }}
        >
          <Icon name={item.icon} size={14} />
          <span>{item.label}</span>
        </div>
      ))}
    </div>
  );
}
