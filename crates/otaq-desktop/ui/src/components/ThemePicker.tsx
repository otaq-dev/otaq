// The theme picker (UI-11, d0069, d0070; *14 · First run & onboarding → 2 · Theme*, *15 · Settings → Appearance*),
// shared by onboarding (T-0233) and Settings (T-0234): the 15 theme cards under a Dark and a Light heading, each
// preview a small window drawn in that theme's own variables. A click applies and saves the theme at once.
import { useCallback } from "react";

import type { ActiveTheme } from "../generated/ActiveTheme";
import type { ThemeParams } from "../generated/ThemeParams";
import type { ThemePick } from "../generated/ThemePick";
import type { ThemePickerCard } from "../generated/ThemePickerCard";
import type { ThemePickerSection } from "../generated/ThemePickerSection";
import { useServiceView, type ViewState } from "../service";
import { request } from "../tauri";
import { Icon } from "../theme/Icon";
import { osIsDark } from "../theme/theme";

/** Stores `key` as the theme (`theme.pick`) and recolors this page at once; every window follows on the service's
 * `settings_changed`. */
export async function pickTheme(key: string, root: Pick<HTMLElement, "dataset"> = document.documentElement) {
  root.dataset.theme = key;
  const params: ThemePick = { key };
  return (await request("theme.pick", params)) as ActiveTheme;
}

/** The picker's sections with the theme in use checked, refetched on every settings change. */
export function useThemePicker(): ViewState<ThemePickerSection[]> {
  const load = useCallback(() => {
    const params: ThemeParams = { os_dark: osIsDark() };
    return request("theme.picker", params) as Promise<ThemePickerSection[]>;
  }, []);
  return useServiceView(load, ["settings_changed"]);
}

/** A small window in the card's theme: title bar, sidebar and three columns of cards. */
function Preview({ theme }: { theme: string }) {
  const column = (statuses: readonly string[]) => (
    <span className="preview-column">
      <span className="preview-heading" />
      {statuses.map((s, i) => (
        <span key={i} className="preview-card">
          <span className="preview-dot" style={{ background: `var(--st-${s})` }} />
          <span className="preview-line" />
        </span>
      ))}
    </span>
  );
  return (
    <span className="preview" data-theme={theme} aria-hidden>
      <span className="preview-bar">
        <span className="preview-light" data-light="error" />
        <span className="preview-light" data-light="paused" />
        <span className="preview-light" data-light="approved" />
        <span className="preview-fill" />
        <span className="preview-stub" style={{ width: 18 }} />
      </span>
      <span className="preview-main">
        <span className="preview-side">
          <span className="preview-accent" />
          <span className="preview-stub" style={{ width: 12 }} />
          <span className="preview-stub" style={{ width: 14 }} />
          <span className="preview-stub" style={{ width: 10 }} />
        </span>
        <span className="preview-columns">
          {column(["working", "investigate"])}
          {column(["paused", "error"])}
          {column(["approved"])}
        </span>
      </span>
    </span>
  );
}

function Card({ card, onPick }: { card: ThemePickerCard; onPick: (key: string) => void }) {
  return (
    <button
      type="button"
      role="radio"
      className="theme-card"
      aria-checked={card.selected}
      onClick={() => {
        onPick(card.id);
      }}
    >
      <span className="theme-card-ring">
        <Preview theme={card.id} />
      </span>
      <span className="theme-card-text">
        <span className="theme-card-name">
          <span>{card.name}</span>
          {card.selected && <Icon name="check" size={12} />}
        </span>
        <span className="theme-card-tagline">{card.tagline}</span>
      </span>
    </button>
  );
}

export interface ThemePickerProps {
  sections: readonly ThemePickerSection[];
  onPick: (key: string) => void;
  /** Under Settings' row "Theme", which draws the line above; a line closes the picker. */
  inRows?: boolean;
}

export function ThemePicker({ sections, onPick, inRows = false }: ThemePickerProps) {
  return (
    <div className="theme-picker" data-in-rows={inRows || undefined}>
      {sections.map((section) => (
        <div key={section.label} className="theme-section" role="radiogroup" aria-label={section.label}>
          <span className="theme-section-label">{section.label}</span>
          {section.rows.map((row, r) => (
            <div key={r} className="theme-row">
              {row.map((card) => (
                <Card key={card.id} card={card} onPick={onPick} />
              ))}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
