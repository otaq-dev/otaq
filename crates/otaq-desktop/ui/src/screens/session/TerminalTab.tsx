// Session's terminal (SHT-04; SES-09, SES-10, SES-11, SES-06, SES-07, SES-08, ATT-03, REC-02; canvas `s25-06`): the line
// over it ("Rust developer · Codex · gpt-5.5 · ~/.otaq/wt/shop-web/SHOP-22", "Typing goes to Rust developer") after
// Session's switch, the strip (state, `$PORT`, "1 message waiting · Deliver now" or the attach command, A− / A+,
// find), the ticket's agent session (or its check's, `side`) in T-0216's terminal, the hint line under it with
// "Instructions: shop-web" or "leave terminal", the end marker of an ended session, and "Before the restart" for an
// Interrupted ticket whose session is gone. The check's side shows the running check (typing goes to it) or the last
// one, read-only. Everything it says comes from `terminal_tab.view`; it keeps only the terminal's state and its width.
import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";

import { openOverlay } from "../../app/overlays";
import { terminalFocusRequests } from "../../app/selection";
import { useStore } from "../../app/store";
import { Button, IconButton, Kbd, Tooltip } from "../../components";
import type { PanelSent } from "../../generated/PanelSent";
import type { TerminalTabLoad } from "../../generated/TerminalTabLoad";
import type { TerminalTabRecovery } from "../../generated/TerminalTabRecovery";
import type { TerminalTabView } from "../../generated/TerminalTabView";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Terminal, type TerminalHandle } from "../../terminal/Terminal";
import { INITIAL_STATE, type TerminalState } from "../../terminal/state";
import { Icon } from "../../theme/Icon";
import {
  answers,
  bodyOf,
  isNarrow,
  scrolledLabel,
  sessionOf,
  type Side,
  stripFor,
  toneColor,
} from "./terminalLogic";

/** The service events that change what the tab shows. */
export const TERMINAL_TAB_EVENTS = [
  "ticket_changed",
  "ticket_deleted",
  "session_state",
  "projects_changed",
  "inbox_changed",
  // The user's keys in its tooltips (KEY-08).
  "settings_changed",
] as const;

const ignore = () => undefined;

/** What the shown terminal last said, and for which session. */
interface Seen {
  session: string | null;
  state: TerminalState;
}

/** The newest focus request answered, across mounts: a request is answered once, by whichever tab shows it. */
let answered = 0;

/** How long a focus request waits for the terminal to take the keyboard. */
const FOCUS_WAIT_MS = 5000;

function useWidth(element: React.RefObject<HTMLElement | null>): number {
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const el = element.current;
    if (!el) return;
    const watch = new ResizeObserver(() => {
      setWidth(el.clientWidth);
    });
    // The observer reports the first size too.
    watch.observe(el);
    return () => {
      watch.disconnect();
    };
  }, [element]);
  return width;
}

/** Deliver now; with unsent text in the terminal the user is asked first (SES-07, d0048). */
function deliverNow(key: string, clear = false): Promise<unknown> {
  return (request("terminal_tab.deliver_now", { key, clear }) as Promise<PanelSent>).then((sent) => {
    if (sent.outcome !== "ask") return;
    openOverlay("confirm", {
      title: sent.title,
      body: sent.body,
      confirmLabel: sent.clear,
      cancelLabel: sent.cancel,
      destructive: false,
      onConfirm: () => deliverNow(key, true),
    });
  }, ignore);
}

export interface TerminalTabBodyProps {
  view: TerminalTabView;
  /** Session's side shown: the agent's terminal or the check's. */
  side?: Side;
  /** What leads the line: Session's switch. */
  head?: ReactNode;
  /** The terminal's state (from its `onState`); the markup tests pass one. */
  state: TerminalState;
  narrow: boolean;
  terminal?: React.RefObject<TerminalHandle | null>;
  onState?: (state: TerminalState) => void;
}

