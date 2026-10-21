// The Ticket tab (UI-05, SES-08; d0019, d0048; *08 → Ticket tab · read mode, with agent edits, Compare · before /
// after, Editing while the agent works*): the editor in read mode inside the panel; Edit (the panel's bar, its "…"
// menu or + Add dependency, through `ticketEditRequests`) opens the form in place. Saving a working ticket tells its
// agent what changed: the note says so, with Open terminal. While it validates, the form warns first that saving
// cancels the validation (VAL-08, *Editing while it validates*). Expand on the description's label row shows it over the
// whole window (T-0260, d0078 D11). The agent's recorded decisions sit under it, read-only (T-0288, DEC-03), then the
// ticket's comments and their box (T-0446, CMT-10) under "Description · the plan".
import { useCallback, useEffect, useRef, useState } from "react";

import type { TicketProps } from "../../app/registry";
import { requestTerminalFocus, showTab, ticketEditRequests } from "../../app/selection";
import { useStore } from "../../app/store";
import { Button, Kbd } from "../../components";
import type { EditorCompareEntry } from "../../generated/EditorCompareEntry";
import type { EditorField } from "../../generated/EditorField";
import type { EditorView } from "../../generated/EditorView";
import { FullView, useFullView } from "../../markdown";
import { useServiceView } from "../../service";
import { TicketDecisions } from "../sections/Decisions";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { CommentThread } from "./CommentThread";
import { EditorFields } from "./Fields";
import { EDITOR_EVENTS, SAVE_KEYS } from "./logic";
import { useFormKeys } from "./Modal";
import { Compare, CopyMarkdown, Description as ReadDescription, ReadMode } from "./ReadMode";
import { useEditor } from "./useEditor";

const ignore = () => undefined;

type Mode = { kind: "read" } | { kind: "edit" } | { kind: "compare"; entries: readonly EditorCompareEntry[] };

/** Takes the edit request meant for `ticket`, once (a later visit to the tab doesn't open the form again). */
export function takeEditRequest(ticket: string): boolean {
  if (ticketEditRequests.get()?.ticket !== ticket) return false;
  ticketEditRequests.set(null);
  return true;
}

/** Gives the description field the height of the tab's scroll area as `--editor-fill`: it may grow until it fills the tab
 * (d0078 D5). */
function useFillHeight(scroll: React.RefObject<HTMLElement | null>) {
  useEffect(() => {
    const el = scroll.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const fit = () => {
      el.style.setProperty("--editor-fill", `${String(el.clientHeight)}px`);
    };
    const observer = new ResizeObserver(fit);
    observer.observe(el);
    return () => {
      observer.disconnect();
    };
  }, [scroll]);
}

function EditForm({ view, onDone }: { view: EditorView; onDone: (note: string | null) => void }) {
  const editor = useEditor(view, { mode: "edit", key: view.key ?? "" });
  const root = useRef<HTMLDivElement>(null);
  const scroll = useRef<HTMLDivElement>(null);
  useFillHeight(scroll);
  const save = () => {
    editor.save((saved) => {
      onDone(saved.note);
    });
  };
  // Esc in a field discards (the panel's Esc waits while a field has the keyboard). In the description it steps back
  // first (d0078 D4): an open popover or menu closes, then the text gives up focus, then the form discards.
  useFormKeys(root, save, () => {
    onDone(null);
  });
  const t = view.texts;
  const bodyChanged = editor.form.body !== view.form.body;
  return (
    <div ref={root} className="editor-tab-form">
      <div ref={scroll} className="editor-tab-scroll">
        {view.save_warning !== null && (
          <div className="editor-warning" role="status">
            <Icon name="triangle-alert" size={14} />
            <p>{view.save_warning}</p>
          </div>
        )}
        <EditorFields
          view={view}
          form={editor.form}
          onChange={editor.setForm}
          titleMissing={editor.titleMissing}
          refusal={editor.refusal}
          chips={editor.chips}
          onAddDep={editor.addDep}
          onRemoveDep={editor.removeDep}
          autoFocusTitle
          onSave={save}
        />
      </div>
      <div className="editor-footer">
        <span className="dialog-hint">
          <Kbd keys={SAVE_KEYS} />
          <span>{t.save_word}</span>
          <Kbd keys="esc" />
          <span>{t.discard_word}</span>
        </span>
        {view.save_hint !== null && <span className="editor-save-hint">{view.save_hint}</span>}
        {view.unsaved_warning !== null && bodyChanged && (
          <span className="editor-unsaved">
            <Icon name="pencil" size={11} />
            {view.unsaved_warning}
          </span>
        )}
        <span className="editor-spacer" />
        <Button
          variant="ghost"
          label={t.cancel}
          onClick={() => {
            onDone(null);
          }}
        />
        <Button label={t.primary} icon="check" loading={editor.saving} onClick={save} />
      </div>
    </div>
  );
}

