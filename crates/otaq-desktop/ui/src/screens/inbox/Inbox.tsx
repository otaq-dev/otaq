// The Needs-you inbox (*10 · Needs-you inbox*; UI-06, ATT-06, ATT-08, ATT-09, ATT-10, ATT-12, USG-07, MSG-05, CLS-04):
// a 440 px popover under the header's inbox button with the total and Next ⌘J, the usage-limit row once at the top,
// the groups with their counts and Resume all, each row with its project, reason and age, its Answer box (numbered
// options with the recommended one picked, or free text), its buttons (the validation and Auto-merge items' actions
// inline, "Answer the agent…" with its note box), the waiting Project agents (named, no ticket key; opening one shows
// it in the Project agent pane), an epic lead's items (*10 → Row · Epic lead items (0083)*: the question it
// passed on with "Recommended by the lead" and its reason, a child's second stop with the lead's note and view, its
// suggestions with Close… / Delete… (the usual confirmation) and Dismiss, its own question opening the epic panel on
// its Lead tab), and the foot's keys. ↑↓ move, ↵ opens,
// 1–4 pick in an open answer box, Esc cancels the box or closes the inbox. Everything it says comes from
// `inbox.view`; it keeps only the selected row and the open draft.
import { type KeyboardEvent, useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from "react";

import { modalOpen, openOverlay, overlays } from "../../app/overlays";
import { type OverlayProps, useShortcut } from "../../app/registry";
import { Button, Kbd, ToolAvatar } from "../../components";
import type { InboxAnswer } from "../../generated/InboxAnswer";
import type { InboxGroup } from "../../generated/InboxGroup";
import type { InboxLeadNote } from "../../generated/InboxLeadNote";
import type { InboxLoad } from "../../generated/InboxLoad";
import type { InboxNoteBox } from "../../generated/InboxNoteBox";
import type { InboxRowView } from "../../generated/InboxRowView";
import type { InboxView } from "../../generated/InboxView";
import type { PanelSent } from "../../generated/PanelSent";
import { watchView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { toolProblemAction } from "../panel/toolProblemRunner";
import {
  canSend,
  changeAgent,
  describeAction,
  initialPick,
  landNext,
  openRow,
  opensNote,
  pickByKey,
  picksOther,
  step,
  suggestionOverlay,
  waitingRows,
} from "./logic";
import { ROUTES } from "./routes";

/** The service events that change the inbox (`settings_changed`: the user's key for Next, KEY-08). */
export const INBOX_EVENTS = [
  "inbox_changed",
  "ticket_changed",
  "ticket_deleted",
  "usage_changed",
  "settings_changed",
] as const;

const ignore = () => undefined;

/** ⌘J: the next item with its terminal focused, or "All clear". */
export function runNext(): Promise<void> {
  return (request("inbox.next") as Promise<Parameters<typeof landNext>[0]>).then((next) => {
    landNext(next, ROUTES);
  }, ignore);
}

/** The reply box open in a row: its answer, or the note of "Answer the agent…" (`note`). */
interface Draft {
  key: string;
  pick: number | null;
  text: string;
  sending: boolean;
  note?: boolean;
}

/** "Answer the agent…": the note goes to the agent with the findings (`ticket.validation_action`). */
function NoteBox({
  row,
  box,
  draft,
  onChange,
  onCancel,
  onSent,
}: {
  row: InboxRowView;
  box: InboxNoteBox;
  draft: Draft;
  onChange: (draft: Draft) => void;
  onCancel: () => void;
  onSent: () => void;
}) {
  const field = useRef<HTMLInputElement>(null);
  useEffect(() => {
    field.current?.focus();
  }, []);
  const ready = draft.text.trim() !== "" && !draft.sending;
  const submit = () => {
    if (!ready) return;
    onChange({ ...draft, sending: true });
    request("inbox.act", { key: row.key, action: { kind: "answer_agent", note: draft.text.trim() } }).then(
      onSent,
      () => {
        onChange({ ...draft, sending: false });
      },
    );
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key !== "Enter" && e.key !== "Escape") return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Enter") submit();
    else onCancel();
  };
  return (
    // eslint-disable-next-line jsx-a11y/no-static-element-interactions -- the box's own keys (↵, Esc) for its field
    <div className="inbox-answer" onKeyDown={onKey}>
      <input
        ref={field}
        className="inbox-answer-field"
        aria-label={box.placeholder}
        placeholder={box.placeholder}
        value={draft.text}
        onChange={(e) => {
          onChange({ ...draft, text: e.target.value });
        }}
      />
      <div className="inbox-answer-foot">
        <span className="inbox-muted">{box.text}</span>
        <Button compact variant="secondary" label={box.cancel} onClick={onCancel} />
        <Button
          compact
          icon={box.send_icon}
          label={box.send}
          disabled={!ready}
          loading={draft.sending}
          onClick={submit}
        />
        <Kbd keys="↵" />
      </div>
    </div>
  );
}

function send(row: InboxRowView, answer: InboxAnswer, draft: Draft, clear = false): Promise<PanelSent> {
  return request("inbox.answer", {
    key: row.key,
    agent: row.agent?.id ?? null,
    question: answer.question,
    message: answer.message,
    pick: draft.pick,
    text: draft.text,
    clear,
  }) as Promise<PanelSent>;
}

function AnswerBox({
  row,
  answer,
  draft,
  onChange,
  onCancel,
  onSent,
}: {
  row: InboxRowView;
  answer: InboxAnswer;
  draft: Draft;
  onChange: (draft: Draft) => void;
  onCancel: () => void;
  onSent: () => void;
}) {
  const field = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const noteId = useId();
  const options = answer.options;
  const showField = options.length === 0 || picksOther(draft.pick, options);
  useEffect(() => {
    if (showField) field.current?.focus();
    else list.current?.focus();
  }, [showField]);
  const ready = canSend(draft.pick, draft.text, options) && !draft.sending;
  const submit = () => {
    if (!ready) return;
    onChange({ ...draft, sending: true });
    const done = (sent: PanelSent) => {
      if (sent.outcome === "ask") {
        openOverlay("confirm", {
          title: sent.title,
          body: sent.body,
          confirmLabel: sent.clear,
          cancelLabel: sent.cancel,
          destructive: false,
          onConfirm: () => send(row, answer, draft, true).then(onSent, ignore),
        });
        onChange({ ...draft, sending: false });
        return;
      }
      onSent();
    };
    send(row, answer, draft).then(done, () => {
      onChange({ ...draft, sending: false });
    });
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      submit();
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
      return;
    }
    if (e.target === list.current) {
      const n = pickByKey(e.key, options);
      if (n !== null) {
        e.preventDefault();
        onChange({ ...draft, pick: n });
      }
    }
  };
  return (
    // eslint-disable-next-line jsx-a11y/no-static-element-interactions -- the box's own keys (1–4, ↵, Esc) for its list and field
    <div className="inbox-answer" onKeyDown={onKey}>
      {answer.heading !== null && (
        <div className="inbox-options" ref={list} tabIndex={-1} role="radiogroup" aria-label={answer.heading}>
          <div className="inbox-options-heading">{answer.heading}</div>
          {options.map((o) => (
            <button
              type="button"
              key={o.number}
              role="radio"
              aria-checked={draft.pick === o.number}
              aria-describedby={o.note === null ? undefined : `${noteId}-${String(o.number)}`}
              className="inbox-option"
              data-picked={draft.pick === o.number || undefined}
              onClick={() => {
                onChange({ ...draft, pick: o.number });
              }}
            >
              <span className="inbox-option-radio" />
              <span className="inbox-option-text">
                <span className="inbox-option-line">
                  <span className="inbox-option-label">{o.label}</span>
                  {o.recommended && (
                    <span className="inbox-option-recommended">
                      <Icon name="star" size={10} />
                      {answer.recommended}
                    </span>
                  )}
                </span>
                {/* The option's note (ATT-17), whole and wrapped: its description, not part of its name. */}
                {o.note !== null && (
                  <span id={`${noteId}-${String(o.number)}`} className="inbox-option-note" aria-hidden>
                    {o.note}
                  </span>
                )}
                {o.lead_reason !== null && <span className="inbox-option-note">{o.lead_reason}</span>}
              </span>
              <Kbd keys={String(o.number)} />
            </button>
          ))}
        </div>
      )}
      {showField && (
        <input
          ref={field}
          className="inbox-answer-field"
          aria-label={answer.placeholder}
          placeholder={answer.placeholder}
          value={draft.text}
          onChange={(e) => {
            onChange({ ...draft, text: e.target.value });
          }}
        />
      )}
      <div className="inbox-answer-foot">
        <span className="inbox-muted">{answer.logged}</span>
        <Button compact variant="secondary" label={answer.cancel} onClick={onCancel} />
        <Button
          compact
          icon={answer.send_icon}
          label={answer.send}
          disabled={!ready}
          loading={draft.sending}
          onClick={submit}
        />
        <Kbd keys="↵" />
      </div>
    </div>
  );
}

