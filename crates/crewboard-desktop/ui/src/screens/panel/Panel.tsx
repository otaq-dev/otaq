// The ticket side panel (UI-04; *05 · Ticket side panel*): the header with key, title, priority, status, branch,
// base, last signal and tokens; previous / next, expand (⇧⌘E) and close (Esc); the action bar of the ticket's state
// with its "…" menu, or the answer options of an Asks-you question; the queued notice, the conflict strip and the
// Start interrupted callout; the registered sections (Stack, Messages); the tab strip with its indicators (⌘⌥1–6);
// and the registered tab's body. Everything it says comes from `panel.view`; it keeps only the dragged width, the
// expanded view, open menus and the picked answer.
import { type PointerEvent, type ReactNode, useCallback, useEffect, useRef, useState } from "react";

import { openOverlay } from "../../app/overlays";
import { useRegistries, useShortcut } from "../../app/registry";
import { PANEL_SECTIONS, PANEL_TOP, panelTabNumbered } from "../../app/registry/panel";
import {
  closeTicket,
  openTicket,
  requestTerminalFocus,
  requestTicketEdit,
  showTab,
  useSelection,
} from "../../app/selection";
import { createStore, useStore } from "../../app/store";
import { AnchoredMenu, Button, IconButton, PriorityLabel, StatusPill, Tabs, Tooltip } from "../../components";
import type { PanelAnswer } from "../../generated/PanelAnswer";
import type { PanelButton } from "../../generated/PanelButton";
import type { PanelButtonLook } from "../../generated/PanelButtonLook";
import type { PanelHint } from "../../generated/PanelHint";
import type { PanelLoad } from "../../generated/PanelLoad";
import type { PanelNotice } from "../../generated/PanelNotice";
import type { PanelSent } from "../../generated/PanelSent";
import type { PanelSizes } from "../../generated/PanelSizes";
import type { PanelStartProblem } from "../../generated/PanelStartProblem";
import type { PanelTab } from "../../generated/PanelTab";
import type { PanelView } from "../../generated/PanelView";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import {
  actionRoute,
  canSend,
  draggedWidth,
  iconTabs,
  initialPick,
  menuRoute,
  panelLayout,
  pickByKey,
  picksOther,
  shownWidth,
  tabIndicator,
} from "./logic";

/** The service events that change what the panel shows. */
export const PANEL_EVENTS = ["ticket_changed", "ticket_deleted", "session_state", "inbox_changed"] as const;

const ignore = () => undefined;

function copy(text: string) {
  navigator.clipboard.writeText(text).catch(ignore);
}

/** The unsent-text question before a send clears the terminal's typed line (SES-07, d0048). */
function askUnsent(sent: PanelSent, again: () => Promise<unknown>) {
  if (sent.outcome !== "ask") return;
  openOverlay("confirm", {
    title: sent.title,
    body: sent.body,
    confirmLabel: sent.clear,
    destructive: false,
    onConfirm: again,
  });
}

function NavButton({
  icon,
  hint,
  disabled = false,
  onClick,
}: {
  icon: Parameters<typeof IconButton>[0]["icon"];
  hint: PanelHint;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <Tooltip text={hint.text} keys={hint.keys ?? undefined}>
      <IconButton icon={icon} label={hint.text} disabled={disabled} onClick={onClick} />
    </Tooltip>
  );
}

function PriorityMenu({ view }: { view: PanelView }) {
  const anchor = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  return (
    <Tooltip text={view.priority_hint}>
      <button
        ref={anchor}
        type="button"
        className="panel-priority"
        data-open={open || undefined}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => {
          setOpen(!open);
        }}
      >
        <PriorityLabel priority={view.priority} />
        <Icon name="chevron-down" size={11} />
      </button>
      {open && (
        <AnchoredMenu
          anchor={anchor}
          label={view.priority_hint}
          items={view.priority_menu}
          width={180}
          onChoose={(item) => {
            setOpen(false);
            request("panel.priority", { key: view.key, item }).catch(ignore);
          }}
          onClose={() => {
            setOpen(false);
          }}
        />
      )}
    </Tooltip>
  );
}

function MoreMenu({ view, onChoose }: { view: PanelView; onChoose: (item: string) => void }) {
  const anchor = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  return (
    <>
      <IconButton
        ref={anchor}
        icon="ellipsis"
        label={view.more_hint}
        shown={open ? "pressed" : undefined}
        onClick={() => {
          setOpen(!open);
        }}
      />
      {open && (
        <AnchoredMenu
          anchor={anchor}
          align="end"
          label={view.more_hint}
          items={view.menu}
          width={280}
          onChoose={(item) => {
            setOpen(false);
            onChoose(item);
          }}
          onClose={() => {
            setOpen(false);
          }}
        />
      )}
    </>
  );
}

