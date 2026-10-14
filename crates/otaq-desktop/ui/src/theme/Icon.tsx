import type { IconName } from "../generated/IconName";
import "./Icon.css";
import { iconUrl } from "./icons";

interface IconProps {
  name: IconName;
  /** What the icon says when nothing next to it does; without it the icon is decoration, hidden from screen readers. */
  label?: string;
  /** Another size in px than the 14 px of *Icons* (11–15 px in chips, menus and banners). */
  size?: number;
}

/** A Lucide icon at 14 px in the current text colour (`color` sets it). */
export function Icon({ name, label, size }: IconProps) {
  const url = iconUrl(name);
  const image = url === undefined ? undefined : `url("${url}")`;
  const style = { maskImage: image, WebkitMaskImage: image, width: size, height: size };
  return label === undefined ? (
    <span className="icon" style={style} aria-hidden />
  ) : (
    <span className="icon" style={style} role="img" aria-label={label} />
  );
}
