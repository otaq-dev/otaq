// Every overlay of the page (dialogs, popovers, drawers, the inbox), fixed here so screens open one another's by id
// (`openOverlay("inbox")`) without editing a shared list. Each overlay's own screen registers its component; opening
// one that isn't registered yet does nothing visible. Menus and tooltips tied to one element stay local to their
// component (they may portal into the overlay host, `OVERLAY_HOST_ID`).
import type { ComponentType } from "react";

import type { ConfirmRow } from "../../generated/ConfirmRow";
import type { IconName } from "../../generated/IconName";
import type { OnboardingStepId } from "../../generated/OnboardingStepId";
import type { GlobalSection, ProjectSection } from "./settings";

/** A confirmation's texts, from the view models, and what its confirm button does. */
export interface Confirmation {
  title: string;
  body: string;
  /** Lines under the body, each with its icon (Park: what is kept). */
  rows?: readonly ConfirmRow[];
  confirmLabel: string;
  confirmIcon?: IconName;
  /** A destructive confirm button, with Cancel as the default (`dialog.rs`). */
  destructive: boolean;
  onConfirm: () => unknown;
}

/** Each overlay id with what it is opened with (`undefined`: nothing). */
export interface OverlayParams {
  /** The needs-you inbox (T-0230). */
  inbox: undefined;
  /** The quick switcher, ⌘P (T-0219). */
  quick_switcher: undefined;
  /** The ticket editor on a new Backlog ticket, in an epic if given (T-0228). */
  new_ticket: { epic?: string } | undefined;
  /** The ticket editor on a ticket (T-0228). */
  edit_ticket: { ticket: string };
  /** Settings, on a project's page and on a section if given (T-0234). */
  settings: { project?: string; section?: GlobalSection | ProjectSection } | undefined;
  /** The running processes (T-0232). */
  processes: undefined;
  /** The run-command dialog of a project's first Run (T-0232). */
  run_command: { project: string; ticket?: string };
  /** Recovery and Cleanup (T-0235). */
  cleanup: undefined;
  /** The project's Terminals & agents drawer (T-0242). */
  terminals_agents: undefined;
  /** The after-restart summary (T-0235, T-0244). */
  after_restart: undefined;
  /** The limits popover of the header (T-0219). */
  limits: undefined;
  /** Add project (T-0233), with a folder already picked or dropped on the window. */
  add_project: { folder?: string } | undefined;
  /** First run (T-0233), over the whole window; Check tools opens it on its tools step. */
  onboarding: { step?: OnboardingStepId } | undefined;
  /** The delete confirmation of a never-started ticket (T-0221). */
  delete_ticket: { ticket: string };
  /** The Close ticket confirmation (T-0222). */
  close_ticket: { ticket: string };
  /** Send back with a note (T-0226). */
  send_back: { ticket: string };
  /** The Local merge confirmation (T-0226). */
  local_merge: { ticket: string };
  /** The Open PR dialog (T-0227). */
  open_pr: { ticket: string };
  /** Any other confirmation (Park, Reopen, Resume all, Restart now, Remove project…), drawn by T-0217's dialog. */
  confirm: Confirmation;
  /** Restart now's confirmation, naming who gets interrupted (T-0231). */
  restart_service: undefined;
  /** The developer-only design gallery, over the whole window (T-0217, `CREWBOARD_GALLERY=1`). */
  gallery: undefined;
}

export type OverlayId = keyof OverlayParams;

/**
 * Whether the overlay is modal: while one is open the app's shortcuts don't run (`modal.rs`, T-0215's dispatcher
 * asks `modalOpen`).
 */
export const OVERLAY_MODAL: Record<OverlayId, boolean> = {
  inbox: false,
  quick_switcher: false,
  new_ticket: true,
  edit_ticket: true,
  settings: true,
  processes: false,
  run_command: true,
  cleanup: true,
  terminals_agents: false,
  after_restart: true,
  limits: false,
  add_project: true,
  onboarding: true,
  delete_ticket: true,
  close_ticket: true,
  send_back: true,
  local_merge: true,
  open_pr: true,
  confirm: true,
  restart_service: true,
  gallery: true,
};

export const OVERLAY_IDS = Object.keys(OVERLAY_MODAL) as OverlayId[];

/** What an overlay's component gets: what it was opened with, and how to close itself. */
export interface OverlayProps<K extends OverlayId> {
  params: OverlayParams[K];
  close: () => void;
}

export type OverlayEntries = { [K in OverlayId]: { Component: ComponentType<OverlayProps<K>> } };
