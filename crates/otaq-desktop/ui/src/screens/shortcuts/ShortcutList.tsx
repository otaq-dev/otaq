// The ? shortcut list (KEY-07, d0097; *02 · Row · The ? shortcut list*, s02-24…s02-27): an overlay over the window,
// opened by `?` while nobody is typing or by "Show all shortcuts" in the palette. Esc, a click outside or the Settings
// link close it and focus goes back where it was; it never replaces the view. Its words, groups, rows, keys and
// marks are Rust's (`shortcuts.list`, `view_models/shortcut_list.rs`), asked again when the settings change.
import { useEffect, useId, useRef } from "react";

import { openOverlay } from "../../app/overlays";
import type { OverlayProps } from "../../app/registry";
import { Kbd, useDismiss, useEnterExit } from "../../components";
import type { ShortcutKey } from "../../generated/ShortcutKey";
import type { ShortcutListView } from "../../generated/ShortcutListView";
import type { ShortcutRow } from "../../generated/ShortcutRow";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { KEYBOARD_SECTION } from "./logic";

const loadList = () => request("shortcuts.list") as Promise<ShortcutListView>;
const LIST_EVENTS = ["settings_changed"] as const;

/** One key: its Kbds, a sequence's two joined by "then". */
function KeyKbds({ shortcut, then }: { shortcut: ShortcutKey; then: string }) {
  return (
    <>
      {shortcut.kbds.map((kbd, i) => (
        <span key={kbd} className="shortcuts-kbd">
          {i > 0 && <span className="shortcuts-word">{then}</span>}
          <Kbd keys={kbd} />
        </span>
      ))}
    </>
  );
}

function Row({ row, view }: { row: ShortcutRow; view: ShortcutListView }) {
  return (
    <div className="shortcuts-row" data-changed={row.changed || undefined} data-off={row.off || undefined}>
      <span className="shortcuts-title">{row.title}</span>
      {row.keys.map((shortcut, i) => (
        <span key={shortcut.kbds.join(" ")} className="shortcuts-kbd">
          {i > 0 && <span className="shortcuts-word">{view.or}</span>}
          <KeyKbds shortcut={shortcut} then={view.then} />
        </span>
      ))}
      {row.mark !== null && <span className="shortcuts-mark">{row.mark}</span>}
    </div>
  );
}

export interface ShortcutCardProps {
  view: ShortcutListView;
  /** The Settings link: opens Settings on Keyboard shortcuts. */
  onSettings: () => void;
  titleId?: string;
}

/** The list's box: head, note, and the groups in two columns (the gallery draws it on its own). */
export function ShortcutCard({ view, onSettings, titleId }: ShortcutCardProps) {
  return (
    <>
      <div className="shortcuts-head">
        <h2 id={titleId} className="shortcuts-heading">
          {view.title}
        </h2>
        <button type="button" className="shortcuts-link" onClick={onSettings}>
          {view.settings_link}
        </button>
        <Kbd keys="Esc" />
        <span className="shortcuts-close">{view.close}</span>
      </div>
      <p className="shortcuts-note">{view.note}</p>
      <div className="shortcuts-columns">
        {view.columns.map((groups) => (
          <div key={groups.map((g) => g.group).join()} className="shortcuts-column">
            {groups.map((group) => (
              <section key={group.group} className="shortcuts-group" aria-label={group.title}>
                <h3 className="shortcuts-group-title">{group.title}</h3>
                {group.rows.map((row) => (
                  <Row key={row.title} row={row} view={view} />
                ))}
              </section>
            ))}
          </div>
        ))}
      </div>
    </>
  );
}

const LIST_BOX = { selector: ".shortcuts", preset: "dialog" } as const;

/** The `shortcuts` overlay. */
export function ShortcutsOverlay({ close }: OverlayProps<"shortcuts">) {
  const { view } = useServiceView(loadList, LIST_EVENTS);
  const titleId = useId();
  const backdrop = useRef<HTMLDivElement>(null);
  const box = useRef<HTMLDivElement>(null);
  useEnterExit(backdrop, "backdrop", LIST_BOX);
  // Esc and a click outside close it.
  useDismiss(box, undefined, close);
  // Focus goes in, so Esc is the list's (the dispatcher leaves a dialog its Esc), and back where it was after.
  useEffect(() => {
    const before = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    box.current?.focus();
    return () => {
      before?.focus();
    };
  }, []);
  return (
    <div ref={backdrop} className="shortcuts-backdrop" role="presentation">
      <div
        ref={box}
        className="shortcuts"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
      >
        {view && (
          <ShortcutCard
            view={view}
            titleId={titleId}
            onSettings={() => {
              close();
              openOverlay("settings", { section: KEYBOARD_SECTION });
            }}
          />
        )}
      </div>
    </div>
  );
}