export function TicketTab({ ticket }: TicketProps) {
  const load = useCallback(
    () => request("editor.open", { mode: "edit", key: ticket }) as Promise<EditorView>,
    [ticket],
  );
  const { view } = useServiceView(load, EDITOR_EVENTS);
  const [mode, setMode] = useState<Mode>({ kind: "read" });
  const [note, setNote] = useState<string | null>(null);
  const pending = useStore(ticketEditRequests);
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- the Edit click arrives from outside React (a store), once
    if (pending?.ticket === ticket && takeEditRequest(ticket)) setMode({ kind: "edit" });
  }, [pending, ticket]);
  // Read mode's description over the whole window (T-0260); it closes once read mode isn't shown.
  const full = useFullView();
  const reading = view?.read != null && view.key === ticket && mode.kind === "read";
  const { collapse } = full;
  useEffect(() => {
    if (!reading) collapse();
  });
  if (!view?.read || view.key !== ticket) return null;
  const t = view.texts;
  if (mode.kind === "edit") {
    return (
      <EditForm
        key={ticket}
        view={view}
        onDone={(saved) => {
          setNote(saved);
          setMode({ kind: "read" });
        }}
      />
    );
  }
  const compare = (field: EditorField | null) => {
    (request("editor.compare", { key: ticket, field }) as Promise<EditorCompareEntry[]>).then((entries) => {
      setMode({ kind: "compare", entries });
    }, ignore);
  };
  return (
    <div className="editor-tab">
      <div className="editor-tab-scroll">
        {note !== null && (
          <div className="editor-note">
            <Icon name="info" size={14} />
            <div>
              <p>{note}</p>
              <Button
                variant="secondary"
                icon="square-terminal"
                label={t.open_terminal}
                onClick={() => {
                  showTab("terminal");
                  requestTerminalFocus(ticket);
                }}
              />
            </div>
          </div>
        )}
        {mode.kind === "compare" ? (
          <Compare entries={mode.entries} texts={t} />
        ) : (
          <ReadMode
            ticket={ticket}
            read={view.read}
            texts={t}
            labels={{
              title: t.title,
              body: t.plan,
              priority: t.priority,
              deps: t.deps,
              base: t.base,
              externalId: t.external_id,
            }}
            onCompare={compare}
            onExpand={full.expand}
          />
        )}
        {mode.kind === "read" && <TicketDecisions ticket={ticket} />}
        {mode.kind === "read" && <CommentThread ticket={ticket} />}
      </div>
      <div className="editor-footer">
        <span className="editor-spacer" />
        {mode.kind === "compare" ? (
          <Button
            label={t.close}
            onClick={() => {
              setMode({ kind: "read" });
            }}
          />
        ) : (
          <Button
            variant="secondary"
            icon="pencil"
            label={t.edit}
            onClick={() => {
              setNote(null);
              setMode({ kind: "edit" });
            }}
          />
        )}
      </div>
      {full.slot !== null && (
        <FullView
          slot={full.slot}
          ticketKey={ticket}
          name={t.body}
          title={view.read.title}
          tools={
            <span className="editor-tools">
              <CopyMarkdown stored={view.read.body_markdown} texts={t} />
            </span>
          }
          collapseLabel={t.collapse}
          onCollapse={full.collapse}
        >
          <ReadDescription read={view.read} texts={t} label={t.body} />
        </FullView>
      )}
    </div>
  );
}
