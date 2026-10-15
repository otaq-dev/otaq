// The Markdown source toggle (T-0251, d0078 D9): the description shown as its raw Markdown in a plain text area, and
// back. Edits made in either mode carry over, and switching never counts as an edit (the untouched rule, changes.ts):
// - The source shows what the parent holds: the stored string, byte for byte, until the user edits, then the
//   editor's Markdown (`serializeMarkdown`, which is what the editor reported).
// - Typing in the source reports the typed text, as is. The editor keeps the document it had when the source opened,
//   so nothing is parsed while the source shows.
// - Back to rich text, the editor is given what the parent holds: unchanged if nobody typed (no reload, no report,
//   undo kept), else the typed text, which it loads like any new value (`loadMarkdown`: no undo step, no report).

/** Which mode an editor shows, and what the editor was given when the source opened. */
export interface SourceState {
  on: boolean;
  /** The parent's value when the source opened: the editor keeps it while the source shows. */
  entered: string;
  /** The user switched in this editor (the text that appears gets the caret); false for the mode it opened in. */
  switched: boolean;
}

/** An editor opening in a mode, holding `value`. */
export function sourceStart(on: boolean, value: string): SourceState {
  return { on, entered: value, switched: false };
}

/** The toggle pressed while the parent holds `value`. Nothing is reported to the parent. */
export function switchSource(state: SourceState, value: string): SourceState {
  return { on: !state.on, entered: value, switched: true };
}

/** The value to give the rich editor: what it had when the source opened, while the source shows. */
export function editorValue(state: SourceState, value: string): string {
  return state.on ? state.entered : value;
}

/** The mode last chosen in this run of the app, once the toggle was used: newer than a view loaded before it. */
let chosen: boolean | null = null;

/** The mode a new editor opens in: the last one chosen, else the one the view reported (the stored setting). */
export function openingMode(reported: boolean): boolean {
  return chosen ?? reported;
}

/** Remembers a choice for the editors opened after it. */
export function rememberMode(on: boolean): void {
  chosen = on;
}
