// The table bar (T-0253, *crewboard.pen → 08 · Ticket editor → f · Caret in a table*): while the caret is in a table in
// edit mode, a small bar floats just above it with the table actions. It's fixed to the window, so the field's
// clipping never cuts it, and drawn inside the editor's surface, so the theme applies. Its buttons keep the caret in
// the text (a press doesn't take focus) and aren't in the tab order, like the toolbar's.
import type { Editor } from "@tiptap/core";
import { useCurrentEditor, useEditorState } from "@tiptap/react";
import { useLayoutEffect, useRef } from "react";

import { Tooltip } from "../../components";
import { Icon } from "../../theme/Icon";
import { placeTableBar, TABLE_ACTIONS, type TableAction, tableAt } from "./tables";
import "./table.css";

/** Drawn inside `MarkdownSurface`; shows only in edit mode, while the text has focus and the caret is in a table. */
export function TableBar() {
  const { editor } = useCurrentEditor();
  return editor ? <Bar editor={editor} /> : null;
}

function Bar({ editor }: { editor: Editor }) {
  const shown = useEditorState({
    editor,
    selector: ({ editor: e }) => {
      if (!e.isEditable || !e.isFocused) return null;
      const table = tableAt(e.state);
      if (table === null) return null;
      return { table, can: TABLE_ACTIONS.map((action) => action.can(e)) };
    },
  });
  const bar = useRef<HTMLDivElement>(null);
  const table = shown?.table ?? null;

  // Placed before paint, and again after every change, scroll or resize: the table moves as text above it changes.
  useLayoutEffect(() => {
    const el = bar.current;
    if (table === null || !el) return;
    const place = () => {
      const dom = editor.view.nodeDOM(table);
      if (!(dom instanceof HTMLElement)) return;
      const box = dom.getBoundingClientRect();
      const { left, top } = placeTableBar(
        box,
        { width: el.offsetWidth, height: el.offsetHeight },
        { width: window.innerWidth, height: window.innerHeight },
      );
      el.style.left = `${String(left)}px`;
      el.style.top = `${String(top)}px`;
      el.style.visibility = "visible";
    };
    place();
    editor.on("transaction", place);
    window.addEventListener("scroll", place, true);
    window.addEventListener("resize", place);
    return () => {
      editor.off("transaction", place);
      window.removeEventListener("scroll", place, true);
      window.removeEventListener("resize", place);
    };
  }, [editor, table]);

  if (shown === null) return null;
  return (
    <div
      ref={bar}
      className="md-table-bar"
      role="toolbar"
      aria-label="Table"
      style={{ visibility: "hidden" }}
    >
      {TABLE_ACTIONS.map((action, index) => (
        <BarButton key={action.id} action={action} enabled={shown.can[index] ?? false} editor={editor} />
      ))}
    </div>
  );
}

function BarButton({ action, enabled, editor }: { action: TableAction; enabled: boolean; editor: Editor }) {
  const iconOnly = action.icon !== undefined && action.danger !== true;
  const button = (
    <button
      type="button"
      className="md-table-bar-button"
      data-icon-only={iconOnly || undefined}
      data-danger={action.danger === true || undefined}
      aria-label={iconOnly ? action.label : undefined}
      disabled={!enabled}
      tabIndex={-1}
      onMouseDown={(event) => {
        // Keeps the caret, and so the bar, in the table.
        event.preventDefault();
      }}
      onClick={() => {
        action.run(editor);
      }}
    >
      {action.icon !== undefined && <Icon name={action.icon} size={iconOnly ? 15 : 13} />}
      {!iconOnly && <span>{action.label}</span>}
    </button>
  );
  return (
    <>
      {action.separated === true && <span className="md-table-bar-rule" aria-hidden />}
      {iconOnly ? <Tooltip text={action.label}>{button}</Tooltip> : button}
    </>
  );
}
