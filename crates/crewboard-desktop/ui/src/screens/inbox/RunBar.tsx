// The bar on top of the panel during a ⌘J run (*10 → 1 · ⌘J from anywhere, 2 · Answered, 3 · Next item*): "Inbox ·
// 1 of 7" with a segment per item, Next ⌘J and Exit; "Done · 6 left" once the open item is answered. It never moves
// on by itself (ATT-07). What it says comes from `inbox.run_bar`.
import { useEffect, useState } from "react";

import type { TicketProps } from "../../app/registry/panel";
import { useStore } from "../../app/store";
import { Button, Kbd } from "../../components";
import type { InboxRunBar } from "../../generated/InboxRunBar";
import { watchView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { runNext } from "./Inbox";
import { runChanges } from "./logic";

const RUN_EVENTS = ["inbox_changed", "ticket_changed"] as const;

/** The bar for a loaded run (also what the markup tests render). */
export function RunBarBody({ bar }: { bar: InboxRunBar }) {
  return (
    <div className="inbox-run" data-tone={bar.tone}>
      <Icon name={bar.tone === "step" ? "inbox" : "check"} size={13} />
      <span className="inbox-run-title">{bar.title}</span>
      <span className="inbox-run-segments">
        {bar.segments.map((segment, i) => (
          <span key={i} className="inbox-run-segment" data-segment={segment} />
        ))}
      </span>
      <span className="inbox-spacer" />
      {bar.tone === "step" ? (
        <Button
          compact
          variant="ghost"
          label={bar.next.label}
          onClick={() => {
            void runNext();
          }}
        />
      ) : (
        <span className="inbox-muted">{bar.next.label}</span>
      )}
      <Kbd keys={bar.next.keys} />
      {bar.exit !== null && (
        <Button
          compact
          variant="ghost"
          label={bar.exit}
          onClick={() => {
            request("inbox.exit_run")
              .catch(() => undefined)
              .finally(() => {
                runChanges.update((n) => n + 1);
              });
          }}
        />
      )}
    </div>
  );
}

/** The panel's top section: the run bar while the open ticket is the run's item. */
export function RunBar({ ticket }: TicketProps) {
  // Asked again on each change of the run (⌘J, Exit) as well as on the inbox's events.
  const changes = useStore(runChanges);
  const [bar, setBar] = useState<{ ticket: string; bar: InboxRunBar | null }>();
  useEffect(
    () =>
      watchView(
        () => request("inbox.run_bar", { key: ticket }) as Promise<InboxRunBar | null>,
        RUN_EVENTS,
        (state) => {
          if (state.view !== undefined) setBar({ ticket, bar: state.view });
        },
      ),
    [ticket, changes],
  );
  return bar?.ticket === ticket && bar.bar ? <RunBarBody bar={bar.bar} /> : null;
}
