// Show the N tickets (FLW-06; *11 → A flow change waiting*): the tickets a waiting flow change waits for, outlined in
// amber on the board, the other cards dimmed. Which tickets comes from Rust (the banner's `tickets`); whether they are
// shown is the page's moment of interaction: the button turns it on and off, Esc, another project, and the change
// applying or being cancelled turn it off. Like the epic strip's hover it marks nothing in the board's components: one
// style rule names the cards by key, so cards that redraw keep it.
import { createStore, type Store } from "../../app/store";
import type { Banner } from "../../generated/Banner";

/** Whether Show the N tickets is on. */
export const flowShown: Store<boolean> = createStore(false);

export function toggleFlowShown(store: Store<boolean> = flowShown): void {
  store.update((on) => !on);
}

/** The flow change banner, if one shows. */
export function flowBanner(banners: readonly Banner[]): Banner | undefined {
  return banners.find((b) => b.kind === "flow_waiting");
}

/** The style that outlines `keys`' cards in amber and dims every other card on the board; empty with no keys. */
export function highlightRules(
  keys: readonly string[],
  escape: (key: string) => string = CSS.escape,
): string {
  if (keys.length === 0) return "";
  const cards = keys.map((key) => `.board-card[data-card="${escape(key)}"]`).join(", ");
  return [
    `.board-card[data-card]:not(${cards}) { opacity: 0.3; transition: opacity var(--motion-fast) var(--ease-in-out); }`,
    `${cards} { box-shadow: 0 0 0 2px var(--st-paused); }`,
  ].join("\n");
}
