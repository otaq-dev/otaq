// The note under the content after a paste left out images or embeds (T-0255, d0078 D6; *crewboard.pen → 08 · Ticket
// editor → 6 · Copy and paste*, "dropped note"): a muted line with an info icon, "1 image left out: descriptions are
// Markdown only". It goes at the next edit (the paste plugin clears it).
import { useCurrentEditor, useEditorState } from "@tiptap/react";

import { Icon } from "../../theme/Icon";
import { droppedNote } from "./dropped";
import { droppedByPaste } from "./paste";
import "./paste.css";

/** Drawn inside `MarkdownSurface`, under the text. */
export function PasteNote() {
  const { editor } = useCurrentEditor();
  const note = useEditorState({
    editor,
    selector: ({ editor: e }) => {
      const dropped = e ? droppedByPaste(e.state) : null;
      return dropped === null ? null : droppedNote(dropped);
    },
  });
  if (note === null) return null;
  return (
    <div className="md-paste-note" role="status">
      <Icon name="info" size={12} />
      <span>{note}</span>
    </div>
  );
}
