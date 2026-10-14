import { describe, expect, it } from "vitest";

import type { KeyModifiers } from "./generated/KeyModifiers";
import type { TerminalPhase } from "./generated/TerminalPhase";

// The generated types are what the Rust side sends as JSON (serde's names), so the page can use them as they are.
// The `switch` below has no `default`: a variant added in Rust and not handled here fails ESLint's
// switch-exhaustiveness-check once the types are regenerated.
function inputAllowed(phase: TerminalPhase): boolean {
  switch (phase) {
    case "live":
      return true;
    case "no_session":
    case "attaching":
    case "reconnecting":
    case "ended":
      return false;
  }
}

describe("the generated types", () => {
  it("read the Rust side's JSON as it is sent", () => {
    const phases = JSON.parse(
      '["no_session", "attaching", "live", "reconnecting", "ended"]',
    ) as TerminalPhase[];
    expect(phases.filter(inputAllowed)).toEqual(["live"]);

    const modifiers: KeyModifiers = {
      control: true,
      alt: false,
      shift: true,
      platform: false,
      function: false,
    };
    expect(JSON.parse(JSON.stringify(modifiers))).toEqual(modifiers);
  });
});