const LOOK_VARIANT = {
  primary: "primary",
  secondary: "secondary",
  attention: "primary",
  danger: "destructive",
} as const satisfies Record<PanelButtonLook, string>;

function ActionButton({
  button,
  busy,
  onPress,
}: {
  button: PanelButton;
  busy: boolean;
  onPress: (button: PanelButton) => void;
}) {
  const attention = button.look === "attention";
  return (
    <span className="panel-action" data-look={button.look}>
      <Button
        label={button.label}
        variant={LOOK_VARIANT[button.look]}
        icon={attention ? "corner-down-right" : (button.icon ?? undefined)}
        loading={busy}
        onClick={() => {
          onPress(button);
        }}
      />
    </span>
  );
}

function AnswerBox({ view, answer, more }: { view: PanelView; answer: PanelAnswer; more: ReactNode }) {
  const [pick, setPick] = useState(() => initialPick(answer.rows));
  const [other, setOther] = useState("");
  const [sending, setSending] = useState(false);
  const send = (clear = false): Promise<unknown> => {
    if (pick === null || !canSend(pick, other, answer.rows) || sending) return Promise.resolve();
    setSending(true);
    const params = { key: view.key, question: answer.question, pick, other, clear };
    return (request("panel.answer", params) as Promise<PanelSent>)
      .then((sent) => {
        askUnsent(sent, () => send(true));
      })
      .catch(ignore)
      .finally(() => {
        setSending(false);
      });
  };
  return (
    <>
      <div className="panel-answer">
        <div className="panel-answer-question">
          <Icon name="message-circle-question-mark" size={14} />
          <span>{answer.text}</span>
        </div>
        <div
          className="panel-answer-options"
          role="radiogroup"
          aria-label={answer.text}
          tabIndex={-1}
          onKeyDown={(e) => {
            const n = pickByKey(e.key, answer.rows);
            if (n !== null && !(e.target instanceof HTMLInputElement)) {
              e.preventDefault();
              setPick(n);
            } else if (e.key === "Enter" && !(e.target instanceof HTMLInputElement)) {
              e.preventDefault();
              void send();
            }
          }}
        >
          {answer.rows.map((row) => (
            <button
              key={row.number}
              type="button"
              role="radio"
              aria-checked={pick === row.number}
              className="panel-answer-option"
              data-picked={pick === row.number || undefined}
              data-other={row.other || undefined}
              onClick={() => {
                setPick(row.number);
              }}
            >
              <span className="panel-answer-radio" />
              <span className="panel-answer-label">{row.label}</span>
              {row.recommended && (
                <span className="panel-answer-rec">
                  <Icon name="star" size={10} />
                  {answer.recommended_label}
                </span>
              )}
              <span className="panel-answer-number">{row.number}</span>
            </button>
          ))}
          {picksOther(pick, answer.rows) && (
            <input
              className="field-input panel-answer-other"
              value={other}
              placeholder={answer.other_placeholder}
              aria-label={answer.other_placeholder}
              // The field appears because Other… was just picked: typing goes straight into it.
              // eslint-disable-next-line jsx-a11y/no-autofocus -- it opens on the user's own pick
              autoFocus
              onChange={(e) => {
                setOther(e.target.value);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  void send();
                }
              }}
            />
          )}
        </div>
      </div>
      <div className="panel-bar">
        <span className="panel-answer-hint">{answer.hint}</span>
        <Button
          label={answer.in_terminal}
          variant="secondary"
          icon="square-terminal"
          onClick={() => {
            showTab("terminal");
            requestTerminalFocus(view.key);
          }}
        />
        <Button
          label={answer.send}
          icon="send"
          disabled={!canSend(pick, other, answer.rows)}
          loading={sending}
          onClick={() => {
            void send();
          }}
        />
        {more}
      </div>
    </>
  );
}

