// One editor's interaction state (T-0228): the form as typed, the save in flight, the title error once a save was
// tried, the refusal under its field, and a new ticket's dependency chips. Shared by the modal and the Ticket tab.
import { useState } from "react";

import type { EditorDepChip } from "../../generated/EditorDepChip";
import type { EditorForm } from "../../generated/EditorForm";
import type { EditorLinked } from "../../generated/EditorLinked";
import type { EditorPickerRow } from "../../generated/EditorPickerRow";
import type { EditorRefusal } from "../../generated/EditorRefusal";
import type { EditorSaved } from "../../generated/EditorSaved";
import type { EditorTarget } from "../../generated/EditorTarget";
import type { EditorView } from "../../generated/EditorView";
import { request } from "../../tauri";
import { canSave, withDep, withoutDep } from "./logic";

const ignore = () => undefined;

export interface Editor {
  form: EditorForm;
  setForm: (form: EditorForm) => void;
  saving: boolean;
  titleMissing: boolean;
  refusal: EditorRefusal | null;
  chips: readonly EditorDepChip[];
  addDep: (row: EditorPickerRow) => void;
  removeDep: (key: string) => void;
  /** Saves; `done` gets the outcome once it is saved. */
  save: (done: (saved: Extract<EditorSaved, { outcome: "saved" }>) => void) => void;
  /** A new ticket was created but a link was refused: the editor goes on as that ticket's. */
  created: string | null;
}

export function useEditor(view: EditorView, target: EditorTarget, initial?: EditorForm): Editor {
  const [form, setForm] = useState<EditorForm>(initial ?? view.form);
  const [saving, setSaving] = useState(false);
  const [titleMissing, setTitleMissing] = useState(false);
  const [refusal, setRefusal] = useState<EditorRefusal | null>(null);
  const [created, setCreated] = useState<string | null>(null);
  // A new ticket's picks with their status; an existing ticket's links come from the service.
  const [picked, setPicked] = useState<readonly EditorDepChip[]>([]);
  const [linked, setLinked] = useState<readonly EditorDepChip[] | null>(null);
  const linksNow = view.create && created === null;
  const chips = linksNow ? picked.filter((c) => form.deps.includes(c.key)) : (linked ?? view.deps);
  const link = (parent: string, add: boolean) => {
    const key = created ?? view.key;
    if (key === null) return;
    setRefusal(null);
    (request("editor.link", { key, parent, add }) as Promise<EditorLinked>).then((out) => {
      if (out.outcome === "linked") setLinked(out.deps);
      else setRefusal(out.refusal);
    }, ignore);
  };
  return {
    form,
    setForm: (next) => {
      setForm(next);
      if (refusal?.field === "form" || refusal?.field === "base") setRefusal(null);
    },
    saving,
    titleMissing,
    refusal,
    chips,
    created,
    addDep: (row) => {
      if (linksNow) {
        setPicked((p) => [...p.filter((c) => c.key !== row.key), { key: row.key, pill: row.pill }]);
        setForm((f) => withDep(f, row.key));
      } else {
        link(row.key, true);
      }
    },
    removeDep: (key) => {
      if (linksNow) setForm((f) => withoutDep(f, key));
      else link(key, false);
    },
    save: (done) => {
      setTitleMissing(true);
      if (!canSave(form, saving)) return;
      setSaving(true);
      setRefusal(null);
      const to: EditorTarget = created === null ? target : { mode: "edit", key: created };
      (request("editor.save", { target: to, form }) as Promise<EditorSaved>)
        .then((out) => {
          if (out.outcome === "saved") done(out);
          else if (out.outcome === "refused") {
            setRefusal(out.refusal);
            if (out.created !== null) setCreated(out.created);
          }
        }, ignore)
        .finally(() => {
          setSaving(false);
        });
    },
  };
}
