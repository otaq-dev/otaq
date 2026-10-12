// Banner (*01 · Components → Banners → Banner*): a full-width 36 px strip with an icon, a title in the banner's
// colour, the text, optional compact buttons and an optional close button. Which banners show, and their wording, come
// from Rust (`banners.rs`, T-0231).
import type { IconName } from "../generated/IconName";
import { Icon } from "../theme/Icon";
import { Button } from "./Button";

/** Orange: the usage limit; amber: warnings; accent: calm news; red: broke and waits; grey: reconnecting. */
export type BannerTone = "attention" | "warning" | "info" | "error" | "neutral";

export interface BannerAction {
  label: string;
  icon?: IconName;
  disabled?: boolean;
  /** The banner's call to action, highlighted (Resume all after the limit reset). */
  primary?: boolean;
  onClick: () => void;
}

export interface BannerProps {
  tone: BannerTone;
  icon: IconName;
  title: string;
  text: string;
  actions?: readonly BannerAction[];
  /** Shows the close button; without it the banner can't be dismissed. */
  onDismiss?: () => void;
}

export function Banner({ tone, icon, title, text, actions = [], onDismiss }: BannerProps) {
  return (
    <div className="banner" data-tone={tone} role={tone === "error" ? "alert" : "status"}>
      <span className="banner-icon">
        <Icon name={icon} size={15} />
      </span>
      <span className="banner-title">{title}</span>
      <span className="banner-text">{text}</span>
      {actions.map((a) => (
        <Button
          key={a.label}
          compact
          variant={a.primary ? "primary" : "secondary"}
          label={a.label}
          icon={a.icon}
          disabled={a.disabled}
          onClick={a.onClick}
        />
      ))}
      {onDismiss && (
        <button type="button" className="banner-close" aria-label="Dismiss" onClick={onDismiss}>
          <Icon name="x" />
        </button>
      )}
    </div>
  );
}
