// Button and Icon Button (*01 · Components → Buttons → Button matrix*): primary, secondary, destructive and ghost, in
// the states default, hover, pressed, focused (the 2 px focus ring), disabled and loading.
import type { Ref } from "react";

import type { IconName } from "../generated/IconName";
import { Icon } from "../theme/Icon";
import { acceptsPress } from "./logic";

export type ButtonVariant = "primary" | "secondary" | "destructive" | "ghost";

/** A state drawn on purpose, for the gallery's matrix; real buttons get it from the pointer and the keyboard. */
export type ShownState = "hover" | "pressed" | "focused";

export interface ButtonProps {
  label: string;
  variant?: ButtonVariant;
  icon?: IconName;
  /** 24 px high, as inside banners (28 px otherwise). */
  compact?: boolean;
  disabled?: boolean;
  /** Shows the loader in place of the icon and ignores presses; the caller sets the label ("Merging…"). */
  loading?: boolean;
  shown?: ShownState;
  onClick?: () => void;
  ref?: Ref<HTMLButtonElement>;
}

export function Button({
  label,
  variant = "primary",
  icon,
  compact = false,
  disabled = false,
  loading = false,
  shown,
  onClick,
  ref,
}: ButtonProps) {
  const name = loading ? "loader-circle" : icon;
  return (
    <button
      ref={ref}
      type="button"
      className="button"
      data-variant={variant}
      data-compact={compact || undefined}
      data-shown={shown}
      data-loading={loading || undefined}
      disabled={disabled}
      aria-busy={loading || undefined}
      onClick={() => {
        if (acceptsPress(disabled, loading)) onClick?.();
      }}
    >
      {name !== undefined && <Icon name={name} />}
      <span>{label}</span>
    </button>
  );
}

export interface IconButtonProps {
  icon: IconName;
  /** What the button does, for screen readers and as its tooltip text. */
  label: string;
  disabled?: boolean;
  loading?: boolean;
  shown?: ShownState;
  onClick?: () => void;
  ref?: Ref<HTMLButtonElement>;
}

/** A 28 px square button with a 15 px icon in the secondary text colour. */
export function IconButton({
  icon,
  label,
  disabled = false,
  loading = false,
  shown,
  onClick,
  ref,
}: IconButtonProps) {
  return (
    <button
      ref={ref}
      type="button"
      className="icon-button"
      aria-label={label}
      data-shown={shown}
      data-loading={loading || undefined}
      disabled={disabled}
      aria-busy={loading || undefined}
      onClick={() => {
        if (acceptsPress(disabled, loading)) onClick?.();
      }}
    >
      <Icon name={loading ? "loader-circle" : icon} size={15} />
    </button>
  );
}
