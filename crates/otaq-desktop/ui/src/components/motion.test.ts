import { describe, expect, it } from "vitest";

import { cssTimeMs, playLeave, playMotion } from "./motion";

describe("motion", () => {
  it("reads the motion tokens' CSS times in milliseconds", () => {
    expect(cssTimeMs("120ms")).toBe(120);
    expect(cssTimeMs(" 0.2s ")).toBe(200);
    expect(cssTimeMs("")).toBe(0);
    expect(cssTimeMs("fast")).toBe(0);
  });

  it("plays nothing where the page can't animate, and a leave then ends at once", () => {
    const el = {} as HTMLElement;
    expect(playMotion(el, "panel", "in")).toBeNull();
    let done = 0;
    playLeave(el, "panel", () => {
      done += 1;
    });
    playLeave(null, "panel", () => {
      done += 1;
    });
    expect(done).toBe(2);
  });
});
