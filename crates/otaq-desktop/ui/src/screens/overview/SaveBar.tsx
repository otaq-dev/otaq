// The tab row's right end while the description is edited in place (SHT-03; T-0465; `s25-04`): "Description changed",
// Discard and Save ⌘S, only once the text differs from what is saved. The open editor publishes itself in
// `descriptionDraft`; the sheet draws this at the end of its tab row.
import { useStore } from "../../app/store";
import { Button, Kbd } from "../../components";
import type { DescriptionSaveBar as Bar } from "../../generated/DescriptionSaveBar";
import { descriptionDraft } from "./draft";

/** The bar itself (the gallery draws it too). */
export function SaveBarView({
  bar,
  saving,
  onSave,
  onDiscard,
}: {
  bar: Bar;
  saving: boolean;
  onSave: () => void;
  onDiscard: () => void;
}) {
  return (
    <div className="overview-save-bar" role="status">
      <span className="overview-save-changed">
        <span className="overview-save-dot" aria-hidden />
        {bar.changed}
      </span>
      <Button compact variant="ghost" label={bar.discard} onClick={onDiscard} />
      <Button compact icon="check" label={bar.save} loading={saving} onClick={onSave} />
      <Kbd keys={bar.keys} />
    </div>
  );
}

export function DescriptionSaveBar({ ticket }: { ticket: string }) {
  const draft = useStore(descriptionDraft);
  if (draft?.ticket !== ticket || !draft.changed) return null;
  return <SaveBarView bar={draft.bar} saving={draft.saving} onSave={draft.save} onDiscard={draft.discard} />;
}