/** A child's second stop: the lead's one note and its view (LEAD-07). */
function LeadNote({ note }: { note: InboxLeadNote }) {
  return (
    <>
      <div className="inbox-lead-box">
        <span className="inbox-lead-heading">
          <Icon name="bot" size={11} />
          {note.heading}
        </span>
        <span>{note.text}</span>
      </div>
      {note.view !== null && (
        <div className="inbox-lead-box">
          <span className="inbox-lead-heading">
            <Icon name="bot" size={11} />
            {note.view_heading}
          </span>
          <span>{note.view}</span>
        </div>
      )}
    </>
  );
}

function Row({
  row,
  selected,
  draft,
  onDraft,
  onSelect,
}: {
  row: InboxRowView;
  selected: boolean;
  draft: Draft | null;
  onDraft: (draft: Draft | null) => void;
  onSelect: () => void;
}) {
  const answer = row.answer;
  const noting = draft !== null && draft.key === row.key && draft.note === true && row.note !== null;
  const drafting = draft !== null && draft.key === row.key && answer !== null && !noting;
  const open = () => {
    openRow(row, ROUTES);
  };
  return (
    // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- ↵ on the list opens the selected row (the inbox's keys)
    <div
      className="inbox-row"
      data-selected={selected || undefined}
      data-resolved={row.resolved || undefined}
      onClick={open}
      onPointerEnter={row.resolved ? undefined : onSelect}
    >
      <span className="inbox-row-icon status-pill" data-palette={row.pill.palette}>
        <Icon name={row.pill.icon} size={13} />
      </span>
      <div className="inbox-row-body">
        <div className="inbox-row-line">
          <span className="inbox-project">{row.project_name}</span>
          {row.keyed && <span className="inbox-key">{row.key}</span>}
          <span className="inbox-title">{row.title}</span>
        </div>
        {row.via !== null && (
          <div className="inbox-via">
            <Icon name="bot" size={12} />
            {row.via}
          </div>
        )}
        <div className="inbox-reason">{row.reason}</div>
        {row.lead_note !== null && <LeadNote note={row.lead_note} />}
        <div className="inbox-meta">
          <span>{row.age}</span>
          {row.hint !== null && (
            <span className="inbox-hint">
              {[row.hint.keys, row.hint.label].filter((t) => t !== "").join(" ")}
            </span>
          )}
          {row.blocks !== null && (
            <span className="inbox-blocks">
              <Icon name="link-2" size={10} />
              {row.blocks}
            </span>
          )}
        </div>
        {row.restart_note !== null && (
          <div className="inbox-note" data-tone="attention">
            <Icon name="zap-off" size={11} />
            {row.restart_note}
          </div>
        )}
        {row.sent_note !== null && (
          <div className="inbox-note">
            <Icon name="inbox" size={11} />
            {row.sent_note}
          </div>
        )}
        {((answer !== null && !drafting) || (row.buttons.length > 0 && !noting)) && (
          // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- keeps a button's click from opening the row
          <div
            className="inbox-buttons"
            onClick={(e) => {
              e.stopPropagation();
            }}
          >
            {answer !== null && !drafting && (
              <Button
                compact
                icon={answer.open_icon}
                label={answer.open}
                onClick={() => {
                  onDraft({ key: row.key, pick: initialPick(answer.options), text: "", sending: false });
                }}
              />
            )}
            {!noting &&
              row.buttons.map((b) => (
                <Button
                  key={describeAction(b.action)}
                  compact
                  variant={b.primary ? "primary" : "secondary"}
                  icon={b.icon}
                  label={b.label}
                  onClick={() => {
                    if (b.action.kind === "change_agent") {
                      changeAgent(row, b.action.role, ROUTES);
                      return;
                    }
                    if (opensNote(b.action)) {
                      onDraft({ key: row.key, pick: null, text: "", sending: false, note: true });
                      return;
                    }
                    const confirm = suggestionOverlay(row, b.action);
                    if (confirm !== null) {
                      const [id, params] = confirm;
                      if (id === "close_ticket") openOverlay("close_ticket", params);
                      else openOverlay("delete_ticket", params);
                      return;
                    }
                    request("inbox.act", { key: row.key, action: b.action }).catch(ignore);
                  }}
                />
              ))}
          </div>
        )}
        {noting && row.note !== null && (
          // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- keeps a click in the box from opening the row
          <div
            onClick={(e) => {
              e.stopPropagation();
            }}
          >
            <NoteBox
              row={row}
              box={row.note}
              draft={draft}
              onChange={onDraft}
              onCancel={() => {
                onDraft(null);
              }}
              onSent={() => {
                onDraft(null);
              }}
            />
          </div>
        )}
        {drafting && (
          // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- keeps a click in the box from opening the row
          <div
            onClick={(e) => {
              e.stopPropagation();
            }}
          >
            <AnswerBox
              row={row}
              answer={answer}
              draft={draft}
              onChange={onDraft}
              onCancel={() => {
                onDraft(null);
              }}
              onSent={() => {
                onDraft(null);
              }}
            />
          </div>
        )}
      </div>
      {selected && !drafting && !noting && <Kbd keys="↵" />}
    </div>
  );
}

