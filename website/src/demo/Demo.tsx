// The live demo on the landing page (WEB-07, decision 0102): the canvas's Demo.dc.html drawn in React at 1280 × 800,
// Otaq Dark. The state and every change live in ./state.ts; this file draws them and keeps only the moment of
// interaction (the hovered and the dragged card) and where keyboard focus goes. Keys are handled only by the demo's
// own controls; there is no handler on the page.

import "./demo.css";

import { Fragment, useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";

import { demoFrame } from "../components/demoScale";
import { Board } from "./Board";
import { SPRITE } from "./icons";
import { Ic } from "./Ic";
import { Inbox, Settings } from "./Overlays";
import { Pane } from "./Pane";
import { Panel } from "./Panel";
import { DemoStore } from "./state";
import { boardView, type Moment, NO_MOMENT } from "./view";

interface FocusSpot {
  el: HTMLElement;
  /** `data-focus-key`: the same control drawn again elsewhere (a card that moved to another column). */
  key: string | null;
  /** `data-home`: the region it was in (the panel, the inbox, Settings, the pane, the board). */
  home: string | null;
}

const spotOf = (el: HTMLElement): FocusSpot => ({
  el,
  key: el.dataset.focusKey ?? null,
  home: el.closest<HTMLElement>("[data-home]")?.dataset.home ?? null,
});

// Below 900 px the page shows the demo as a view-only preview (WEB-05, components/demoScale.ts) and the pointer can't
// reach it; the demo is then inert too, so the keyboard and screen readers skip its controls, scaled far too small.
const subscribeWidth = (onChange: () => void) => {
  window.addEventListener("resize", onChange);
  return () => {
    window.removeEventListener("resize", onChange);
  };
};
const isViewOnly = () => demoFrame(document.documentElement.clientWidth || window.innerWidth).viewOnly;

export default function Demo() {
  const [store] = useState(() => new DemoStore());
  const s = useSyncExternalStore(store.subscribe, store.getState, store.getState);
  const [moment, setMoment] = useState<Moment>(NO_MOMENT);
  const viewOnly = useSyncExternalStore(subscribeWidth, isViewOnly, () => false);

  useEffect(() => {
    store.start();
    return () => {
      store.dispose();
    };
  }, [store]);

  // Focus never leaves the demo on its own (WEB-06): when the focused control goes away (an answer button after the
  // answer, Move to Dev once the ticket moved, a closed panel), focus moves to the same control drawn again, to the
  // region it was in, to what opened that region, or to the demo itself.
  const rootRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLElement>(null);
  const popRef = useRef<HTMLDivElement>(null);
  const dlgRef = useRef<HTMLDivElement>(null);
  const last = useRef<FocusSpot | null>(null);
  const openers = useRef<Record<string, FocusSpot | undefined>>({});

  const find = (spot: FocusSpot | undefined): HTMLElement | null => {
    const root = rootRef.current;
    if (!spot || !root) return null;
    if (spot.el.isConnected) return spot.el;
    if (spot.key) return root.querySelector<HTMLElement>(`[data-focus-key="${CSS.escape(spot.key)}"]`);
    return null;
  };

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    const onIn = (e: FocusEvent) => {
      if (e.target instanceof HTMLElement) last.current = spotOf(e.target);
    };
    const onOut = (e: FocusEvent) => {
      const to = e.relatedTarget;
      if (to instanceof Node) {
        if (!root.contains(to)) last.current = null;
        return;
      }
      // Focus went nowhere: the visitor clicked the page outside the demo, or the control is being removed.
      queueMicrotask(() => {
        if (last.current?.el.isConnected && !root.contains(document.activeElement)) last.current = null;
      });
    };
    root.addEventListener("focusin", onIn);
    root.addEventListener("focusout", onOut);
    return () => {
      root.removeEventListener("focusin", onIn);
      root.removeEventListener("focusout", onOut);
    };
  }, []);

  useLayoutEffect(() => {
    const root = rootRef.current;
    const l = last.current;
    if (!root || !l || l.el.isConnected) return;
    const active = document.activeElement;
    if (active && active !== document.body) return;
    const home = l.home ? root.querySelector<HTMLElement>(`[data-home="${l.home}"]`) : null;
    const target = find(l) ?? home ?? (l.home ? find(openers.current[l.home]) : null) ?? root;
    target.focus({ preventScroll: true });
  });

  // Opening the panel, the inbox or Settings from inside the demo moves focus into it.
  const shown = {
    panel: s.openKey,
    inbox: s.inboxOpen ? "open" : null,
    settings: s.settingsOpen ? "open" : null,
  };
  const prevShown = useRef<Record<string, string | null>>({ panel: null, inbox: null, settings: null });
  useLayoutEffect(() => {
    const root = rootRef.current;
    const refs = { panel: panelRef, inbox: popRef, settings: dlgRef };
    for (const name of ["panel", "inbox", "settings"] as const) {
      const now = shown[name];
      if (now !== null && now !== prevShown.current[name]) {
        const active = document.activeElement;
        if (root && active instanceof HTMLElement && root.contains(active)) {
          if (!refs[name].current?.contains(active)) openers.current[name] = spotOf(active);
          refs[name].current?.focus({ preventScroll: true });
        }
      }
      prevShown.current[name] = now;
    }
  });

  const v = boardView(s, moment);
  return (
    <div className="oq" ref={rootRef} tabIndex={-1} inert={viewOnly}>
      <svg
        width="0"
        height="0"
        style={{ position: "absolute" }}
        aria-hidden="true"
        dangerouslySetInnerHTML={{ __html: SPRITE }}
      ></svg>

      <header className="hd">
        <span className="lights" aria-hidden="true">
          <i></i>
          <i></i>
          <i></i>
        </span>
        <span className="pname">{v.proj.name}</span>
        <span className="chip">
          <Ic name="git-branch" cls="s12" />
          {v.proj.branch}
        </span>
        <div className="search">
          <label>
            <Ic name="search" />
            <input
              type="text"
              placeholder="Search tickets"
              aria-label="Search tickets"
              value={s.search}
              onChange={(e) => {
                store.setSearch(e.target.value);
              }}
            />
            <span className="kbd">⌘K</span>
          </label>
        </div>
        <button
          className="hbtn"
          type="button"
          onClick={() => {
            store.say(
              "Activity: " +
                String(v.running) +
                " agents running. Usage shows here only when a tool has 20% or less left.",
            );
          }}
        >
          <span className="dot" style={{ background: "var(--work)" }}></span>
          <span className="mono" style={{ fontSize: 12 }}>
            {v.runningLabel}
          </span>
        </button>
        <button
          className={"hbtn " + (v.needs ? "attn" : "out")}
          type="button"
          onClick={() => {
            store.toggleInbox();
          }}
          aria-expanded={s.inboxOpen}
        >
          <Ic name="inbox" />
          Needs you
          {v.needs > 0 && <span className="badge">{v.needs}</span>}
        </button>
        <button
          className="btn btn-p"
          type="button"
          onClick={() => {
            store.say(
              "In the app this opens the ticket editor: a title, a Markdown description, priority and dependencies.",
            );
          }}
        >
          <Ic name="plus" />
          New ticket
        </button>
        <button
          className="ibtn"
          type="button"
          onClick={() => {
            store.openSettings();
          }}
          aria-label="Settings"
        >
          <Ic name="settings" cls="s16" />
        </button>
      </header>

      <div className="body">
        <nav className="rail" aria-label="Projects">
          {v.projects.map((p) => (
            <Fragment key={p.key}>
              <button
                className={"proj" + (p.on ? " on" : "")}
                type="button"
                onClick={() => {
                  setMoment(NO_MOMENT);
                  store.pickProject(p.key);
                }}
                aria-current={p.on}
              >
                {p.key}
                {p.badge > 0 && <span className="badge pb">{p.badge}</span>}
              </button>
              <span className="proj-n">{p.name}</span>
            </Fragment>
          ))}
          <button
            className="ibtn"
            type="button"
            onClick={() => {
              store.say(
                "In the app you pick any folder that’s a git repository. Otaq suggests a key like SHOP.",
              );
            }}
            aria-label="Add project"
            style={{ border: "1px dashed var(--bds)" }}
          >
            <Ic name="plus" />
          </button>
          <span className="sp"></span>
          <button
            className="ibtn"
            type="button"
            onClick={() => {
              store.togglePane();
            }}
            aria-label="Show or hide the Project agent pane"
            data-focus-key="pane-toggle"
          >
            <Ic name="panel-left" cls="s16" />
          </button>
        </nav>

        <Pane v={v} store={store} hidden={s.paneHidden} />

        <section className="main" aria-label="Board" tabIndex={-1} data-home="board">
          <div className="mtabs">
            <button className="mtab on" type="button">
              <Ic name="layers" />
              Board
            </button>
            <span className="mhint">Hover a card to see what it waits for · drag cards between columns</span>
          </div>
          <Board v={v} store={store} moment={moment} setMoment={setMoment} />
          {v.panel && (
            <Panel
              t={v.panel}
              s={s}
              branch={v.proj.branch}
              projName={v.proj.name}
              store={store}
              panelRef={panelRef}
            />
          )}
        </section>
      </div>

      {s.inboxOpen && <Inbox inbox={v.inbox} store={store} popRef={popRef} />}
      {s.settingsOpen && <Settings s={s} store={store} dlgRef={dlgRef} />}
      {s.toast !== null && (
        <div className="toast" role="status">
          <Ic name="info" cls="s16" />
          <span>{s.toast}</span>
        </div>
      )}
    </div>
  );
}
