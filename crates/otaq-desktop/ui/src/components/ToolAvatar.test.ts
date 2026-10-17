import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { ToolAvatarView } from "../generated/ToolAvatarView";
import { COLOR_TOKENS } from "../theme/generated/tokens";
import { ToolAvatar } from "./ToolAvatar";

const CX: ToolAvatarView = {
  tool: "codex",
  letters: "CX",
  background: "tool-cx",
  foreground: "tool-cx-fg",
  hover: "Codex · gpt-5.4",
};

describe("the tool avatar", () => {
  it("draws the letters on the tool's two theme colours, with its hover as the label", () => {
    const out = renderToStaticMarkup(createElement(ToolAvatar, { avatar: CX }));
    expect(out).toContain(">CX</span>");
    expect(out).toContain("background:var(--tool-cx)");
    expect(out).toContain("color:var(--tool-cx-fg)");
    expect(out).toContain('aria-label="Codex · gpt-5.4"');
    expect(out).toContain('data-tool="codex"');
  });

  it("uses tokens every theme defines", () => {
    for (const letters of ["cc", "cx", "cp", "oc"]) {
      expect(COLOR_TOKENS).toContain(`tool-${letters}`);
      expect(COLOR_TOKENS).toContain(`tool-${letters}-fg`);
    }
  });
});
