// "All clear" over the board after the last ⌘J item (*10 → 4 · All clear*): a calm moment, then it goes and the
// board has the keyboard again.
import { useEffect } from "react";

import { closeOverlay } from "../../app/overlays";
import type { OverlayProps } from "../../app/registry";
import type { InboxAllClear } from "../../generated/InboxAllClear";
import { Icon } from "../../theme/Icon";

export function AllClearBody({ clear }: { clear: InboxAllClear }) {
  return (
    <div className="inbox-all-clear" role="status">
      <div className="inbox-all-clear-card">
        <span className="inbox-all-clear-icon">
          <Icon name="check-check" size={24} />
        </span>
        <strong>{clear.title}</strong>
        <span>{clear.text}</span>
        <span className="inbox-all-clear-segments">
          {Array.from({ length: clear.segments }, (_, i) => (
            <span key={i} />
          ))}
        </span>
      </div>
    </div>
  );
}

/** The `all_clear` overlay: shown for its time, then closed. */
export function AllClear({ params }: OverlayProps<"all_clear">) {
  useEffect(() => {
    const timer = window.setTimeout(() => {
      closeOverlay("all_clear");
    }, params.shown_ms);
    return () => {
      window.clearTimeout(timer);
    };
  }, [params]);
  return <AllClearBody clear={params} />;
}
