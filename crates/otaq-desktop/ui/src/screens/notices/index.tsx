// Area 11 · Banners, toasts & OS notifications (T-0231): the banner area above board and panel (at most two rows,
// one per situation), the toast host bottom-right, Restart now's confirmation, and notification clicks. What shows,
// its wording and its buttons come from Rust (`src/notices.rs`); OS notifications are posted by the service.
import { useEffect, useState } from "react";

import type { OverlayProps, Screen } from "../../app/registry";
import { openTicket } from "../../app/selection";
import { useStore } from "../../app/store";
import { Banner, ConfirmDialog, Toast } from "../../components";
import type { Banner as BannerView } from "../../generated/Banner";
import type { RestartDialog } from "../../generated/RestartDialog";
import type { Toast as ToastView } from "../../generated/Toast";
import type { ToastPress } from "../../generated/ToastPress";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { toolProblemAction } from "../panel/toolProblemRunner";
import { type LimitLineHandlers, LimitLines } from "./LimitLines";
import {
  followNotifications,
  limitLinesOf,
  moveWaiting,
  noticesStore,
  pressBanner,
  watchNotices,
} from "./notices";
import "./notices.css";

export { noticesStore, onlyRestartAllowed } from "./notices";

/** On the root element while the service can't take input. */
const INPUT_OFF = "data-service-input-off";

function warnOnFailure(what: string) {
  return (error: unknown) => {
    console.error(`${what} failed`, error);
  };
}

/** The limit lines' presses in the app: a line's button as its tool problem action, an agent picked in Move waiting to…. */
export const LINE_HANDLERS: LimitLineHandlers = {
  onAction: (action) => {
    toolProblemAction(action, {});
  },
  onMove: (tool, agent) => {
    moveWaiting(tool, agent).catch(warnOnFailure("Move waiting"));
  },
};

/** The banner rows, top first; the usage limit with several tools as one line per tool. */
export function BannerRows({
  banners,
  lineHandlers = LINE_HANDLERS,
}: {
  banners: readonly BannerView[];
  lineHandlers?: LimitLineHandlers;
}) {
  if (banners.length === 0) return null;
  return (
    <div className="notices-banners">
      {banners.map((b) => {
        const lines = limitLinesOf(b);
        if (lines.length > 0) return <LimitLines key={b.kind} lines={lines} {...lineHandlers} />;
        return (
          <Banner
            key={b.kind}
            tone={b.tone}
            icon={b.icon}
            title={b.title}
            text={b.text}
            tooltip={b.tooltip ?? undefined}
            actions={b.buttons.map((button) => ({
              label: button.label,
              icon: button.icon,
              disabled: !button.enabled,
              primary: button.primary,
              onClick: () => {
                pressBanner(b.kind, button.id).catch(warnOnFailure(button.label));
              },
            }))}
            onDismiss={
              b.dismissable
                ? () => {
                    request("notices.dismiss_banner", { kind: b.kind }).catch(warnOnFailure("Dismiss"));
                  }
                : undefined
            }
          />
        );
      })}
    </div>
  );
}

/** The banner slot: follows the notices and the notification clicks while the page is up. */
function BannerArea() {
  const notices = useStore(noticesStore);
  useEffect(() => watchNotices(noticesStore.set), []);
  useEffect(() => followNotifications(), []);
  // While the service can't take input, the rail, board and panel are dimmed and ignore clicks (`notices.css`).
  const inputOff = notices !== undefined && !notices.input_enabled;
  useEffect(() => {
    const root = document.documentElement;
    root.toggleAttribute(INPUT_OFF, inputOff);
    return () => {
      root.removeAttribute(INPUT_OFF);
    };
  }, [inputOff]);
  return <BannerRows banners={notices?.banners ?? []} />;
}

