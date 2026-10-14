// The window's own title bar (UI-01, d0074; *02 · App shell → Title bar · macOS*, *Title bar · Windows & Linux*): the
// header row is the title bar on every OS, and this screen fills its two ends. On macOS the start keeps room for the
// system's traffic lights, inset in the row; on Windows and Linux the end carries the caption buttons. The ends also
// give the row its outer padding, so the header (T-0219) sits flush between them.
import { useEffect } from "react";

import type { Screen } from "../../app/registry";
import { createStore, type Store, useStore } from "../../app/store";
import type { IconName } from "../../generated/IconName";
import type { WindowAction } from "../../generated/WindowAction";
import type { WindowChrome } from "../../generated/WindowChrome";
import { request, windowAction } from "../../tauri";
import { Icon } from "../../theme/Icon";
import "./title_bar.css";

/** The OS's window controls; `undefined` until the Rust side says (asked once: they never change). */
export const chromeStore: Store<WindowChrome | undefined> = createStore<WindowChrome | undefined>(undefined);

function useChrome(): WindowChrome | undefined {
  const value = useStore(chromeStore);
  useEffect(() => {
    if (chromeStore.get() !== undefined) return;
    request("window.chrome").then(
      (answer) => {
        chromeStore.set(answer as WindowChrome);
      },
      // Without an answer the ends stay empty: the header still works, the OS can still move and close the window.
      () => undefined,
    );
  }, []);
  return value;
}

/** The caption buttons, left to right (40 × 32 hit areas). */
export const CAPTION_BUTTONS: readonly { action: WindowAction; icon: IconName; label: string }[] = [
  { action: "minimize", icon: "minus", label: "Minimize" },
  { action: "toggle_maximize", icon: "square", label: "Maximize" },
  { action: "close", icon: "x", label: "Close" },
];

export function TitleBarStart({ chrome }: { chrome: WindowChrome | undefined }) {
  if (chrome === undefined) return null;
  return <div className={`title-bar-start title-bar-start-${chrome}`} />;
}

export function TitleBarEnd({ chrome }: { chrome: WindowChrome | undefined }) {
  switch (chrome) {
    case undefined:
      return null;
    case "traffic_lights":
      return <div className="title-bar-end" />;
    case "caption_buttons":
      return (
        <div className="title-bar-end">
          <div className="title-bar-divider" />
          <div className="title-bar-captions">
            {CAPTION_BUTTONS.map(({ action, icon, label }) => (
              <button
                key={action}
                type="button"
                className={`title-bar-caption title-bar-caption-${action}`}
                aria-label={label}
                title={label}
                onClick={() => {
                  void windowAction(action);
                }}
              >
                <Icon name={icon} />
              </button>
            ))}
          </div>
        </div>
      );
  }
}

export const titleBar: Screen = (app) => {
  app.slots.register("title_bar_start", {
    Component: function Start() {
      return <TitleBarStart chrome={useChrome()} />;
    },
  });
  app.slots.register("title_bar_end", {
    Component: function End() {
      return <TitleBarEnd chrome={useChrome()} />;
    },
  });
};
