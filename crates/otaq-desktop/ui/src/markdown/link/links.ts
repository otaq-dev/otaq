// Links in a description (T-0252, d0078 D2): what the popover edits, and the extension that watches the pointer over
// links and keeps links from ever opening inside Otaq. Opening the popover or hovering a link never touches the
// document: the state lives beside the editor, in a small store per instance (`linkUi`).
import { Extension, getMarkRange, type Editor } from "@tiptap/core";
import type { EditorState } from "@tiptap/pm/state";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

import { createStore, type Store } from "../../app/store";
import { openInBrowser } from "./url";

/** The text a link goes on and its current address; `from === to` inserts the URL itself as the text. */
export interface LinkTarget {
  from: number;
  to: number;
  /** The link's address, or the selected text when that is a URL, else "". */
  href: string;
  /** Whether the text already holds a link (Remove has something to remove). */
  linked: boolean;
}

export interface LinkUiState {
  /** The open popover's target, or `null` when it is closed. */
  popover: LinkTarget | null;
  /** The link under the pointer. */
  hover: HTMLAnchorElement | null;
}

const CLOSED: LinkUiState = { popover: null, hover: null };
const stores = new WeakMap<Editor, Store<LinkUiState>>();

/** The link popover and hover state of one editor instance. */
export function linkUi(editor: Editor): Store<LinkUiState> {
  let store = stores.get(editor);
  if (!store) {
    store = createStore(CLOSED);
    stores.set(editor, store);
  }
  return store;
}

const URL_LIKE = /^https?:\S+$/i;

/**
 * What ⌘K edits where the caret is: the link around the caret, or the selection (linked already or not), or the caret
 * alone, where Apply types the URL as the link's text.
 */
export function linkTargetAt(state: EditorState): LinkTarget {
  const type = state.schema.marks.link;
  const { from, to, empty, $from } = state.selection;
  if (!type) return { from, to, href: "", linked: false };
  if (empty) {
    const range = getMarkRange($from, type);
    if (range) {
      const mark = state.doc.nodeAt(range.from)?.marks.find((m) => m.type === type);
      return { ...range, href: String(mark?.attrs.href ?? ""), linked: true };
    }
    return { from, to, href: "", linked: false };
  }
  const hrefs: string[] = [];
  state.doc.nodesBetween(from, to, (node) => {
    const mark = type.isInSet(node.marks);
    if (mark) hrefs.push(String(mark.attrs.href ?? ""));
  });
  const [href] = hrefs;
  if (href !== undefined) return { from, to, href, linked: true };
  const text = state.doc.textBetween(from, to).trim();
  return { from, to, href: URL_LIKE.test(text) ? text : "", linked: false };
}

const linkKey = new PluginKey("linkUi");

/** Sets the popover and redraws the text's highlight of its target, with a transaction that changes nothing. */
export function setLinkPopover(editor: Editor, popover: LinkTarget | null): void {
  linkUi(editor).update((s) => (s.popover === popover ? s : { ...s, popover, hover: null }));
  if (!editor.isDestroyed)
    editor.view.dispatch(editor.state.tr.setMeta(linkKey, true).setMeta("addToHistory", false));
}

/** Opens the popover on what the caret is on (the Link button and ⌘K). Read mode has no popover. */
export function openLinkPopover(editor: Editor): void {
  if (!editor.isEditable) return;
  setLinkPopover(editor, linkTargetAt(editor.state));
}

export function closeLinkPopover(editor: Editor): void {
  if (linkUi(editor).get().popover !== null) setLinkPopover(editor, null);
}

/**
 * Links `target` to `href` (already checked), or types `href` as a new link at the caret. The caret ends after the
 * link, and what is typed next isn't part of it.
 */
export function applyLink(editor: Editor, target: LinkTarget, href: string): void {
  const chain = editor.chain().focus();
  if (target.from === target.to) {
    chain
      .insertContentAt(target.from, { type: "text", text: href, marks: [{ type: "link", attrs: { href } }] })
      .setTextSelection(target.from + href.length);
  } else {
    chain
      .setTextSelection({ from: target.from, to: target.to })
      .extendMarkRange("link")
      .setLink({ href })
      .setTextSelection(target.to);
  }
  chain.unsetMark("link").run();
}

/** Takes the link off `target`'s text; the text stays. */
export function removeLink(editor: Editor, target: LinkTarget): void {
  editor
    .chain()
    .focus()
    .setTextSelection({ from: target.from, to: target.to })
    .extendMarkRange("link")
    .unsetLink()
    .setTextSelection(target.to)
    .run();
}

/** The target of a link element in the text (the bubble's Edit and Remove on a hovered link). */
export function linkTargetOf(editor: Editor, element: HTMLAnchorElement): LinkTarget | null {
  let pos: number;
  try {
    pos = editor.view.posAtDOM(element, 0);
  } catch {
    return null;
  }
  const type = editor.schema.marks.link;
  const range = type && getMarkRange(editor.state.doc.resolve(pos), type);
  if (!range) return null;
  return { ...range, href: element.getAttribute("href") ?? "", linked: true };
}

const anchorIn = (root: HTMLElement, target: EventTarget | null): HTMLAnchorElement | null => {
  const a = target instanceof Element ? target.closest("a[href]") : null;
  return a instanceof HTMLAnchorElement && root.contains(a) ? a : null;
};

/**
 * The link behaviour: a click on a link never follows it inside the webview; in read mode it opens the link in the
 * system browser. Hovering a link is reported to `linkUi` for the bubble (edit mode) and the tooltip (read mode). The
 * popover's target follows edits made while it's open, and keeps the selection's highlight while the field has the focus.
 */
export const LinkUi = Extension.create({
  name: "linkUi",
  onTransaction({ transaction }) {
    if (!transaction.docChanged) return;
    const store = linkUi(this.editor);
    const open = store.get().popover;
    if (!open) return;
    const from = transaction.mapping.map(open.from, -1);
    const to = transaction.mapping.map(open.to, 1);
    store.update((s) => ({ ...s, popover: { ...open, from, to: Math.max(from, to) } }));
  },
  onDestroy() {
    stores.delete(this.editor);
  },
  addProseMirrorPlugins() {
    const editor = this.editor;
    const setHover = (hover: HTMLAnchorElement | null) => {
      linkUi(editor).update((s) => (s.hover === hover ? s : { ...s, hover }));
    };
    return [
      new Plugin({
        key: linkKey,
        props: {
          decorations: (state) => {
            const open = linkUi(editor).get().popover;
            if (!open || open.from === open.to) return null;
            return DecorationSet.create(state.doc, [
              Decoration.inline(open.from, open.to, { class: "md-link-editing" }),
            ]);
          },
          handleDOMEvents: {
            click: (view, event) => {
              const a = anchorIn(view.dom, event.target);
              if (!a) return false;
              event.preventDefault();
              if (view.editable) return false;
              openInBrowser(a.getAttribute("href") ?? "");
              return true;
            },
            auxclick: (view, event) => {
              if (anchorIn(view.dom, event.target)) event.preventDefault();
              return false;
            },
            mouseover: (view, event) => {
              setHover(anchorIn(view.dom, event.target));
              return false;
            },
            mouseout: (view, event) => {
              const next = anchorIn(view.dom, event.relatedTarget);
              if (next !== anchorIn(view.dom, event.target)) setHover(next);
              return false;
            },
          },
        },
      }),
    ];
  },
});
