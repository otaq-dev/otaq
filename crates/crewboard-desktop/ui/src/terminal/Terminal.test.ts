import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { createRegistries, RegistriesContext } from "../app/registry";
import { Terminal } from "./Terminal";

// Without a DOM the terminal renders its well only (xterm.js opens in an effect, which the server renderer skips).
describe("the terminal component", () => {
  it("renders its well with the terminal's name, and no find bar, pill or reconnecting strip yet", () => {
    const html = renderToStaticMarkup(
      createElement(
        RegistriesContext,
        { value: createRegistries() },
        createElement(Terminal, { session: "s1", label: "SHOP-19 · agent terminal" }),
      ),
    );
    expect(html).toContain('class="terminal-well" data-phase="attaching"');
    expect(html).toContain('aria-label="SHOP-19 · agent terminal"');
    expect(html).not.toContain("terminal-find");
    expect(html).not.toContain("terminal-jump");
    expect(html).not.toContain("Reconnecting");
  });
});
