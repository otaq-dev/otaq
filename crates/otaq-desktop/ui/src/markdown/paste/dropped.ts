// What a paste left out (T-0255, d0078 D6): images and embeds, counted so a muted line under the content can say how
// many until the next edit. Dropped colours, fonts, highlights, underline and alignment aren't counted (D6: "Dropped
// colours and fonts aren't reported").

/** The images and embeds (video, audio, frames, plugins) a paste left out. */
export interface Dropped {
  images: number;
  embeds: number;
}

export const NOTHING_DROPPED: Dropped = { images: 0, embeds: 0 };

export function droppedAny({ images, embeds }: Dropped): boolean {
  return images > 0 || embeds > 0;
}

/**
 * The note's wording (*crewboard.pen → 08 · Ticket editor → 6 · Copy and paste*). It lives here, not in Rust's editor
 * texts (SDD §5.5), because no Rust text reaches the shared editor component; the screens' texts don't pass into it.
 */
const NOTE_REASON = "descriptions are Markdown only";

function counted(n: number, one: string, many: string): string {
  return `${String(n)} ${n === 1 ? one : many}`;
}

/** "1 image left out: descriptions are Markdown only", "2 images and 1 embed left out: …"; `null` for nothing. */
export function droppedNote(dropped: Dropped): string | null {
  const parts = [
    ...(dropped.images > 0 ? [counted(dropped.images, "image", "images")] : []),
    ...(dropped.embeds > 0 ? [counted(dropped.embeds, "embed", "embeds")] : []),
  ];
  return parts.length === 0 ? null : `${parts.join(" and ")} left out: ${NOTE_REASON}`;
}
