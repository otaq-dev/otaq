// The bundled Lucide icons (ISC), the same SVG files and names as the Rust side's `icons.rs`: Vite bundles them from
// `otaq-view`, so Rust and the page name one set and nothing loads from the network.
import type { IconName } from "../generated/IconName";

const FOLDER = "../../../../otaq-view/assets/icons/";

const urls = import.meta.glob<string>("../../../../otaq-view/assets/icons/*.svg", {
  query: "?url",
  import: "default",
  eager: true,
});

/** The bundled file of an icon, as a URL for CSS (`mask-image`). */
export function iconUrl(name: IconName): string | undefined {
  return urls[`${FOLDER}${name}.svg`];
}

/** How many icons are bundled. */
export const ICON_COUNT = Object.keys(urls).length;
