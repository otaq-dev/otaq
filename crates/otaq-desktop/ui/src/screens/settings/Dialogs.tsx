// The dialogs Settings opens: Remove project (*15 → Remove project · confirm*, PRJ-04; destructive, ↵ = Cancel),
// Theme credits (*15 → Theme credits · dialog*, UI-11, d0069) and turning the warm build switch off (*15 → Warm
// build · turning off · confirm (0082)*, WRM-05; destructive, ↵ = Cancel). Their texts come from `settings_page.rs`.
import { useState } from "react";

import { Button, ConfirmDialog, Dialog } from "../../components";
import type { CleanupWarmConfirm } from "../../generated/CleanupWarmConfirm";
import type { RemoveProjectDialog } from "../../generated/RemoveProjectDialog";
import type { ThemeCreditsDialog } from "../../generated/ThemeCreditsDialog";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";

function List({ heading, items, kept }: { heading: string; items: readonly string[]; kept: boolean }) {
  return (
    <div className="settings-remove-list" data-kept={kept || undefined}>
      <strong>{heading}</strong>
      {items.map((item) => (
        <span key={item} className="settings-remove-item">
          <Icon name={kept ? "check" : "x"} size={12} />
          <span>{item}</span>
        </span>
      ))}
    </div>
  );
}

export function RemoveDialog({
  dialog,
  onConfirm,
  onCancel,
}: {
  dialog: RemoveProjectDialog;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <ConfirmDialog
      title={dialog.title}
      destructive
      confirmLabel={dialog.confirm}
      confirmIcon="trash-2"
      width={480}
      onConfirm={onConfirm}
      onCancel={onCancel}
      body={
        <div className="settings-remove">
          <List heading={dialog.deleted_heading} items={dialog.deleted} kept={false} />
          <List heading={dialog.kept_heading} items={dialog.kept} kept />
          {dialog.agents !== null && (
            <p className="settings-remove-agents">
              <Icon name="triangle-alert" size={13} />
              <span>{dialog.agents}</span>
            </p>
          )}
        </div>
      }
    />
  );
}

/** Turning the warm build switch off: what it deletes, "up to" its size, and that clones share the space. */
export function WarmOffDialog({
  confirm,
  onConfirm,
  onCancel,
}: {
  confirm: CleanupWarmConfirm;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <ConfirmDialog
      title={confirm.title}
      destructive
      confirmLabel={confirm.confirm}
      confirmIcon="trash-2"
      width={480}
      onConfirm={onConfirm}
      onCancel={onCancel}
      body={
        <div className="settings-warm-confirm">
          <p className="dialog-text">{confirm.text}</p>
          <p className="dialog-text">{confirm.shared}</p>
          <p className="settings-warm-confirm-line">
            <Icon name={confirm.line.icon} size={13} />
            <span>{confirm.line.text}</span>
          </p>
        </div>
      }
    />
  );
}

export function CreditsDialog({ dialog, onClose }: { dialog: ThemeCreditsDialog; onClose: () => void }) {
  // As the frame shows it: the MIT text unfolded.
  const [mitOpen, setMitOpen] = useState(true);
  const [copied, setCopied] = useState(false);
  return (
    <Dialog
      title={dialog.title}
      hint="close"
      width={600}
      onConfirm={onClose}
      onCancel={onClose}
      footer={
        <>
          <Button
            variant="secondary"
            label={copied ? dialog.copied_label : dialog.copy_label}
            icon={copied ? "check" : "copy"}
            onClick={() => {
              void navigator.clipboard.writeText(dialog.notices).then(() => {
                setCopied(true);
              });
            }}
          />
          <Button label={dialog.done_label} onClick={onClose} />
        </>
      }
    >
      <div className="settings-credits">
        <p className="settings-credits-description">{dialog.description}</p>
        <div className="settings-credits-list">
          {dialog.credits.map((credit) => (
            <div key={credit.project} className="settings-credit">
              <div className="settings-credit-text">
                <span>
                  <strong>{credit.project}</strong>{" "}
                  <span className="settings-credit-themes">{credit.themes}</span>
                </span>
                <span className="settings-credit-copyright">{credit.copyright}</span>
              </div>
              <span className="settings-credit-license">{credit.license}</span>
              <button
                type="button"
                className="settings-link"
                onClick={() => {
                  void request("settings.open_source", { url: credit.source });
                }}
              >
                {dialog.source_label}
                <Icon name="arrow-up-right" size={12} />
              </button>
            </div>
          ))}
        </div>
        <button
          type="button"
          className="settings-mit-toggle"
          aria-expanded={mitOpen}
          onClick={() => {
            setMitOpen((open) => !open);
          }}
        >
          <Icon name={mitOpen ? "chevron-down" : "chevron-right"} />
          <strong>{dialog.mit_title}</strong>
          <span>{dialog.mit_note}</span>
        </button>
        {mitOpen && (
          <div className="settings-mit-text">
            {dialog.mit_text.map((paragraph) => (
              <p key={paragraph}>{paragraph}</p>
            ))}
          </div>
        )}
      </div>
    </Dialog>
  );
}