function GroupHead({ group }: { group: InboxGroup }) {
  return (
    <div className="inbox-group" data-group={group.id}>
      <span className="inbox-group-title">{group.title}</span>
      <span className="inbox-group-count">{group.count}</span>
      <span className="inbox-spacer" />
      {group.resume_all !== null && (
        <button
          type="button"
          className="inbox-resume-all"
          onClick={() => {
            openOverlay("resume_all");
          }}
        >
          {group.resume_all}
        </button>
      )}
    </div>
  );
}

export interface InboxBodyProps {
  view: InboxView;
  selected: number;
  draft: Draft | null;
  onDraft: (draft: Draft | null) => void;
  onSelect: (index: number) => void;
}

/** The inbox for a loaded view (also what the markup tests render). */
export function InboxBody({ view, selected, draft, onDraft, onSelect }: InboxBodyProps) {
  const waiting = waitingRows(view);
  return (
    <>
      <div className="inbox-head">
        <Icon name="inbox" size={15} />
        <span className="inbox-head-title">{view.title}</span>
        <span className="inbox-head-count" data-active={view.total > 0 || undefined}>
          {view.total}
        </span>
        <span className="inbox-muted">{view.scope}</span>
        <span className="inbox-spacer" />
        {view.next !== null && (
          <>
            <Button
              compact
              icon="arrow-right"
              label={view.next.label}
              onClick={() => {
                void runNext();
              }}
            />
            {view.next.keys !== "" && <Kbd keys={view.next.keys} />}
          </>
        )}
      </div>
      {view.limits.map((limit) => (
        <div key={limit.tool} className="inbox-limit">
          {limit.avatar ? <ToolAvatar avatar={limit.avatar} /> : <Icon name="gauge" size={13} />}
          <span>{limit.text}</span>
          {limit.button !== null && (
            <Button
              compact
              variant="secondary"
              icon={limit.button.icon}
              label={limit.button.label}
              onClick={() => {
                if (limit.button) toolProblemAction(limit.button.action, {});
              }}
            />
          )}
          {limit.resume !== null && (
            <Button
              compact
              icon="play"
              label={limit.resume}
              onClick={() => {
                request("limits.resume_tool", { tool: limit.resume_tool }).catch(ignore);
              }}
            />
          )}
        </div>
      ))}
      <div className="inbox-list">
        {view.groups.map((group) => (
          <div key={group.id}>
            <GroupHead group={group} />
            {group.rows.map((row) => {
              const index = waiting.indexOf(row);
              return (
                <Row
                  key={row.key}
                  row={row}
                  selected={!row.resolved && index === selected}
                  draft={draft}
                  onDraft={onDraft}
                  onSelect={() => {
                    onSelect(index);
                  }}
                />
              );
            })}
          </div>
        ))}
        {view.empty !== null && (
          <div className="inbox-empty">
            <span className="inbox-empty-icon">
              <Icon name="check" size={20} />
            </span>
            <strong>{view.empty.title}</strong>
            <span>{view.empty.text}</span>
          </div>
        )}
      </div>
      {view.foot.length > 0 && (
        <div className="inbox-foot">
          {view.foot.map((hint) => (
            <span key={hint.label} className="inbox-foot-hint">
              <Kbd keys={hint.keys} />
              {hint.label}
            </span>
          ))}
        </div>
      )}
    </>
  );
}

