// The description edited in place (SHT-03, d0078; T-0465; *25 → Editing the description*): the moment of typing that
// React keeps. Whether the text changed, when the tab row shows "Description changed · Discard · Save ⌘S" and when a
// move away asks first follow Rust's `overview.rs` (`state`, `save_bar`, `leaving`); its words come with
// `OverviewView`. The open editor publishes itself in `descriptionDraft`, so the sheet's tab row can draw the bar and
// press its buttons.
import type { Selection } from "../../app/selection";
import { createStore, type Store } from "../../app/store";
import type { DescriptionSaveBar } from "../../generated/DescriptionSaveBar";
import type { DescriptionState } from "../../generated/DescriptionState";
import type { SheetContent } from "../../generated/SheetContent";

/** `overview::state`: reading, editing with the saved text, or changed. Back to the saved text is unchanged again. */
export function descriptionState(saved: string, text: string | null): DescriptionState {
  if (text === null) return "reading";
  return text === saved ? "editing" : "changed";
}

/**
 * `overview::leaving`: whether the move to `next` asks first. Only a changed description asks, and only when the move
 * leaves this ticket's Overview (`overview` lists the selection values Overview shows: `null` and `overview`).
 */
export function asksFirst(
  state: DescriptionState,
  ticket: string,
  next: Pick<Selection, "ticket" | "tab">,
  overview: readonly SheetContent[],
): boolean {
  if (state !== "changed") return false;
  const stays = next.ticket === ticket && (next.tab === null || overview.includes(next.tab));
  return !stays;
}

/** ⌘S on a Mac, Ctrl+S elsewhere (the bar's key). */
export function isSaveDescriptionKey(
  e: { key: string; metaKey: boolean; ctrlKey: boolean; shiftKey: boolean; altKey: boolean },
  mac: boolean,
): boolean {
  const command = mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
  return command && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "s";
}

/** The open editor, as the tab row's bar sees it: shown while the text differs from what is saved. */
export interface DescriptionDraft {
  ticket: string;
  bar: DescriptionSaveBar;
  changed: boolean;
  saving: boolean;
  save: () => void;
  discard: () => void;
}

export const descriptionDraft: Store<DescriptionDraft | null> = createStore<DescriptionDraft | null>(null);
