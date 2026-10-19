// The `g` hint (KEY-02; *02 → Row · The g hint*, `s02-29`…`s02-31`): while Otaq waits for a sequence's second key, a
// quiet line at the bottom centre lists the second keys from the user's keys ("g → b Board · i Inbox …"). It comes and
// goes with the `toast` motion (0095; reduced motion fades). The wait and its timer are the dispatcher's.
import { Fragment, useRef } from "react";

import "./sequencehint.css";
import { Kbd, useEnterExit } from "../components";
import type { SequenceHint as Hint } from "../generated/SequenceHint";
import { keymap, sequenceWait } from "./keyboard";
import { useStore } from "./store";

/** The hint's line (also what the gallery draws). */
export function SequenceHintLine({ hint }: { hint: readonly Hint[] }) {
  return (
    <div className="g-hint">
      <Kbd keys="g" />
      <span className="g-hint-muted">→</span>
      {hint.map((h, i) => (
        <Fragment key={h.then}>
          {i > 0 && <span className="g-hint-muted">·</span>}
          <Kbd keys={h.then} />
          <span>{h.name}</span>
        </Fragment>
      ))}
    </div>
  );
}

function Shown({ hint }: { hint: readonly Hint[] }) {
  const layer = useRef<HTMLDivElement>(null);
  useEnterExit(layer, "toast");
  return (
    <div ref={layer} className="g-hint-layer" role="status">
      <SequenceHintLine hint={hint} />
    </div>
  );
}

/** Shows the hint while Otaq waits after `g`. */
export function SequenceHint() {
  const waiting = useStore(sequenceWait);
  const map = useStore(keymap);
  if (!waiting || !map || map.sequences.length === 0) return null;
  return <Shown hint={map.sequences} />;
}
