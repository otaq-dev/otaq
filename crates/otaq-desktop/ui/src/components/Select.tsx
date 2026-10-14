// The select (*Badges & Inputs → Fields → Dropdown*): a 30 px button with the current value and a chevron that opens
// its list, a menu of `MenuItem`s from Rust with the current one checked.
import { useRef, useState } from "react";

import type { IconName } from "../generated/IconName";
import type { MenuItem } from "../generated/MenuItem";
import { Icon } from "../theme/Icon";
import { AnchoredMenu } from "./Menu";

export interface SelectProps {
  id?: string;
  /** What is chosen, for screen readers when no Field label names it. */
  label?: string;
  /** The current value as shown on the button. */
  value: string;
  icon?: IconName | null;
  /** Code-like values in mono. */
  mono?: boolean;
  /** Another trailing icon than the chevron: chevrons-up-down in the editor, a lock on a locked base. */
  trailing?: IconName;
  disabled?: boolean;
  items: readonly MenuItem[];
  onChange: (id: string) => void;
}

export function Select({ id, label, value, icon, mono, trailing, disabled, items, onChange }: SelectProps) {
  const button = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  // The list is as wide as the button, measured when it opens.
  const [width, setWidth] = useState<number>();
  return (
    <>
      <button
        ref={button}
        id={id}
        type="button"
        className="select"
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={open}
        data-open={open || undefined}
        disabled={disabled}
        // While open, a press here must not blur the list first (it would close, then this click reopen it).
        onMouseDown={(e) => {
          if (open) e.preventDefault();
        }}
        onClick={() => {
          setWidth(button.current?.offsetWidth);
          setOpen((o) => !o);
        }}
      >
        <span className="select-value" data-mono={mono || undefined}>
          {icon !== undefined && icon !== null && <Icon name={icon} size={13} />}
          <span>{value}</span>
        </span>
        <Icon name={trailing ?? "chevron-down"} size={trailing ? 13 : 14} />
      </button>
      {open && (
        <AnchoredMenu
          anchor={button}
          label={label ?? value}
          list
          items={items}
          width={width}
          onChoose={onChange}
          onClose={(how) => {
            setOpen(false);
            if (how === "key") button.current?.focus();
          }}
        />
      )}
    </>
  );
}
