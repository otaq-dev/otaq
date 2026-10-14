// How the cards turn the view model's unions into colours and states (T-0218). Every union the card draws has an
// exhaustive switch here (the linter's switch-exhaustiveness-check fails on a missing case), and each list of
// variants below is checked against its union, so a variant added in Rust can't go undrawn.
import type { CardBarLook } from "../generated/CardBarLook";
import type { CardLook } from "../generated/CardLook";
import type { CardOutline } from "../generated/CardOutline";
import type { CardTone } from "../generated/CardTone";
import type { EpicChildState } from "../generated/EpicChildState";

/** A tone's ink and its soft background, as CSS colours from the theme's variables. */
export interface Ink {
  fg: string;
  bg: string;
}

export function toneInk(tone: CardTone): Ink {
  switch (tone) {
    case "muted":
      return { fg: "var(--text-muted)", bg: "var(--bg-column)" };
    case "attention":
      return { fg: "var(--st-attn)", bg: "var(--st-attn-bg)" };
    case "error":
      return { fg: "var(--st-error)", bg: "var(--st-error-bg)" };
    case "warning":
      return { fg: "var(--st-paused)", bg: "var(--st-paused-bg)" };
    case "working":
      return { fg: "var(--st-working)", bg: "var(--st-working-bg)" };
    case "review":
      return { fg: "var(--st-review)", bg: "var(--st-review-bg)" };
    case "running":
      return { fg: "var(--st-approved)", bg: "var(--st-approved-bg)" };
    case "epic":
      return { fg: "var(--st-investigate)", bg: "var(--st-investigate-bg)" };
    case "accent":
      return { fg: "var(--accent)", bg: "var(--accent-soft)" };
    case "secondary":
      return { fg: "var(--text-secondary)", bg: "var(--bg-column)" };
  }
}

/** The card's border colour for its outline (turquoise: blocked on you; red: a closed parent blocks it). */
export function outlineColor(outline: CardOutline): string {
  switch (outline) {
    case "normal":
      return "var(--border)";
    case "attention":
      return "var(--st-attn)";
    case "error":
      return "var(--st-error)";
  }
}

/** A bar under the title: the accent "messages waiting" bar, or the calm grey one. */
export function barInk(look: CardBarLook): Ink {
  switch (look) {
    case "messages":
      return { fg: "var(--accent)", bg: "var(--accent-soft)" };
    case "quiet":
      return { fg: "var(--text-secondary)", bg: "var(--bg-column)" };
  }
}

/** An epic child's icon and status colour in the list. */
export function childInk(state: EpicChildState): string {
  switch (state) {
    case "merged":
    case "closed":
      return "var(--st-done)";
    case "working":
      return "var(--st-working)";
    case "not_started":
      return "var(--st-queued)";
  }
}

/** An epic child's segment in the progress bar. */
export function segmentFill(state: EpicChildState): string {
  switch (state) {
    case "merged":
      return "var(--st-approved)";
    case "working":
      return "var(--st-working)";
    case "closed":
    case "not_started":
      return "var(--border-strong)";
  }
}

/** How a card relates to the dependency chain being highlighted (DEP-05): the hovered or selected card itself
 * (`origin`, the frame's 2 px accent outline), another member, or a card outside the chain. */
export type ChainState = "none" | "origin" | "highlighted" | "dimmed";

/** The moment of interaction a card is drawn in; the board decides it, the gallery draws each on purpose. */
export interface CardState {
  selected: boolean;
  chain: ChainState;
  dragging: boolean;
  /** Draws the hovered look (and the quick actions) without the pointer. */
  hovered: boolean;
}

export const RESTING: CardState = { selected: false, chain: "none", dragging: false, hovered: false };

/** The gallery's variant look as a card state. */
export function stateOfLook(look: CardLook): CardState {
  switch (look) {
    case "plain":
      return RESTING;
    case "selected":
      return { ...RESTING, selected: true };
    case "hovered":
      return { ...RESTING, hovered: true };
    case "dragging":
      return { ...RESTING, dragging: true };
    case "chain":
      return { ...RESTING, chain: "highlighted" };
    case "dimmed":
      return { ...RESTING, chain: "dimmed" };
  }
}

/** Every variant of a union, checked both ways: no unknown entry, none missing. */
type Every<U extends string, L extends readonly U[]> = [Exclude<U, L[number]>] extends [never] ? L : never;

const every =
  <U extends string>() =>
  <L extends readonly U[]>(list: Every<U, L>) =>
    list;

export const TONES = every<CardTone>()([
  "muted",
  "attention",
  "error",
  "warning",
  "working",
  "review",
  "running",
  "epic",
  "accent",
  "secondary",
] as const);
export const OUTLINES = every<CardOutline>()(["normal", "attention", "error"] as const);
export const BAR_LOOKS = every<CardBarLook>()(["messages", "quiet"] as const);
export const CHILD_STATES = every<EpicChildState>()(["merged", "closed", "working", "not_started"] as const);
export const LOOKS = every<CardLook>()([
  "plain",
  "selected",
  "hovered",
  "dragging",
  "chain",
  "dimmed",
] as const);
