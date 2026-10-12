// Status Pill, Meta Chip and the count badge (*01 · Components → Primitives*, *Badges & Inputs → Badges & pills*).
import type { IconName } from "../generated/IconName";
import type { PillView } from "../generated/PillView";
import { Icon } from "../theme/Icon";

/** A status: 12 px icon + 11/500 label on the status background, never colour alone. Built from a `PillView`. */
export function StatusPill({ pill }: { pill: PillView }) {
  return (
    <span className="status-pill" data-palette={pill.palette}>
      <Icon name={pill.icon} size={12} />
      <span>{pill.label}</span>
    </span>
  );
}

/** How a Meta Chip is tinted: plain; green outline for a running process (`▶ :4020`); amber for a stale signal. */
export type ChipTone = "plain" | "running" | "warning";

export interface MetaChipProps {
  icon: IconName;
  label: string;
  tone?: ChipTone;
}

/** Key/value metadata in mono type with an 11 px icon: dependencies, branch, tokens, port, PR number. */
export function MetaChip({ icon, label, tone = "plain" }: MetaChipProps) {
  return (
    <span className="meta-chip" data-tone={tone}>
      <Icon name={icon} size={11} />
      <span>{label}</span>
    </span>
  );
}

/** Orange for items needing you, neutral, or accent. */
export type CountTone = "attention" | "neutral" | "accent";

/** A small rounded count (inbox, switcher, tab counts). */
export function CountBadge({ count, tone }: { count: number; tone: CountTone }) {
  return (
    <span className="count-badge" data-tone={tone}>
      {count}
    </span>
  );
}
