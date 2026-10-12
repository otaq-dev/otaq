// The Diff tab's interaction state (UIT-04): the file filter, folded folders, which files load, the chunks long files
// are drawn in, and the comment box being edited. Everything worded or decided by the board's rules comes from Rust
// (`diff_page.rs`).
import type { DiffAnchor } from "../../generated/DiffAnchor";
import type { DiffComments } from "../../generated/DiffComments";
import type { DiffFileRow } from "../../generated/DiffFileRow";
import type { DiffGroup } from "../../generated/DiffGroup";
import type { DiffThread } from "../../generated/DiffThread";

/** One diff line's height (the frame's 20 px rows); placeholders and chunk sizes are counted in it. */
export const LINE_HEIGHT = 20;
/** Long files are drawn in chunks of this many rows; the webview skips laying out the chunks out of view. */
export const CHUNK_ROWS = 120;
/** The file list's width, narrower in a narrow panel (the GPUI app's). */
export const FILE_LIST = 280;
export const FILE_LIST_NARROW = 200;
export const NARROW_BELOW = 820;

export function fileListWidth(width: number): number {
  return width < NARROW_BELOW ? FILE_LIST_NARROW : FILE_LIST;
}

/** A path cut after each "/", where it may wrap. */
export function pathParts(path: string): string[] {
  return path.split(/(?<=\/)/);
}

/** Side-by-side when chosen and the panel is wide enough; Unified otherwise (*Side-by-side*). */
export function showsSideBySide(chosen: boolean, width: number, from: number): boolean {
  return chosen && width >= from;
}

/** Whether a file's label matches the filter (case-insensitive; an empty filter matches everything). */
export function matchesFilter(file: DiffFileRow, filter: string): boolean {
  const wanted = filter.trim().toLowerCase();
  return wanted === "" || file.label.toLowerCase().includes(wanted);
}

/** A long list's folders with only the files matching the filter; folders left empty go. */
export function filteredGroups(
  groups: readonly DiffGroup[],
  files: readonly DiffFileRow[],
  filter: string,
): DiffGroup[] {
  return groups
    .map((g) => ({
      ...g,
      files: g.files.filter((i) => {
        const file = files[i];
        return file !== undefined && matchesFilter(file, filter);
      }),
    }))
    .filter((g) => g.files.length > 0);
}

/** Whether a folder is folded: the generated one starts folded; a click flips it. */
export function groupFolded(group: DiffGroup, toggled: ReadonlySet<string>): boolean {
  return group.starts_folded !== toggled.has(group.name);
}

/** A set with `item` flipped. */
export function toggled<T>(set: ReadonlySet<T>, item: T): Set<T> {
  const next = new Set(set);
  if (!next.delete(item)) next.add(item);
  return next;
}

/** Whether a file shows its lines: binary files never do; collapsed ones after "Load diff". */
export function showsLines(file: DiffFileRow, opened: ReadonlySet<string>): boolean {
  return !file.binary && (file.collapsed === null || opened.has(file.path));
}

/** `[start, end)` ranges of `count` rows, `size` at a time. */
export function chunks(count: number, size: number = CHUNK_ROWS): [number, number][] {
  const out: [number, number][] = [];
  for (let start = 0; start < count; start += size) out.push([start, Math.min(count, start + size)]);
  return out;
}

/** The threads by their anchor's key. */
export function threadsByAnchor(comments: DiffComments | undefined): Map<string, DiffThread> {
  return new Map((comments?.threads ?? []).map((t) => [t.anchor.key, t]));
}

/** The distinct anchors of a side-by-side row's two halves (a context line is the same anchor on both). */
export function rowAnchors(
  left: DiffAnchor | null | undefined,
  right: DiffAnchor | null | undefined,
): DiffAnchor[] {
  const out: DiffAnchor[] = [];
  for (const a of [left, right]) {
    if (a && !out.some((b) => b.key === a.key)) out.push(a);
  }
  return out;
}

/** The open comment box: a new comment or reply at an anchor, or the edit of a pending comment. The text being typed
 * stays in the box itself, so typing doesn't redraw the diff. */
export interface Composer {
  anchor: DiffAnchor;
  /** The pending comment being edited. */
  editing: string | null;
  /** The thread's first comment, when this is a reply. */
  replyTo: string | null;
  /** The text the box opens with (the comment being edited). */
  initial: string;
  saving: boolean;
  error: string | null;
}

export type ComposerAction =
  | { kind: "open"; anchor: DiffAnchor; replyTo?: string | null }
  | { kind: "edit"; anchor: DiffAnchor; id: string; body: string }
  | { kind: "saving" }
  | { kind: "saved" }
  | { kind: "failed"; message: string }
  | { kind: "cancel" };

export function composerReducer(state: Composer | null, action: ComposerAction): Composer | null {
  switch (action.kind) {
    case "open":
      return {
        anchor: action.anchor,
        editing: null,
        replyTo: action.replyTo ?? null,
        initial: "",
        saving: false,
        error: null,
      };
    case "edit":
      return {
        anchor: action.anchor,
        editing: action.id,
        replyTo: null,
        initial: action.body,
        saving: false,
        error: null,
      };
    case "saving":
      return state && { ...state, saving: true, error: null };
    case "failed":
      return state && { ...state, saving: false, error: action.message };
    case "saved":
    case "cancel":
      return null;
  }
}

/** Whether Save can be pressed: some text, and no save on its way (one click, one comment). */
export function canSave(state: Composer | null, text: string): boolean {
  return state !== null && !state.saving && text.trim() !== "";
}

/** The request Save sends: an edit, a reply in the thread, or a new comment on the line. */
export function saveRequest(
  state: Composer,
  text: string,
  ticket: string,
): { method: string; params: Record<string, unknown> } {
  const body = text.trim();
  if (state.editing !== null) return { method: "diff.comment_edit", params: { id: state.editing, body } };
  if (state.replyTo !== null) return { method: "diff.comment_reply", params: { id: state.replyTo, body } };
  const { file, line, side } = state.anchor;
  return { method: "diff.comment_add", params: { key: ticket, file, line, side, body } };
}

/** ⌘↵ (Ctrl+↵ elsewhere) saves the comment; Esc cancels. */
export function composerKey(e: {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
}): "save" | "cancel" | null {
  if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) return "save";
  if (e.key === "Escape") return "cancel";
  return null;
}
