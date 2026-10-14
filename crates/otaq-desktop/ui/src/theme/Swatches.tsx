import "./Swatches.css";
import { COLOR_TOKENS, type ThemeKey } from "./generated/tokens";

const ANSI = Array.from({ length: 16 }, (_, n) => `ansi-${String(n)}`);

/**
 * Every colour token and terminal colour of one theme as a named swatch, drawn in that theme whatever the page's is:
 * for checking by hand against *01 · Foundations → Color Tokens* (T-0217's gallery shows it).
 */
export function TokenSwatches({ theme }: { theme: ThemeKey }) {
  return (
    <section className="swatches" data-theme={theme} aria-label={`Colour tokens · ${theme}`}>
      {[...COLOR_TOKENS, ...ANSI].map((name) => (
        <div className="swatch" key={name}>
          <span className="swatch-chip" style={{ background: `var(--${name})` }} />
          <code>{name}</code>
        </div>
      ))}
    </section>
  );
}
