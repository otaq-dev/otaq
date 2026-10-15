// The description editor (T-0249, d0078): one Tiptap editor for every description in the app, editing it in the form
// and the Ticket tab and showing it in read mode (`editable={false}`). The stored value is Markdown both ways.
//
// It comes in two layers:
// - `useMarkdownEditor` owns the Tiptap instance: loading `value`, reporting `onChange`, editable, placeholder, label and
//   the `onKeyDown` pass-through. A screen that shows the same editor in two places (the full-window view, T-0260) calls
//   it once and keeps the instance: text, caret and undo travel with it.
// - `MarkdownSurface` draws an instance: the field (in edit mode), the toolbar slot and the content. `MarkdownEditor` is
//   both together, for the screens that show one editor in one place.
import type { Editor, EditorEvents } from "@tiptap/core";
import { EditorContent, EditorContext, useEditor } from "@tiptap/react";
import { type ReactNode, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import { ChangeTracker, loadMarkdown, serializeMarkdown } from "./changes";
import { markdownExtensions } from "./extensions";
import "./markdown.css";
import { TableBar } from "./table/TableBar";

export interface MarkdownEditorOptions {
  /**
   * The description as Markdown. The editor is controlled: the parent stores what `onChange` reports and passes it back.
   * A `value` the editor didn't report (another ticket, an agent's update) is loaded in its place, without an undo step.
   */
  value: string;
  /**
   * Called with the Markdown after the user changes the text. Never called for a description nobody edited, and a text
   * brought back to what was loaded (typed, then undone) reports `value`'s original string, byte for byte.
   */
  onChange?: (markdown: string) => void;
  /** Off for read mode: nothing can be typed and task boxes don't react. Default on. */
  editable?: boolean;
  /** The muted text shown while the description is empty (edit mode only). */
  placeholder?: string;
  /** The accessible name of the text ("What and why"). */
  label: string;
  /**
   * Sees every key pressed in the text before the editor does (the form's ⌘↵). Return `true`, or call
   * `preventDefault()`, when the key is handled: the editor then leaves it alone. The event still bubbles to the page.
   */
  onKeyDown?: (event: KeyboardEvent) => boolean | undefined;
}

/** The Tiptap instance for one description; it stays the same for the life of the calling component. */
export function useMarkdownEditor({
  value,
  onChange,
  editable = true,
  placeholder = "",
  label,
  onKeyDown,
}: MarkdownEditorOptions): Editor {
  // The latest callbacks, read by the editor's handlers, so the instance never has to be rebuilt for them.
  const latest = useRef({ onChange, onKeyDown, placeholder });
  useLayoutEffect(() => {
    latest.current = { onChange, onKeyDown, placeholder };
  });
  // eslint-disable-next-line react-hooks/refs -- the ref is read each time the placeholder is drawn, never during render
  const [extensions] = useState(() => markdownExtensions({ placeholder: () => latest.current.placeholder }));
  const editorProps = useMemo(
    () => ({
      attributes: { "aria-label": label, "aria-multiline": "true", "aria-readonly": String(!editable) },
      handleKeyDown: (_view: unknown, event: KeyboardEvent) =>
        latest.current.onKeyDown?.(event) === true || event.defaultPrevented,
    }),
    [editable, label],
  );
  // Created empty: `value` is loaded below, the same way as every later value.
  const editor = useEditor({
    extensions,
    editable,
    editorProps,
    immediatelyRender: true,
    shouldRerenderOnTransaction: false,
  });

  // What the parent holds, and how the loaded description saves untouched (set when it's loaded).
  const [tracker] = useState(() => new ChangeTracker(value, value));
  const loadedInto = useRef<Editor | null>(null);

  // Loads `value` into a new instance, and any value the editor didn't report itself. Before paint, and before the
  // user can touch anything, so the baseline is the untouched document.
  useLayoutEffect(() => {
    if (loadedInto.current === editor && value === tracker.value) return;
    loadedInto.current = editor;
    loadMarkdown(editor, value);
    tracker.reload(value, serializeMarkdown(editor));
  }, [editor, tracker, value]);

  useEffect(() => {
    const onUpdate = ({ editor: changed, transaction }: EditorEvents["update"]) => {
      // Only a change the user made counts. The editor tidying its own document (the trailing paragraph it adds after
      // the first click, a table fixed up) comes as an appended transaction to one that changed nothing.
      if (!transaction.docChanged) return;
      const next = tracker.report(serializeMarkdown(changed));
      if (next !== null) latest.current.onChange?.(next);
    };
    editor.on("update", onUpdate);
    return () => {
      editor.off("update", onUpdate);
    };
  }, [editor, tracker]);

  useEffect(() => {
    editor.setEditable(editable, false);
  }, [editor, editable]);

  return editor;
}

interface MarkdownSurfaceProps {
  editor: Editor;
  /** Drawn inside the field, above the text, in edit mode: the toolbar (T-0250). */
  toolbar?: ReactNode;
  className?: string;
}

/**
 * Draws a description editor. In edit mode it is a field (border, focus ring, padding); in read mode it is the text
 * alone, set in the label row's column. Everything inside it can reach the instance with Tiptap's `useCurrentEditor`.
 */
export function MarkdownSurface({ editor, toolbar, className }: MarkdownSurfaceProps) {
  const context = useMemo(() => ({ editor }), [editor]);
  return (
    <EditorContext.Provider value={context}>
      <div className={className === undefined ? "md-editor" : `md-editor ${className}`}>
        {toolbar}
        <EditorContent editor={editor} className="md-content" />
        <TableBar />
      </div>
    </EditorContext.Provider>
  );
}

export interface MarkdownEditorProps extends MarkdownEditorOptions {
  /** Drawn inside the field, above the text, in edit mode: the toolbar (T-0250). */
  toolbar?: ReactNode;
  className?: string;
}

/** `<MarkdownEditor>`: a description, editable or read-only, in one place. */
export function MarkdownEditor({ toolbar, className, ...options }: MarkdownEditorProps) {
  const editor = useMarkdownEditor(options);
  return (
    <MarkdownSurface
      editor={editor}
      {...(toolbar === undefined ? {} : { toolbar })}
      {...(className === undefined ? {} : { className })}
    />
  );
}
