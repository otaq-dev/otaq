// Modal dialogs (*01 · Components → Overlays & feedback → Confirm dialog*): a dimmed backdrop over the whole window
// that takes every press, and a raised box with a title, a body and a footer with the `esc cancel` hint. Focus moves
// in when it opens (to the default button), Tab and ⇧Tab stay inside, Esc cancels, and focus goes back where it was
// when it closes. ↵ presses the default button: confirm on an ordinary dialog, Cancel on a destructive one
// (`dialog.rs`).
import { type ReactNode, type RefObject, useCallback, useEffect, useId, useRef } from "react";

import type { IconName } from "../generated/IconName";
import { Button } from "./Button";
import { Kbd } from "./Kbd";
import { defaultOutcome, dialogKey, type DialogOutcome, trapFocus } from "./logic";

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Keeps Tab inside `root` and turns Esc and ↵ into the dialog's outcomes, while mounted. */
function useModalKeys(
  root: RefObject<HTMLElement | null>,
  destructive: boolean,
  onOutcome: (outcome: DialogOutcome) => void,
): void {
  // Focus goes back where it was when the dialog closes.
  useEffect(() => {
    const before = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    return () => {
      before?.focus();
    };
  }, []);
  useEffect(() => {
    const box = root.current;
    if (!box) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Tab") {
        const items = [...box.querySelectorAll<HTMLElement>(FOCUSABLE)];
        const current = items.indexOf(document.activeElement as HTMLElement);
        items[trapFocus(items.length, current, e.shiftKey)]?.focus();
        e.preventDefault();
        return;
      }
      const target = e.target as HTMLElement | null;
      // ↵ on a button presses that button; in a text area it is a new line.
      if (e.key === "Enter" && (target?.closest("button, textarea") || e.isComposing)) return;
      const outcome = dialogKey(destructive, e.key);
      if (outcome === null) return;
      e.preventDefault();
      // The dialog's own keys: nothing behind it acts on them.
      e.stopPropagation();
      onOutcome(outcome);
    };
    box.addEventListener("keydown", onKey);
    return () => {
      box.removeEventListener("keydown", onKey);
    };
  }, [root, destructive, onOutcome]);
}

export interface DialogProps {
  title: string;
  children: ReactNode;
  /** The footer's buttons, after the `esc` hint. */
  footer: ReactNode;
  /** The word after the `esc` hint: "cancel", or "close" for a dialog that only shows something. */
  hint?: string;
  /** 440 px by default. */
  width?: number;
  /** A destructive dialog: ↵ cancels. */
  destructive?: boolean;
  onConfirm?: () => void;
  onCancel: () => void;
  /** The element focused when it opens; the first focusable one otherwise. */
  initialFocus?: RefObject<HTMLElement | null>;
}

/** The modal shell; `ConfirmDialog` is the common case. */
export function Dialog({
  title,
  children,
  footer,
  hint = "cancel",
  width = 440,
  destructive = false,
  onConfirm,
  onCancel,
  initialFocus,
}: DialogProps) {
  const titleId = useId();
  const box = useRef<HTMLDivElement>(null);
  const onOutcome = useCallback(
    (outcome: DialogOutcome) => {
      if (outcome === "confirm") onConfirm?.();
      else onCancel();
    },
    [onConfirm, onCancel],
  );
  useModalKeys(box, destructive, onOutcome);
  useEffect(() => {
    (initialFocus?.current ?? box.current?.querySelector<HTMLElement>(FOCUSABLE) ?? box.current)?.focus();
  }, [initialFocus]);
  return (
    <div
      className="dialog-backdrop"
      role="presentation"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
    >
      <div
        ref={box}
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        style={{ width }}
      >
        <div className="dialog-content">
          <h2 id={titleId} className="dialog-title">
            {title}
          </h2>
          <div className="dialog-body">{children}</div>
        </div>
        <div className="dialog-footer">
          <span className="dialog-hint">
            <Kbd keys="esc" />
            <span>{hint}</span>
          </span>
          {footer}
        </div>
      </div>
    </div>
  );
}

export interface ConfirmDialogProps {
  title: string;
  /** The body: a paragraph, or anything else the dialog shows. */
  body: ReactNode;
  confirmLabel: string;
  confirmIcon?: IconName;
  /** A destructive confirm button, with Cancel as the default. */
  destructive?: boolean;
  confirmDisabled?: boolean;
  /** A second action in place of the Cancel button ("Start anyway"); Esc and the backdrop still cancel. */
  secondary?: { label: string; onClick: () => void };
  hint?: string;
  width?: number;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Title, body, Cancel and the confirm button; the default button is focused and outlined. */
export function ConfirmDialog({
  title,
  body,
  confirmLabel,
  confirmIcon,
  destructive = false,
  confirmDisabled = false,
  secondary,
  hint,
  width,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const confirm = useRef<HTMLButtonElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const cancelDefault = defaultOutcome(destructive) === "cancel";
  return (
    <Dialog
      title={title}
      hint={hint}
      width={width}
      destructive={destructive}
      onConfirm={() => {
        if (!confirmDisabled) onConfirm();
      }}
      onCancel={onCancel}
      initialFocus={cancelDefault || confirmDisabled ? cancel : confirm}
      footer={
        <>
          {secondary ? (
            <Button ref={cancel} variant="secondary" label={secondary.label} onClick={secondary.onClick} />
          ) : (
            <Button
              ref={cancel}
              variant={cancelDefault ? "secondary" : "ghost"}
              label="Cancel"
              onClick={onCancel}
            />
          )}
          <Button
            ref={confirm}
            variant={destructive ? "destructive" : "primary"}
            label={confirmLabel}
            icon={confirmIcon}
            disabled={confirmDisabled}
            onClick={onConfirm}
          />
        </>
      }
    >
      {typeof body === "string" ? <p className="dialog-text">{body}</p> : body}
    </Dialog>
  );
}
