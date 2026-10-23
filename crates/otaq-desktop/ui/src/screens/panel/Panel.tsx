// The ticket's actions and what its sheet's Overview shows until T-0465 (UI-04; *05 · Ticket side panel*, now the
// sheet of SHT-01, T-0464): the action bar of the ticket's state, or the answer options of an Asks-you question; the
// held roles' callouts, the refusal, the queued notice, the conflict strip and the Start interrupted callout; the
// choices and the path; the registered sections (Epic, Stack, Messages). The sheet (`screens/sheet`) draws the header
// and the tabs and holds the actions (`usePanelActions`): its "…" menu (`MoreMenu`) is this file's. Everything it
// says comes from `panel.view`; it keeps only open menus and the picked answer. A board card's button lands here too
// (`cardActions`): the sheet runs it once it shows that ticket. The M12 rows (d0063, d0064): the validation buttons,
// the token split, the Auto-merge stop message and Answer the agent's note box. An epic (T-0290; d0083) keeps its
// Messages with its Ticket content.
import {
  type ReactNode,
  useCallback,
  useEffect,
  useEffectEvent,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

import { cardActions, takeCardAction } from "../../app/cardActions";
import { openOverlay } from "../../app/overlays";
import { useRegistries, useShortcut } from "../../app/registry";
import { PANEL_SECTIONS, PANEL_UNDER_TABS } from "../../app/registry/panel";
import {
  openTicket,
  requestTerminalFocus,
  requestTicketEdit,
  showTab,
  terminalFocusRequests,
} from "../../app/selection";
import { useStore } from "../../app/store";
import {
  AnchoredMenu,
  Button,
  IconButton,
  StageChoices,
  TextArea,
  TicketPath,
  ToolProblemCallout,
  Tooltip,
} from "../../components";
import type { PanelAnswer } from "../../generated/PanelAnswer";
import type { PanelAnswerAgent } from "../../generated/PanelAnswerAgent";
import type { PanelButton } from "../../generated/PanelButton";
import type { PanelButtonLook } from "../../generated/PanelButtonLook";
import type { PanelCardParams } from "../../generated/PanelCardParams";
import type { PanelChoiceParams } from "../../generated/PanelChoiceParams";
import type { PanelNotice } from "../../generated/PanelNotice";
import type { PanelSent } from "../../generated/PanelSent";
import type { PanelRefusal } from "../../generated/PanelRefusal";
import type { PanelStartProblem } from "../../generated/PanelStartProblem";
import type { PanelRunPill } from "../../generated/PanelRunPill";
import type { PanelView } from "../../generated/PanelView";
import { asRequestError } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { passTicket } from "../review/CheckDialogs";
import { startRun } from "../run/logic";
import {
  actionRoute,
  agentSettings,
  canSend,
  cardRoute,
  CLOSE_TICKET,
  initialPick,
  menuRoute,
  pickByKey,
  picksOther,
} from "./logic";
import { toolProblemAction } from "./toolProblemRunner";

/** The service events that change what the panel shows. */
export const PANEL_EVENTS = [
  "ticket_changed",
  "ticket_deleted",
  "session_state",
  "inbox_changed",
  "usage_changed",
  "validation_cancelled",
  "lead_changed",
  "project_agent_changed",
  // The user's keys in its tooltips (KEY-08).
  "settings_changed",
] as const;

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
    cancelLabel: sent.cancel,
    destructive: false,
    onConfirm: again,
  });
}

/** The "…" menu (the sheet's header): the ticket's menu of its state, Edit ticket… first. */
export function MoreMenu({ view, onChoose }: { view: PanelView; onChoose: (item: string) => void }) {
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
  const pressable = (
    <Button
      label={button.label}
      variant={LOOK_VARIANT[button.look]}
      icon={attention ? "corner-down-right" : (button.icon ?? undefined)}
      loading={busy}
      onClick={() => {
        onPress(button);
      }}
    />
  );
  return (
    <span className="panel-action" data-look={button.look}>
      {button.tooltip === null ? pressable : <Tooltip text={button.tooltip}>{pressable}</Tooltip>}
    </span>
  );
}

/** A card's answer button handed to the answer list of `question`: an option to pick, or Send answer. */
type CardAnswer = { question: string } & ({ kind: "pick"; number: number } | { kind: "send_answer" });

