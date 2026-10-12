// The page's side of the banner area and the toasts (T-0231). Which banners and toasts show, their wording and their
// buttons come from Rust (`notices.view`, then every change as a `notices` event); this file only routes a click to
// the request or the dialog it stands for, and opens what a notification click names.
import { type OverlayStack, overlays } from "../../app/overlays";
import {
  openTicket,
  requestTerminalFocus,
  type Selection,
  selection,
  selectProject,
  type TerminalFocusRequest,
  terminalFocusRequests,
} from "../../app/selection";
import { createStore, type Store } from "../../app/store";
import type { BannerButtonId } from "../../generated/BannerButtonId";
import type { BannerKind } from "../../generated/BannerKind";
import type { Notices } from "../../generated/Notices";
import type { NotificationTarget } from "../../generated/NotificationTarget";
import type { ResumeAllConfirmation } from "../../generated/ResumeAllConfirmation";
import { asRequestError, type PageEvents, pageEvents } from "../../service";
import { request, windowAction } from "../../tauri";

/** The banner rows and toasts as last heard; `undefined` until the first answer. */
export const noticesStore: Store<Notices | undefined> = createStore<Notices | undefined>(undefined);

/** Whether only Restart works ("Restart required"): screens disable everything else that talks to the service. */
export function onlyRestartAllowed(notices: Notices | undefined): boolean {
  return notices?.only_restart_allowed ?? false;
}

/** Follows the notices: the view now (`notices.view`), then each change. Returns the stop. */
export function watchNotices(
  onChange: (notices: Notices) => void,
  events: PageEvents = pageEvents,
  ask: typeof request = request,
): () => void {
  let stopped = false;
  let changed = false;
  const listening = events.listen((event) => {
    if (event.kind !== "notices" || stopped) return;
    changed = true;
    onChange(event.notices);
  });
  listening.ready
    .then(() => ask("notices.view"))
    .then(
      (notices) => {
        // A change that arrived meanwhile is newer than this answer.
        if (!stopped && !changed) onChange(notices as Notices);
      },
      // The Rust side computes the banners itself; a failed first load only waits for the next change.
      () => undefined,
    );
  return () => {
    stopped = true;
    listening.stop();
  };
}

/** What a banner button does, besides the requests: the dialogs and the inbox. */
export interface BannerRoutes {
  ask: typeof request;
  overlays: Pick<OverlayStack, "open">;
}

const ROUTES: BannerRoutes = { ask: request, overlays };

/** A banner button was pressed. Restarts and resumes only ever start here, on the user's click (d0020, d0022). */
export async function pressBanner(
  kind: BannerKind,
  button: BannerButtonId,
  { ask, overlays: stack }: BannerRoutes = ROUTES,
): Promise<void> {
  // "Review & resume": the inbox, where each item resumes on its own.
  if (button === "secondary") {
    stack.open("inbox");
    return;
  }
  if (button === "resume") {
    await ask("notices.resume_after_limit");
    return;
  }
  switch (kind) {
    case "service_update_ready":
    case "restart_required":
      stack.open("restart_service");
      return;
    // Nothing runs any more, so starting interrupts no one: no confirmation.
    case "service_stopped":
      await ask("notices.restart");
      return;
    case "sessions_interrupted": {
      const confirm = (await ask("notices.resume_all_confirm")) as ResumeAllConfirmation | null;
      if (confirm === null) return;
      stack.open("confirm", {
        title: confirm.title,
        body: confirm.body,
        confirmLabel: confirm.confirm,
        destructive: false,
        onConfirm: () => ask("notices.resume_all"),
      });
      return;
    }
    case "permission_warning":
      await ask("notices.open_docs");
      return;
    case "usage_limit":
      await ask("notices.resume_after_limit");
      return;
    case "reconnecting":
      return;
  }
}

/** Where opening a notification's target lands: the selection, the terminal focus requests and the overlays. */
export interface OpenRoutes {
  selection: Store<Selection>;
  focus: Store<TerminalFocusRequest | null>;
  overlays: Pick<OverlayStack, "open">;
}

const OPEN_ROUTES: OpenRoutes = { selection, focus: terminalFocusRequests, overlays };

/** Opens a ticket on its Terminal tab with the terminal focused, or the inbox for a grouped notification. */
export function openTarget(target: NotificationTarget, routes: OpenRoutes = OPEN_ROUTES): void {
  if (target.ticket === null) {
    routes.overlays.open("inbox");
    return;
  }
  if (target.project !== null) selectProject(target.project, routes.selection);
  openTicket(target.ticket, "terminal", routes.selection);
  requestTerminalFocus(target.ticket, routes.focus);
}

/**
 * Notification clicks (ATT-11): the target the app was started with (`CREWBOARD_OPEN`), once, then every
 * `open_from_notification` event, which also brings the window forward. Returns the stop.
 */
export function followNotifications(
  events: PageEvents = pageEvents,
  ask: typeof request = request,
  open: (target: NotificationTarget) => void = openTarget,
  bringForward: () => Promise<void> = () => windowAction("focus"),
): () => void {
  let stopped = false;
  const listening = events.listen((event) => {
    if (stopped || event.kind !== "service" || event.name !== "open_from_notification") return;
    const key = typeof event.event.key === "string" ? event.event.key : null;
    void bringForward().catch(() => undefined);
    ask("notices.open_target", { key }).then(
      (target) => {
        if (!stopped) open(target as NotificationTarget);
      },
      (error: unknown) => {
        console.warn("can't open the notification's ticket:", asRequestError(error).message);
      },
    );
  });
  listening.ready
    .then(() => ask("notices.open_at_start"))
    .then(
      (target) => {
        if (!stopped && target !== null) open(target as NotificationTarget);
      },
      () => undefined,
    );
  return () => {
    stopped = true;
    listening.stop();
  };
}
