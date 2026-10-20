// The gallery's limit banner per tool and Activity with four tools (T-0368; *11* `DcZCp` / `hS30M`, `saXdl` / `x8C2T`,
// `UI9hR` / `WFdMY`; *02* `D7lOb` / `R0GSC`), in Otaq Light and Dark: every sample of `gallery.limit_lines` drawn as the
// banner draws it, and the Activity popovers with a tool at its limit and with every tool.
// Presses are only recorded; nothing is sent.
import { useCallback, useState } from "react";

import { Banner } from "../../components";
import type { ActivityGallery } from "../../generated/ActivityGallery";
import type { LimitLine } from "../../generated/LimitLine";
import type { LimitLineGallery } from "../../generated/LimitLineGallery";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { ActivityBody } from "../shell/ActivityPopover";
import { type LimitLineHandlers, LimitLines } from "./LimitLines";
import { limitLinesOf } from "./notices";

const THEMES = ["otaq-light", "otaq-dark"] as const;
const themed = { padding: 16, background: "var(--bg-app)", color: "var(--text-primary)" };
/** The Activity popovers this section shows of `gallery.activity`'s. */
const POPOVERS = ["A tool at its limit", "Every tool"];
const ignore = () => undefined;

interface Views {
  lines: LimitLineGallery;
  activity: ActivityGallery;
}

/** A sample as the banner draws it: its lines, or with one tool connected the one-tool banner (d0086). */
function CellBanner({ lines, handlers }: { lines: LimitLine[]; handlers: LimitLineHandlers }) {
  const drawn = limitLinesOf({
    kind: "usage_limit",
    tone: "neutral",
    icon: "gauge",
    title: "",
    text: "",
    buttons: [],
    dismissable: false,
    tooltip: null,
    lines,
    tickets: [],
  });
  if (drawn.length > 0) return <LimitLines lines={drawn} {...handlers} />;
  const line = lines[0];
  if (line === undefined) return null;
  return (
    <Banner
      tone={line.tone}
      icon={line.icon}
      title={line.title}
      text={line.text}
      actions={line.buttons.map((button) => ({
        label: button.label,
        icon: button.icon,
        disabled: !button.enabled,
        primary: button.primary,
        onClick: () => {
          handlers.onAction(button.action);
        },
      }))}
    />
  );
}

export function LimitLinesGallery() {
  const load = useCallback(
    () =>
      Promise.all([request("gallery.limit_lines"), request("gallery.activity")]).then(
        ([lines, activity]) => ({ lines, activity }) as Views,
      ),
    [],
  );
  const { view, error } = useServiceView(load, []);
  const [picked, setPicked] = useState<string | null>(null);
  if (error) return <p role="alert">{error.message}</p>;
  if (!view) return null;
  const handlers: LimitLineHandlers = {
    onAction: (action) => {
      setPicked("tool" in action ? `${action.kind} · ${action.tool}` : action.kind);
    },
  };
  const popovers = view.activity.popovers.filter(([caption]) => POPOVERS.includes(caption));
  return (
    <div className="gallery-stack">
      <span className="gallery-caption">Last call: {picked ?? "—"}</span>
      {THEMES.map((theme) => (
        <div key={theme} data-theme={theme} className="gallery-stack" style={themed}>
          <span className="gallery-caption">{theme.toUpperCase()}</span>
          {view.lines.cells.map((cell) => (
            <div key={cell.name} className="gallery-stack-tight">
              <span className="gallery-caption">{cell.name.toUpperCase()}</span>
              <CellBanner lines={cell.lines} handlers={handlers} />
            </div>
          ))}
          <div className="gallery-wrap gallery-wrap-wide activity-gallery-popovers">
            {popovers.map(([caption, popover]) => (
              <figure key={caption} className="gallery-stack-tight">
                <figcaption className="gallery-caption">ACTIVITY · {caption.toUpperCase()}</figcaption>
                <div className="popover activity-gallery-popover">
                  <ActivityBody
                    view={popover}
                    onStep={ignore}
                    onOpenTicket={ignore}
                    onOpenTerminals={ignore}
                  />
                </div>
              </figure>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
