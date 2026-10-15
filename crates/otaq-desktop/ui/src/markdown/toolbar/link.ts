// The Link hook: the one place the Link button and ⌘K go. T-0252 replaces `openLink`'s body with its popover
// (`markdown/link/`); until then it adds or removes a link without one.
import type { Editor } from "@tiptap/core";

const URL_LIKE = /^(https?:|mailto:)\S+$/i;

/**
 * The Link button and ⌘K (d0078 D2). In a link it removes the link; on a selection it links it, to the selected text
 * when that is a URL, else to `https://` for the user to fill in.
 */
export function openLink(editor: Editor): void {
  if (editor.isActive("link")) {
    editor.chain().focus().extendMarkRange("link").unsetLink().run();
    return;
  }
  const { from, to, empty } = editor.state.selection;
  if (empty) return;
  const text = editor.state.doc.textBetween(from, to).trim();
  editor
    .chain()
    .focus()
    .setLink({ href: URL_LIKE.test(text) ? text : "https://" })
    .run();
}