/** The question bar with its options (ATT-12, ATT-17); the Gallery draws it too. */
export function AnswerBox({
  view,
  answer,
  more,
  fromCard,
}: {
  view: PanelView;
  answer: PanelAnswer;
  more: ReactNode;
  fromCard: CardAnswer | null;
}) {
  const [pick, setPick] = useState(() => initialPick(answer.rows));
  const options = useRef<HTMLDivElement>(null);
  const noteId = useId();
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
  // A card's option opens the list with it picked (sending stays one more click or ↵, ATT-12); its Send answer
  // sends the picked (recommended) option.
  const onCard = useEffectEvent((card: CardAnswer) => {
    if (card.question !== answer.question) return;
    if (card.kind === "send_answer") {
      void send();
    } else if (answer.rows.some((r) => r.number === card.number)) {
      setPick(card.number);
      options.current?.focus();
    }
  });
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- the card's click arrives from outside React, once
    if (fromCard !== null) onCard(fromCard);
  }, [fromCard]);
  return (
    <>
      <div className="panel-answer">
        <div className="panel-answer-question">
          <Icon name="message-circle-question-mark" size={14} />
          <span>{answer.text}</span>
        </div>
        <div
          ref={options}
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
              aria-describedby={row.note === null ? undefined : `${noteId}-${String(row.number)}`}
              className="panel-answer-option"
              data-picked={pick === row.number || undefined}
              data-other={row.other || undefined}
              onClick={() => {
                setPick(row.number);
              }}
            >
              <span className="panel-answer-radio" />
              <span className="panel-answer-label">
                {row.label}
                {/* The option's note (ATT-17), whole and wrapped: its description. */}
                {row.note !== null && (
                  <span id={`${noteId}-${String(row.number)}`} className="panel-answer-note" aria-hidden>
                    {row.note}
                  </span>
                )}
              </span>
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
            // An epic's question is its lead's: its terminal is in the Lead tab.
            showTab(view.epic ? "lead" : "terminal");
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

/** *SHOP-32 · Validation failed 3 times*: the note goes with the last findings (`panel.answer_agent`). */
function AnswerAgentBox({
  view,
  box,
  onClose,
}: {
  view: PanelView;
  box: PanelAnswerAgent;
  onClose: () => void;
}) {
  const [note, setNote] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const send = () => {
    if (sending || note.trim() === "") return;
    setSending(true);
    setError(null);
    request("panel.answer_agent", { key: view.key, note }).then(onClose, (e: unknown) => {
      setError(asRequestError(e).message);
      setSending(false);
    });
  };
  return (
    <div className="panel-answer-agent">
      <span className="panel-answer-agent-heading">{box.heading}</span>
      <TextArea id="panel-answer-agent-note" value={note} placeholder={box.placeholder} onChange={setNote} />
      <p className="panel-answer-agent-footer">{box.footer}</p>
      {error !== null && <p className="panel-dialog-error">{error}</p>}
      <div className="panel-answer-agent-buttons">
        <Button variant="ghost" icon="check" label={box.cancel} onClick={onClose} />
        <Button icon="send" label={box.send} disabled={note.trim() === ""} loading={sending} onClick={send} />
      </div>
    </div>
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

/** A refused model above the bar (AGT-05, *05 → Start failed on the model*, *No result: the validator's model*): what
 * couldn't run (red for Start failed) and the tool's message in quotes. */
function Refusal({ refusal }: { refusal: PanelRefusal }) {
  return (
    <div className="panel-refusal" data-error={refusal.error || undefined}>
      <span className="panel-refusal-heading">{refusal.heading}</span>
      <span className="panel-refusal-message">{refusal.message}</span>
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

/**
 * The held roles' callouts above the bar (§7.2, SCH-04 `[M23]`; *05 → Column 3 · several tools (M23)*): "Model
 * unavailable", "Codex isn't available · hidden in Settings", "Claude Code limit reached · resets 3:45 pm", with Use
 * fallback, Open agent…, Sign in… and Show in Agent tools.
 */
function Holds({ view }: { view: PanelView }) {
  if (view.holds.length === 0) return null;
  return (
    <div className="panel-holds">
      {view.holds.map((hold) => (
        <ToolProblemCallout
          key={`${hold.role ?? ""}${hold.heading}`}
          callout={hold}
          look="edge"
          onAction={(action) => {
            toolProblemAction(action, { ticket: view.key });
          }}
        />
      ))}
    </div>
  );
}

/**
 * The ticket's choices and its path under the action bar, in every panel state (AGM-09; *05 → Row · Stage choices and
 * path (update AG §9)*): a pick goes to the service at once (`panel.set_choice`), and the panel follows the ticket's
 * change. None for an epic; only the path on a flow without options.
 */
function ChoicesAndPath({ view }: { view: PanelView }) {
  if (view.choices === null && view.path === null) return null;
  return (
    <div className="panel-agents">
      {view.choices !== null && (
        <StageChoices
          key={view.key}
          view={view.choices}
          look="panel"
          onPick={(stage, choice) => {
            request("panel.set_choice", { key: view.key, stage, choice } satisfies PanelChoiceParams).catch(
              ignore,
            );
          }}
        />
      )}
      {view.path !== null && <TicketPath view={view.path} />}
    </div>
  );
}

/** What the ticket's buttons, its "…" menu and a board card's button do, shared by the sheet's header and Overview. */
export interface PanelActions {
  press: (button: PanelButton) => void;
  choose: (item: string) => void;
  /** The button whose request is on its way. */
  busy: string | null;
  /** The ticket whose Answer the agent box is open. */
  answering: string | null;
  setAnswering: (key: string | null) => void;
  /** A card's answer button, handed to the answer list. */
  cardAnswer: CardAnswer | null;
}

/**
 * The ticket's actions (T-0222), held by the sheet while it shows `view`: its buttons and "…" menu, Close ticket and
 * Re-run validation as keys while it offers them (0097: palette only unless the user gives them a key), and a board
 * card's button for this ticket (T-0220), run once the sheet shows it.
 */
export function usePanelActions(view: PanelView): PanelActions {
  const [busy, setBusy] = useState<string | null>(null);
  const [answering, setAnswering] = useState<string | null>(null);
  const askMarkClosed = () => {
    if (view.mark_closed === null) return;
    openOverlay("confirm", {
      title: view.mark_closed.title,
      body: view.mark_closed.body,
      confirmLabel: view.mark_closed.confirm,
      destructive: true,
      onConfirm: () => request("panel.mark_closed", { key: view.key }),
    });
  };
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
      case "validator":
        showTab("terminal");
        requestTerminalFocus(view.key, terminalFocusRequests, "validator");
        return;
      case "answer_agent":
        setAnswering(view.key);
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
        // With open recorded decisions, "Approve · keeps N decisions" opens the choice: Local merge / Open PR (DEC-05).
        openOverlay(
          route.id,
          button.action === "approve" && view.approve_opens_menu
            ? { ticket: view.key, menu: true }
            : { ticket: view.key },
        );
        return;
      case "pass":
        setBusy(button.action);
        void passTicket(view.key, view.approve_opens_menu).finally(() => {
          setBusy(null);
        });
        return;
      case "mark_closed":
        askMarkClosed();
        return;
      // Run (RUN-01): Session's Run shows while it starts, or the run-command dialog asks first (T-0232).
      case "run":
        showTab("run");
        void startRun(view.key);
        return;
      case "open_agent":
        openOverlay("settings", agentSettings(view.open_agent));
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
  const latest = useRef({ view, press, choose });
  useLayoutEffect(() => {
    latest.current = { view, press, choose };
  });
  useShortcut(
    "close_ticket",
    useCallback(() => {
      const item = latest.current.view.menu.find((i) => i.id === CLOSE_TICKET && !i.disabled);
      if (!item) return false;
      latest.current.choose(item.id);
      return true;
    }, []),
  );
  useShortcut(
    "rerun_validation",
    useCallback(() => {
      const button = latest.current.view.buttons.find((b) => b.action === "retry_validation");
      if (!button) return false;
      latest.current.press(button);
      return true;
    }, []),
  );
  const [cardAnswer, setCardAnswer] = useState<CardAnswer | null>(null);
  const pendingCard = useStore(cardActions);
  const runCard = useEffectEvent((kind: PanelCardParams["kind"]) => {
    const route = cardRoute(kind);
    switch (route.kind) {
      case "service": {
        const send = (clear: boolean): Promise<unknown> =>
          (
            request("panel.card", {
              key: view.key,
              kind,
              clear,
            } satisfies PanelCardParams) as Promise<PanelSent>
          )
            .then((sent) => {
              askUnsent(sent, () => send(true));
            })
            .catch(ignore);
        void send(false);
        return;
      }
      case "pick":
        if (view.answer)
          setCardAnswer({ question: view.answer.question, kind: "pick", number: route.number });
        return;
      case "send_answer":
        if (view.answer) setCardAnswer({ question: view.answer.question, kind: "send_answer" });
        return;
      case "overlay":
        openOverlay(route.id, { ticket: view.key });
        return;
      case "pass":
        void passTicket(view.key, view.approve_opens_menu);
        return;
      case "mark_closed":
        askMarkClosed();
        return;
      case "run_tab":
        showTab("run");
        return;
      case "open_agent":
        openOverlay("settings", agentSettings(view.open_agent));
        return;
      case "none":
        return;
    }
  });
  useEffect(() => {
    if (pendingCard?.ticket !== view.key) return;
    const taken = takeCardAction(view.key);
    // eslint-disable-next-line react-hooks/set-state-in-effect -- the card's click arrives from outside React (a store), once
    if (taken) runCard(taken.kind);
  }, [pendingCard, view.key]);
  return { press, choose, busy, answering, setAnswering, cardAnswer };
}

/** The last signal, the tokens by agent and the Run's pill (the old header's meta row), when there are any. */
function MetaLine({ view }: { view: PanelView }) {
  if (view.signal === null && view.token_split === null && view.run === null) return null;
  return (
    <div className="panel-meta">
      {view.signal && (
        <span className="panel-meta-item" data-stale={view.signal.stale || undefined}>
          <Icon name="activity" size={11} />
          <span>{view.signal.text}</span>
        </span>
      )}
      {view.token_split !== null && (
        <span className="panel-split">
          <Icon name="coins" size={11} />
          <span>{view.token_split}</span>
        </span>
      )}
      {view.run && <RunPill ticket={view.key} pill={view.run} />}
    </div>
  );
}

/**
 * The top of the sheet's Overview until T-0465 (SHT-03's "what it needs from you"): the old panel header's lower
 * part (the conflict strip, the question with its options or the action bar with its callouts, the Auto-merge stop
 * message, Answer the agent's box), the choices and the path, the queued notice, the registered sections (Epic,
 * Stack, Messages; an epic's Messages sit with its Ticket content, accepted Q9), the open PR's box and the Start
 * interrupted callout.
 */
export function PanelOverview({ view, actions }: { view: PanelView; actions: PanelActions }) {
  const app = useRegistries();
  const { press, busy, answering, setAnswering, cardAnswer } = actions;
  const UnderTabs = app.panelSections.get(PANEL_UNDER_TABS)?.Component;
  return (
    <div className="panel-overview">
      <div className="panel-header">
        <MetaLine view={view} />
        {view.conflict && (
          <div className="panel-conflict">
            <Icon name="triangle-alert" size={13} />
            <span className="panel-conflict-line">{view.conflict.line}</span>
          </div>
        )}
        {view.answer ? (
          <AnswerBox
            key={view.answer.question}
            view={view}
            answer={view.answer}
            more={null}
            fromCard={cardAnswer}
          />
        ) : (
          <>
            {view.refusal && <Refusal refusal={view.refusal} />}
            <Holds view={view} />
            {view.buttons.length > 0 && (
              <div className="panel-bar">
                {view.buttons.map((button) => (
                  <span key={button.action} className="panel-action-group">
                    <ActionButton button={button} busy={busy === button.action} onPress={press} />
                    {button.action === "approve" && view.approve_more !== null && (
                      <Tooltip text={view.approve_more}>
                        <Button
                          label="▾"
                          onClick={() => {
                            openOverlay("local_merge", { ticket: view.key, menu: true });
                          }}
                        />
                      </Tooltip>
                    )}
                  </span>
                ))}
              </div>
            )}
          </>
        )}
        {view.stop_message !== null && (
          <p className="panel-stop-message">
            <Icon name="triangle-alert" size={13} />
            <span>{view.stop_message}</span>
          </p>
        )}
        {answering === view.key && view.answer_agent && (
          <AnswerAgentBox
            key={view.key}
            view={view}
            box={view.answer_agent}
            onClose={() => {
              setAnswering(null);
            }}
          />
        )}
      </div>
      <ChoicesAndPath view={view} />
      {view.notice && <Notice view={view} notice={view.notice} />}
      {PANEL_SECTIONS.map((id) => {
        if (id === "messages" && view.epic) return null;
        const Section = app.panelSections.get(id)?.Component;
        return Section ? <Section key={id} ticket={view.key} /> : null;
      })}
      {UnderTabs && <UnderTabs ticket={view.key} />}
      {view.start_problem && <StartProblem key={view.key} view={view} problem={view.start_problem} />}
    </div>
  );
}

/** "● :4020 ↗ ▢" while the Run is up (*13 → Card & panel-header controls*): Open (with an Open URL) and Stop. */
function RunPill({ ticket, pill }: { ticket: string; pill: PanelRunPill }) {
  return (
    <span className="panel-run">
      <span className="panel-run-dot" />
      <button
        type="button"
        className="panel-run-port"
        onClick={() => {
          showTab("run");
        }}
      >
        {pill.port}
      </button>
      {pill.url !== null && (
        <Tooltip text={pill.open_hint}>
          <button
            type="button"
            className="panel-run-icon"
            aria-label={pill.open_hint}
            onClick={() => {
              request("run.open", { url: pill.url }).catch(ignore);
            }}
          >
            <Icon name="external-link" size={11} />
          </button>
        </Tooltip>
      )}
      <Tooltip text={pill.stop_hint}>
        <button
          type="button"
          className="panel-run-icon"
          aria-label={pill.stop_hint}
          onClick={() => {
            request("run.stop", { key: ticket }).catch(ignore);
          }}
        >
          <Icon name="square" size={11} />
        </button>
      </Tooltip>
    </span>
  );
}