function linkOf(toast: ToastView) {
  const link = toast.link;
  if (link === null) return undefined;
  switch (link.kind) {
    case "copy":
      return {
        label: link.label,
        onClick: () => {
          navigator.clipboard.writeText(link.text).catch(warnOnFailure(link.label));
        },
      };
    case "open":
      return {
        label: link.label,
        onClick: () => {
          openTicket(link.ticket, link.tab ?? undefined);
        },
      };
    case "actions":
      return undefined;
  }
}

/** The toast's buttons ("Undo", "Turn on for epic children"): a press goes to Rust, which replaces the toast. */
export function actionsOf(toast: ToastView) {
  const link = toast.link;
  if (link?.kind !== "actions") return undefined;
  return link.actions.map((button) => ({
    label: button.label,
    onClick: () => {
      const press: ToastPress = { id: toast.id, label: button.label, action: button.action };
      request("notices.toast_action", press).catch(warnOnFailure(button.label));
    },
  }));
}

/** The toasts, oldest at the top; hovering one pauses its time-out, clicking a ticket's opens the ticket. */
export function ToastStack({ toasts }: { toasts: readonly ToastView[] }) {
  if (toasts.length === 0) return null;
  return (
    <div className="notices-toasts">
      {toasts.map((t) => {
        const ticket = t.ticket;
        return (
          // eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions -- hovering only pauses the time-out; the toast's own buttons stay the interactive parts
          <div
            key={t.id}
            className="notices-toast"
            role="group"
            aria-label={t.title}
            onMouseEnter={() => {
              request("notices.hold_toast", { id: t.id, held: true }).catch(warnOnFailure("Hold"));
            }}
            onMouseLeave={() => {
              request("notices.hold_toast", { id: t.id, held: false }).catch(warnOnFailure("Hold"));
            }}
          >
            <Toast
              kind={t.level}
              title={t.title}
              text={t.text}
              icon={t.icon ?? undefined}
              iconTone={t.icon_tone ?? undefined}
              action={linkOf(t)}
              actions={actionsOf(t)}
              onOpen={
                ticket === null
                  ? undefined
                  : () => {
                      openTicket(ticket);
                    }
              }
              onClose={() => {
                request("notices.dismiss_toast", { id: t.id }).catch(warnOnFailure("Close"));
              }}
            />
          </div>
        );
      })}
    </div>
  );
}

function ToastHost() {
  const notices = useStore(noticesStore);
  return <ToastStack toasts={notices?.toasts ?? []} />;
}

/** The dialog's body: who gets interrupted, in the warning callout, then what else happens. */
export function RestartBody({ dialog }: { dialog: RestartDialog }) {
  return (
    <div className="notices-restart">
      <p className="notices-restart-message">
        <Icon name="activity" size={14} />
        <span>{dialog.message}</span>
      </p>
      <p className="notices-restart-detail">{dialog.detail}</p>
    </div>
  );
}

/** *11 → Service restart · confirm*: only its confirm button restarts (d0022). */
function RestartOverlay({ close }: OverlayProps<"restart_service">) {
  const [dialog, setDialog] = useState<RestartDialog>();
  useEffect(() => {
    let current = true;
    request("notices.restart_dialog").then((answer) => {
      if (current) setDialog(answer as RestartDialog);
    }, warnOnFailure("The restart dialog"));
    return () => {
      current = false;
    };
  }, []);
  if (dialog === undefined) return null;
  return (
    <ConfirmDialog
      title={dialog.title}
      body={<RestartBody dialog={dialog} />}
      confirmLabel={dialog.confirm}
      confirmIcon="rotate-cw"
      width={480}
      onConfirm={() => {
        close();
        request("notices.restart").catch(warnOnFailure("Restart"));
      }}
      onCancel={close}
    />
  );
}

export const notices: Screen = (app) => {
  app.slots.register("banners", { Component: BannerArea });
  app.slots.register("toasts", { Component: ToastHost });
  app.overlays.register("restart_service", { Component: RestartOverlay });
};
