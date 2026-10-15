// The full-window view's plumbing (T-0260, d0078 D11), without a page. The view is a registered overlay
// (`full_view`): the overlay host draws an empty box over the whole window and hands its element to whoever opened
// it, the `owner`. The owner (the form, the Ticket tab) then draws the description into that box with a portal, from
// its own component: the same editor instance, the same form state and callbacks, so nothing is copied across.
import { createStore, type Store } from "../../app/store";

/** The box the overlay host drew, and which owner it is for. */
export interface FullViewSlot {
  owner: string;
  element: HTMLElement;
}

/** The open full view's box; `null` while none is drawn. */
export const fullViewSlot: Store<FullViewSlot | null> = createStore<FullViewSlot | null>(null);

/** The box `owner` draws into: the open view's, when `owner` opened it. */
export function slotFor(slot: FullViewSlot | null, owner: string): HTMLElement | null {
  return slot !== null && slot.owner === owner ? slot.element : null;
}