function Notice({ view, notice }: { view: PanelView; notice: PanelNotice }) {
  return (
    <div className="panel-notice" data-closed={notice.parent_closed || undefined}>
      <Icon name={notice.parent_closed ? "circle-slash" : "hourglass"} size={13} />
      <div className="panel-notice-body">
        <p>{notice.text}</p>
        {notice.choices.length > 0 && (
          <div className="panel-notice-choices">
            {notice.choices.map((choice) => (
              <span key={choice.parent} className="panel-notice-pair">
                <Button
                  compact
                  variant="secondary"
                  icon="rotate-ccw"
                  label={choice.reopen}
                  onClick={() => {
                    request("panel.reopen_parent", { key: view.key, parent: choice.parent }).catch(ignore);
                  }}
                />
                <Button
                  compact
                  variant="secondary"
                  label={choice.remove}
                  onClick={() => {
                    openOverlay("confirm", {
                      title: choice.confirm_title,
                      body: choice.confirm_body,
                      confirmLabel: choice.confirm_label,
                      destructive: false,
                      onConfirm: () =>
                        request("panel.remove_dependency", { key: view.key, parent: choice.parent }),
                    });
                  }}
                />
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function StartProblem({ view, problem }: { view: PanelView; problem: PanelStartProblem }) {
  const [log, setLog] = useState<string | null>(null);
  const [shown, setShown] = useState(false);
  return (
    <div className="panel-start-problem">
      <div className="panel-start-callout">
        <div className="panel-start-heading">
          <Icon name="circle-alert" size={14} />
          <span>{problem.heading}</span>
        </div>
        <p className="panel-start-message">{problem.message}</p>
        {problem.has_log && (
          <button
            type="button"
            className="panel-link"
            onClick={() => {
              if (!shown && log === null) {
                (request("panel.setup_log", { key: view.key }) as Promise<string | null>).then(
                  setLog,
                  ignore,
                );
              }
              setShown(!shown);
            }}
          >
            {shown ? problem.hide_log : problem.show_log}
          </button>
        )}
      </div>
      {shown && log !== null && <pre className="panel-setup-log">{log}</pre>}
      <div className="panel-start-does">
        <span className="panel-start-does-heading">{problem.retry_heading}</span>
        <ul>
          {problem.retry_does.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      </div>
    </div>
  );
}

/** How far an arrow key moves the focused edge. */
const KEY_STEP = 16;

/** The left edge: drag to resize (480–960 px), double-click for the default width; ←/→ when focused. */
function ResizeEdge({ view, width }: { view: PanelView; width: number }) {
  const resizeTo = (pointerX: number) => {
    const next = draggedWidth(pointerX, document.documentElement.clientWidth, view.sizes);
    panelLayout.update((l) => ({ ...l, width: next }));
  };
  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    const edge = e.currentTarget;
    edge.setPointerCapture(e.pointerId);
    const move = (m: globalThis.PointerEvent) => {
      resizeTo(m.clientX);
    };
    const up = () => {
      edge.removeEventListener("pointermove", move);
      edge.removeEventListener("pointerup", up);
    };
    edge.addEventListener("pointermove", move);
    edge.addEventListener("pointerup", up);
  };
  /* eslint-disable jsx-a11y/no-noninteractive-element-interactions, jsx-a11y/no-noninteractive-tabindex --
     a focusable separator with a value is a widget in WAI-ARIA (a splitter); jsx-a11y treats every separator as static */
  return (
    <div
      className="panel-edge"
      role="separator"
      aria-orientation="vertical"
      aria-label={view.resize_hint}
      aria-valuenow={width}
      aria-valuemin={view.sizes.min_width}
      aria-valuemax={view.sizes.max_width}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onKeyDown={(e) => {
        const step = e.key === "ArrowLeft" ? KEY_STEP : e.key === "ArrowRight" ? -KEY_STEP : 0;
        if (step === 0) return;
        e.preventDefault();
        resizeTo(document.documentElement.clientWidth - width - step);
      }}
      onDoubleClick={() => {
        panelLayout.update((l) => ({ ...l, width: null }));
      }}
    />
  );
  /* eslint-enable jsx-a11y/no-noninteractive-element-interactions, jsx-a11y/no-noninteractive-tabindex -- the splitter ends here */
}

function useWindowWidth(): number {
  const [width, setWidth] = useState(() => document.documentElement.clientWidth);
  useEffect(() => {
    const onResize = () => {
      setWidth(document.documentElement.clientWidth);
    };
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("resize", onResize);
    };
  }, []);
  return width;
}

export interface PanelBodyProps {
  view: PanelView;
  tab: PanelTab;
  expanded: boolean;
  width: number;
}

/** The panel for a loaded view (also what the markup tests render). */
export function PanelBody({ view, tab, expanded, width }: PanelBodyProps) {
  const app = useRegistries();
  const [busy, setBusy] = useState<string | null>(null);
  const press = (button: PanelButton) => {
    const route = actionRoute(button.action);
    switch (route.kind) {
      case "service":
        setBusy(button.action);
        request("panel.action", { key: view.key, action: button.action })
          .catch(ignore)
          .finally(() => {
            setBusy(null);
          });
        return;
      case "terminal":
        showTab("terminal");
        requestTerminalFocus(view.key);
        return;
      case "diff":
        showTab("diff");
        return;
      case "edit":
        requestTicketEdit(view.key);
        return;
      case "parent":
        if (view.parent !== null) openTicket(view.parent);
        return;
      case "overlay":
        openOverlay(route.id, { ticket: view.key });
        return;
      case "mark_closed":
        if (view.mark_closed !== null) {
          openOverlay("confirm", {
            title: view.mark_closed.title,
            body: view.mark_closed.body,
            confirmLabel: view.mark_closed.confirm,
            destructive: true,
            onConfirm: () => request("panel.mark_closed", { key: view.key }),
          });
        }
        return;
      case "later":
        return;
    }
  };
  const choose = (item: string) => {
    const route = menuRoute(item);
    switch (route.kind) {
      case "edit":
        requestTicketEdit(view.key);
        return;
      case "service":
        request("panel.menu", { key: view.key, item }).catch(ignore);
        return;
      case "copy":
        copy(route.what === "branch" ? (view.branch?.full ?? "") : view.attach_command);
        return;
      case "overlay":
        openOverlay(route.id, { ticket: view.key });
        return;
      case "close_epic":
        openOverlay("close_epic", { epic: view.key });
        return;
      case "unknown":
        return;
    }
  };
  const [fixing, setFixing] = useState(false);
  const sendToFix = (clear = false): Promise<unknown> => {
    setFixing(true);
    return (request("panel.send_to_fix", { key: view.key, clear }) as Promise<PanelSent>)
      .then((sent) => {
        if (sent.outcome === "done") showTab("terminal");
        askUnsent(sent, () => sendToFix(true));
      })
      .catch(ignore)
      .finally(() => {
        setFixing(false);
      });
  };
  const active = view.tabs.findIndex((t) => t.tab === tab);
  const Body = app.panelTabs.get(tab)?.Component;
  const Top = app.panelSections.get(PANEL_TOP)?.Component;
  return (
    <div className="panel" data-expanded={expanded || undefined} style={expanded ? undefined : { width }}>
      {!expanded && <ResizeEdge view={view} width={width} />}
      {Top && <Top ticket={view.key} />}
      <div className="panel-header">
        <div className="panel-title-row">
          <span className="panel-key">{view.key}</span>
          <Tooltip text={view.title}>
            <h2 className="panel-title">{view.title}</h2>
          </Tooltip>
          <PriorityMenu view={view} />
          <NavButton
            icon="chevron-up"
            hint={view.previous_hint}
            disabled={view.previous === null}
            onClick={() => {
              if (view.previous !== null) openTicket(view.previous);
            }}
          />
          <NavButton
            icon="chevron-down"
            hint={view.next_hint}
            disabled={view.next === null}
            onClick={() => {
              if (view.next !== null) openTicket(view.next);
            }}
          />
          <NavButton
            icon={expanded ? "minimize-2" : "maximize-2"}
            hint={expanded ? view.collapse_hint : view.expand_hint}
            onClick={() => {
              panelLayout.update((l) => ({ ...l, expanded: !l.expanded }));
            }}
          />
          <NavButton
            icon="x"
            hint={view.close_hint}
            onClick={() => {
              closeTicket();
            }}
          />
        </div>
        <div className="panel-meta">
          <StatusPill pill={view.pill} />
          {view.branch && (
            <>
              <Tooltip text={view.branch.full}>
                <span className="panel-branch">
                  <Icon name="git-branch" size={11} />
                  <span>{view.branch.shown}</span>
                </span>
              </Tooltip>
              <Tooltip text={view.copy_branch}>
                <button
                  type="button"
                  className="panel-copy"
                  aria-label={view.copy_branch}
                  onClick={() => {
                    copy(view.branch?.full ?? "");
                  }}
                >
                  <Icon name="copy" size={12} />
                </button>
              </Tooltip>
            </>
          )}
          <span className="panel-muted">on</span>
          <span className="panel-base">{view.base}</span>
          <span className="panel-spacer" />
          {view.signal && (
            <span className="panel-meta-item" data-stale={view.signal.stale || undefined}>
              <Icon name="activity" size={11} />
              <span>{view.signal.text}</span>
            </span>
          )}
          {view.tokens &&
            (view.tokens.tooltip === null ? (
              <span className="panel-meta-item">
                <Icon name="coins" size={11} />
                <span>{view.tokens.text}</span>
              </span>
            ) : (
              <Tooltip text={view.tokens.tooltip}>
                <span className="panel-meta-item">
                  <Icon name="coins" size={11} />
                  <span>{view.tokens.text}</span>
                </span>
              </Tooltip>
            ))}
        </div>
        {view.conflict && (
          <div className="panel-conflict">
            <Icon name="triangle-alert" size={13} />
            <span className="panel-conflict-line">{view.conflict.line}</span>
            {view.conflict.fix !== null && (
              <Button
                compact
                variant="secondary"
                icon="undo-2"
                label={view.conflict.fix}
                loading={fixing}
                onClick={() => {
                  void sendToFix();
                }}
              />
            )}
          </div>
        )}
        {view.answer ? (
          <AnswerBox
            key={view.answer.question}
            view={view}
            answer={view.answer}
            more={<MoreMenu view={view} onChoose={choose} />}
          />
        ) : (
          <div className="panel-bar">
            {view.buttons.map((button) => (
              <ActionButton
                key={button.action}
                button={button}
                busy={busy === button.action}
                onPress={press}
              />
            ))}
            <span className="panel-spacer" />
            <MoreMenu view={view} onChoose={choose} />
          </div>
        )}
      </div>
      {view.notice && <Notice view={view} notice={view.notice} />}
      {PANEL_SECTIONS.map((id) => {
        const Section = app.panelSections.get(id)?.Component;
        return Section ? <Section key={id} ticket={view.key} /> : null;
      })}
      <div className="panel-tabs">
        <Tabs
          label={view.key}
          items={view.tabs.map((t) => ({
            icon: t.icon,
            label: t.label,
            indicator: tabIndicator(t.mark),
            keys: t.keys ?? undefined,
          }))}
          active={Math.max(active, 0)}
          iconsOnly={iconTabs(width, expanded, view.sizes)}
          onSelect={(i) => {
            const next = view.tabs[i];
            if (next) showTab(next.tab);
          }}
        />
      </div>
      {view.start_problem && <StartProblem key={view.key} view={view} problem={view.start_problem} />}
      <div className="panel-tab-body" data-tab={tab}>
        {Body && <Body ticket={view.key} />}
      </div>
    </div>
  );
}

/** The sizes of the last view, so the panel keeps its width while the next ticket loads. */
const lastSizes = createStore<PanelSizes | null>(null);

/** The panel slot's component: the open ticket's panel, with its keys while it shows. */
export function SidePanel() {
  const { ticket, tab } = useSelection();
  const layout = useStore(panelLayout);
  const windowWidth = useWindowWidth();
  const key = ticket ?? "";
  const load = useCallback(() => request("panel.view", { key }) as Promise<PanelLoad>, [key]);
  const { view: load_ } = useServiceView(load, PANEL_EVENTS);
  const view = load_?.view ?? null;
  const sizes = useStore(lastSizes);
  const current = useRef<PanelView | null>(null);
  useEffect(() => {
    current.current = view;
    if (view) lastSizes.set(view.sizes);
  }, [view]);

  useShortcut(
    "close_panel",
    useCallback(() => {
      closeTicket();
    }, []),
  );
  useShortcut(
    "expand_panel",
    useCallback(() => {
      panelLayout.update((l) => ({ ...l, expanded: !l.expanded }));
    }, []),
  );
  useShortcut(
    "previous_ticket",
    useCallback(() => {
      const previous = current.current?.previous;
      if (previous) openTicket(previous);
    }, []),
  );
  useShortcut(
    "next_ticket",
    useCallback(() => {
      const next = current.current?.next;
      if (next) openTicket(next);
    }, []),
  );
  useShortcut(
    "switch_tab",
    useCallback(({ number }: { number: number }) => {
      const next = panelTabNumbered(number);
      if (next === undefined) return false;
      showTab(next);
      return true;
    }, []),
  );

  if (ticket === null) return null;
  if (view === null || view.key !== ticket) {
    // Loading the next ticket, or gone: the panel keeps its width meanwhile.
    const style = sizes ? { width: shownWidth(layout, windowWidth, sizes) } : undefined;
    return (
      <div className="panel panel-gone" style={style}>
        {load_ && load_.view === null ? load_.gone : null}
      </div>
    );
  }
  return (
    <PanelBody
      view={view}
      tab={tab}
      expanded={layout.expanded}
      width={shownWidth(layout, windowWidth, view.sizes)}
    />
  );
}