/** The tab for a loaded view (also what the markup tests render). */
export function TerminalTabBody({
  view,
  side = "agent",
  head,
  state,
  narrow,
  terminal,
  onState,
}: TerminalTabBodyProps) {
  const validator = side === "check" ? view.check : null;
  const body = bodyOf(view, side);
  const session = sessionOf(view, side);
  const showTerminal = body === "terminal" && session !== null;
  const strip = stripFor(view, state.phase, showTerminal && state.focused, side);
  const attachCommand = validator?.attach_command ?? view.attach_command;
  const line = validator ? validator.line : view.line;
  const typing = validator ? validator.typing : view.typing;
  const [delivering, setDelivering] = useState(false);
  const scrolled =
    showTerminal && state.scrolledUp > 0 && (state.phase === "live" || state.phase === "ended");
  return (
    <div className="terminal-tab" data-focused={(showTerminal && state.focused) || undefined}>
      <div className="session-line">
        {head}
        {line !== null && <span className="session-line-text">{line}</span>}
        <span className="terminal-tab-spacer" />
        {typing !== null && <span className="session-typing">{typing}</span>}
      </div>
      <div className="terminal-tab-strip">
        {strip && (
          <Tooltip text={view.background ?? strip.state.label}>
            <span className="terminal-tab-state" style={{ color: toneColor(strip.state.tone) }}>
              <span className="terminal-tab-dot" />
              {strip.state.label}
            </span>
          </Tooltip>
        )}
        {!narrow && !validator && view.port !== null && (
          <span className="terminal-tab-mono">{view.port}</span>
        )}
        <span className="terminal-tab-spacer" />
        {view.waiting && !validator ? (
          <button
            type="button"
            className="terminal-tab-deliver"
            disabled={delivering}
            onClick={() => {
              setDelivering(true);
              void deliverNow(view.key).finally(() => {
                setDelivering(false);
              });
            }}
          >
            <Icon name="message-square-dot" size={11} />
            <span>{view.waiting.label}</span>
            <strong>{view.waiting.deliver}</strong>
          </button>
        ) : (
          <Tooltip text={view.copy_attach}>
            <button
              type="button"
              className="terminal-tab-attach"
              aria-label={view.copy_attach}
              onClick={() => {
                navigator.clipboard.writeText(attachCommand).catch(ignore);
              }}
            >
              <Icon name="copy" size={11} />
              <span className="terminal-tab-mono">{narrow ? view.attach_short : attachCommand}</span>
            </button>
          </Tooltip>
        )}
        {showTerminal && (
          <>
            <Tooltip text={view.smaller}>
              <IconButton
                icon="a-arrow-down"
                label={view.smaller}
                onClick={() => terminal?.current?.changeFontSize(-1)}
              />
            </Tooltip>
            <Tooltip text={view.bigger}>
              <IconButton
                icon="a-arrow-up"
                label={view.bigger}
                onClick={() => terminal?.current?.changeFontSize(1)}
              />
            </Tooltip>
            <Tooltip text={view.find.text} keys={view.find.keys ?? undefined}>
              <IconButton
                icon="search"
                label={view.find.text}
                onClick={() => terminal?.current?.openFind()}
              />
            </Tooltip>
          </>
        )}
      </div>
      <div className="terminal-tab-well">
        {body === "recovery" && view.recovery && <RecoveryBody view={view} recovery={view.recovery} />}
        {body === "no_session" && <div className="terminal-tab-empty">{view.no_session}</div>}
        {showTerminal && (
          <div className="terminal-tab-terminal">
            <Terminal
              key={session}
              session={session}
              label={validator?.terminal_label ?? view.label}
              readOnly={validator?.read_only}
              onState={onState}
              files={{ kind: "ticket", key: view.key }}
              ref={terminal}
            />
            {!validator && state.phase === "ended" && view.ended && (
              <div className="terminal-tab-ended">
                <Icon name="circle-check" size={15} />
                <div>
                  <strong>{view.ended.title}</strong>
                  <span>{view.ended.text}</span>
                </div>
              </div>
            )}
          </div>
        )}
        <div className="terminal-tab-hint">
          {scrolled ? (
            <>
              <Icon name="arrow-up" size={13} />
              <span>{scrolledLabel(state.scrolledUp)}</span>
            </>
          ) : (
            strip?.hint && (
              <span className="terminal-tab-hint-line" data-tone={strip.hint.tone}>
                <Icon name={strip.hint.icon} size={13} />
                <span>{strip.hint.text}</span>
              </span>
            )
          )}
          <span className="terminal-tab-spacer" />
          {showTerminal && state.focused ? (
            <span className="terminal-tab-leave">
              {view.leave.keys !== null && <Kbd keys={view.leave.keys} />}
              <span>{view.leave.text}</span>
            </span>
          ) : (
            body === "terminal" &&
            !validator &&
            state.phase === "live" && (
              <button
                type="button"
                className="terminal-tab-instructions"
                onClick={() => {
                  openOverlay("settings", {
                    project: view.instructions.project,
                    section: "project_notes",
                  });
                }}
              >
                <Icon name="info" size={12} />
                <span>{view.instructions.label}</span>
              </button>
            )
          )}
        </div>
      </div>
    </div>
  );
}

/** "Before the restart"'s texts, as Rust words them (a ticket's Terminal tab, an epic's Lead tab). */
export interface BeforeRestartTexts {
  title: string;
  source: string;
  session: string;
  lines: { time: string; who: string; text: string; action: boolean }[];
  ended: string;
  resume: string;
}

/**
 * "Before the restart" (REC-02, d0045): the last messages and actions from Claude's saved conversation, read-only,
 * and the ended line with Resume. The epic's Lead tab shows its lead's the same way (T-0290).
 */
