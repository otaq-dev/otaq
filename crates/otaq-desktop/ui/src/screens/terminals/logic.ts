// The drawer's moment of interaction (T-0242): which tab the user picked, a tab a shortcut asks to show, whether the
// drawer is maximized and whether a new terminal was typed into (its hint goes). The tabs, their order and states, the
// hint's lines and which tab ⇧⌘T shows are Rust's (`src/api/drawer.rs`, `project_terminal.rs`).
import { createStore, type Store } from "../../app/store";
import type { DrawerTab } from "../../generated/DrawerTab";

/** A tab to show and focus (⇧⌘T, the rail's Terminals, +); `seq` makes asking twice two requests. */
export interface TabRequest {
  tab: string;
  seq: number;
}

export const tabRequests: Store<TabRequest | null> = createStore<TabRequest | null>(null);

export function requestTab(tab: string, store: Store<TabRequest | null> = tabRequests): void {
  store.update((last) => ({ tab, seq: (last?.seq ?? 0) + 1 }));
}

/** The tab shown: the picked one while it is still open, else the one Rust names (last used, else first). */
export function shownTab(
  tabs: readonly DrawerTab[],
  picked: string | null,
  fallback: string | null,
): DrawerTab | undefined {
  return tabs.find((t) => t.id === picked) ?? tabs.find((t) => t.id === fallback) ?? tabs[0];
}

/** The hint in a new terminal's scrollback shows until its first keystroke (*17 → Empty · shell prompt*). */
export function showsHint(tab: DrawerTab, typed: ReadonlySet<string>, live: boolean): boolean {
  return tab.hint.length > 0 && !tab.ended && live && !typed.has(tab.id);
}