/** Where the popover sits: under the header's inbox button, its right edge on the button's. */
function usePlace(): { top: number; right: number } {
  const [place, setPlace] = useState({ top: 52, right: 16 });
  useLayoutEffect(() => {
    const place = () => {
      const button = document.querySelector(".header-inbox");
      if (!button) return;
      const r = button.getBoundingClientRect();
      setPlace({ top: r.bottom + 6, right: Math.max(8, document.documentElement.clientWidth - r.right) });
    };
    place();
    window.addEventListener("resize", place);
    return () => {
      window.removeEventListener("resize", place);
    };
  }, []);
  return place;
}

/** The `inbox` overlay. */
export function Inbox({ close }: OverlayProps<"inbox">) {
  const [loaded, setLoaded] = useState<InboxLoad>();
  // `again` asks once more after a resolved row has faded out.
  const [again, setAgain] = useState(0);
  useEffect(
    () =>
      watchView(
        () => request("inbox.view") as Promise<InboxLoad>,
        INBOX_EVENTS,
        (state) => {
          if (state.view) setLoaded(state.view);
        },
      ),
    [again],
  );
  const view = loaded?.view ?? null;
  const [selected, setSelected] = useState(0);
  const [draft, setDraft] = useState<Draft | null>(null);
  const box = useRef<HTMLDivElement>(null);
  const place = usePlace();

  // A resolved row fades out: ask again once it has gone.
  const refresh = loaded?.refresh_ms ?? null;
  useEffect(() => {
    if (refresh === null) return;
    const timer = window.setTimeout(() => {
      setAgain((n) => n + 1);
    }, refresh);
    return () => {
      window.clearTimeout(timer);
    };
  }, [refresh, loaded]);

  // The keyboard goes to the inbox when it opens.
  useEffect(() => {
    box.current?.focus();
  }, []);

  // Esc cancels the answer box, else closes; a press outside (and outside the header button, which toggles) closes.
  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      // A dialog over the inbox (the unsent-text question, Resume all) takes its own Esc.
      if (e.key !== "Escape" || modalOpen(overlays.store.get())) return;
      e.stopPropagation();
      if (draft !== null) setDraft(null);
      else close();
    };
    const onPress = (e: PointerEvent) => {
      const target = e.target as Element | null;
      if (box.current?.contains(target) || target?.closest(".header-inbox")) return;
      // A dialog opened from the inbox (the unsent-text question, Resume all) keeps it open.
      if (target?.closest('[role="dialog"]')) return;
      close();
    };
    document.addEventListener("keydown", onKey, true);
    document.addEventListener("pointerdown", onPress);
    return () => {
      document.removeEventListener("keydown", onKey, true);
      document.removeEventListener("pointerdown", onPress);
    };
  }, [close, draft]);

  const rows = view ? waitingRows(view) : [];
  const current = Math.min(selected, Math.max(0, rows.length - 1));
  // j / k (Next / previous ticket, 0097) walk the rows as ↓ / ↑ do, while the keyboard is on the list.
  const walk = useRef<(by: 1 | -1) => boolean>(() => false);
  useLayoutEffect(() => {
    walk.current = (by) => {
      if (draft !== null || document.activeElement !== box.current) return false;
      setSelected(step(current, by, rows.length));
      return true;
    };
  });
  useShortcut(
    "next_ticket",
    useCallback(() => walk.current(1), []),
  );
  useShortcut(
    "previous_ticket",
    useCallback(() => walk.current(-1), []),
  );
  const onKey = (e: KeyboardEvent) => {
    if (draft !== null || e.target !== box.current) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      setSelected(step(current, e.key === "ArrowDown" ? 1 : -1, rows.length));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const row = rows[current];
      if (row) openRow(row, ROUTES);
    }
  };
  return (
    // eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions -- the popover's own keys: ↑↓ move, ↵ opens
    <div
      ref={box}
      className="inbox"
      role="dialog"
      aria-label={view?.title}
      // The app's single letters work here (KEY-02: j / k), unlike in other dialogs.
      data-letters=""
      tabIndex={-1}
      style={{ top: place.top, right: place.right }}
      onKeyDown={onKey}
    >
      {view && (
        <InboxBody view={view} selected={current} draft={draft} onDraft={setDraft} onSelect={setSelected} />
      )}
    </div>
  );
}