export function BeforeRestart({
  before,
  busy,
  onResume,
}: {
  before: BeforeRestartTexts;
  busy: boolean;
  onResume: () => void;
}) {
  return (
    <div className="terminal-tab-recovery">
      <div className="terminal-tab-recovery-head">
        <Icon name="history" size={13} />
        <strong>{before.title}</strong>
        <span className="terminal-tab-muted">{before.source}</span>
        <span className="terminal-tab-spacer" />
        <span className="terminal-tab-mono">{before.session}</span>
      </div>
      <div className="terminal-tab-recovery-lines">
        {before.lines.map((line, i) => (
          <div key={i} className="terminal-tab-recovery-line" data-action={line.action || undefined}>
            <span className="terminal-tab-recovery-time">{line.time}</span>
            <span className="terminal-tab-recovery-who">{line.who}</span>
            <span className="terminal-tab-recovery-text">{line.text}</span>
          </div>
        ))}
      </div>
      <div className="terminal-tab-recovery-ended">
        <Icon name="zap-off" size={15} />
        <span>{before.ended}</span>
        <Button compact icon="circle-play" label={before.resume} loading={busy} onClick={onResume} />
      </div>
    </div>
  );
}

function RecoveryBody({ view, recovery }: { view: TerminalTabView; recovery: TerminalTabRecovery }) {
  const [busy, setBusy] = useState(false);
  const act = (action: "resume" | "retry_resume") => {
    setBusy(true);
    request("panel.action", { key: view.key, action })
      .catch(ignore)
      .finally(() => {
        setBusy(false);
      });
  };
  switch (recovery.kind) {
    case "loading":
      return <div className="terminal-tab-empty">{recovery.text}</div>;
    case "before_restart":
      return (
        <BeforeRestart
          before={recovery}
          busy={busy}
          onResume={() => {
            act("resume");
          }}
        />
      );
    case "not_available":
      return (
        <div className="terminal-tab-recovery">
          <div className="terminal-tab-recovery-gone">
            <span className="terminal-tab-muted">{recovery.label}</span>
            <p>{recovery.text}</p>
            <div className="terminal-tab-recovery-buttons">
              <Button
                compact
                variant="secondary"
                icon="rotate-ccw"
                label={recovery.retry}
                loading={busy}
                onClick={() => {
                  act("retry_resume");
                }}
              />
              <Button
                compact
                variant="secondary"
                icon="square-plus"
                label={recovery.scratch}
                onClick={() => {
                  openOverlay("start_from_scratch", { ticket: view.key });
                }}
              />
            </div>
          </div>
        </div>
      );
  }
}

/**
 * Session's Agent or check side: the open ticket's view, its terminal's state, and focus requests (T-0231) for the
 * ticket, answered by the side the selection shows.
 */
export function TerminalTab({ ticket, side, head }: { ticket: string; side: Side; head?: ReactNode }) {
  const load = useCallback(
    () => request("terminal_tab.view", { key: ticket }) as Promise<TerminalTabLoad>,
    [ticket],
  );
  const { view: loaded } = useServiceView(load, TERMINAL_TAB_EVENTS);
  const view = loaded?.view ?? null;
  const root = useRef<HTMLDivElement>(null);
  const width = useWidth(root);
  const terminal = useRef<TerminalHandle | null>(null);
  const focusRequest = useStore(terminalFocusRequests);
  const session = view === null ? null : sessionOf(view, side);

  // The shown terminal's state; another side mounts another terminal, whose state starts over.
  const [seen, setSeen] = useState<Seen>({ session: null, state: INITIAL_STATE });
  const state = seen.session === session ? seen.state : INITIAL_STATE;
  const latest = useRef<Seen>(seen);
  const onState = useCallback(
    (next: TerminalState) => {
      latest.current = { session, state: next };
      setSeen(latest.current);
    },
    [session],
  );
  const shown = useRef(session);
  useEffect(() => {
    shown.current = session;
  }, [session]);

  // A notification click or the action bar asked for this ticket's terminal: focus it once it takes the keyboard
  // (the session may still be attaching, or xterm.js still opening), for a few seconds at most.
  useEffect(() => {
    if (focusRequest === null || !answers(focusRequest, ticket, answered)) return;
    answered = focusRequest.seq;
    const deadline = Date.now() + FOCUS_WAIT_MS;
    let timer: number | undefined;
    const attempt = () => {
      if (Date.now() > deadline) return;
      const { session: of, state: now } = latest.current;
      if (of === shown.current && of !== null) {
        if (now.focused || now.phase === "ended" || now.phase === "no_session") return;
        if (now.phase === "live") terminal.current?.focus();
      }
      timer = window.setTimeout(attempt, 100);
    };
    attempt();
    return () => {
      window.clearTimeout(timer);
    };
  }, [focusRequest, ticket]);

  return (
    <div className="terminal-tab-root" ref={root}>
      {view === null || view.key !== ticket ? (
        <>
          <div className="session-line">{head}</div>
          <div className="terminal-tab-empty">{loaded && loaded.view === null ? loaded.gone : null}</div>
        </>
      ) : (
        <TerminalTabBody
          view={view}
          side={side}
          head={head}
          state={state}
          narrow={isNarrow(width, view)}
          terminal={terminal}
          onState={onState}
        />
      )}
    </div>
  );
}
