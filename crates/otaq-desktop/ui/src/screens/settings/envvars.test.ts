// @vitest-environment happy-dom
// The Environment variables row's own logic (T-0300): what is sent when, and Esc that reverts instead of closing.
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import type { SettingsControl } from "../../generated/SettingsControl";
import type { SettingsRow } from "../../generated/SettingsRow";
import type { SettingsValue } from "../../generated/SettingsValue";
import { SettingsContext, type SettingsContextValue } from "./context";
import { EnvVarsRow } from "./EnvVars";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// The shape `settings_page.rs` sends (`env_vars_row`).
function envRow(rows: { name: string; value: string; problem?: string }[]): SettingsRow {
  const control: SettingsControl = {
    kind: "env_vars",
    rows: rows.map((r) => ({
      name: r.name,
      value: r.value,
      problem: r.problem ?? null,
      remove_label: `Remove ${r.name}`,
    })),
    hint_example: "CI = true",
    hint: "e.g. so tools never wait for input or start watch mode",
    empty: "No variables. Sessions get your normal environment.",
    add_label: "Add variable",
    drop_label: "Remove variable",
    applies: "Changes apply to sessions, Runs and Shells started after this.",
    name_placeholder: "NAME",
    value_placeholder: "value",
  };
  return {
    field: "env_vars",
    label: "Environment variables",
    description: "Set in every agent session, Run and Shell of this project.",
    control,
    dimmed: false,
    reset: null,
    button: null,
  };
}

let root: Root;
let box: HTMLElement;
let edits: [string, SettingsValue][];
let closed: number;

function show(row: SettingsRow) {
  const value: SettingsContextValue = {
    page: { page: "project", key: "SHOP", title: "shop-web", intro: "", sections: [] },
    project: "SHOP",
    edit: (field, v) => {
      edits.push([field, v]);
    },
    act: () => undefined,
  };
  act(() => {
    root.render(createElement(SettingsContext.Provider, { value }, createElement(EnvVarsRow, { row })));
  });
}

const inputs = () => [...box.querySelectorAll("input")];

function type(input: HTMLInputElement, text: string) {
  act(() => {
    // The prototype's setter, so React's value tracker sees a change.
    Reflect.set(HTMLInputElement.prototype, "value", text, input);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

function key(target: Element, k: string) {
  act(() => {
    target.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true }));
  });
}

function addButton(): HTMLButtonElement {
  const found = [...box.querySelectorAll("button")].find((b) => b.textContent === "Add variable");
  if (!found) throw new Error("no Add variable");
  return found;
}

beforeEach(() => {
  edits = [];
  closed = 0;
  // Settings' own Esc listener on its box: it closes Settings unless a field took the key.
  box = document.body.appendChild(document.createElement("div"));
  box.addEventListener("keydown", (e) => {
    if (e.key === "Escape") closed += 1;
  });
  root = createRoot(box);
});

afterEach(() => {
  act(() => {
    root.unmount();
  });
  document.body.replaceChildren();
});

describe("the Environment variables row", () => {
  it("shows the empty box, the hint and the applies line", () => {
    show(envRow([]));
    expect(box.textContent).toContain("No variables. Sessions get your normal environment.");
    expect(box.querySelector("code")?.textContent).toBe("CI = true");
    expect(box.textContent).toContain("Changes apply to sessions, Runs and Shells started after this.");
  });

  it("adds a row with NAME focused and saves it with ↵", () => {
    show(envRow([]));
    act(() => {
      addButton().click();
    });
    const [name, value] = inputs();
    if (!name || !value) throw new Error("no new line");
    expect(document.activeElement).toBe(name);
    expect(box.textContent).not.toContain("No variables");
    type(name, "CI");
    type(value, "true");
    key(value, "Enter");
    expect(edits).toEqual([["env_vars.add", { name: "CI", value: "true" }]]);
    expect(inputs()).toHaveLength(0);
    expect(document.activeElement).toBe(addButton());
  });

  it("drops a new line on Esc without closing Settings, and never saves one without a name", () => {
    show(envRow([]));
    act(() => {
      addButton().click();
    });
    const [name] = inputs();
    if (!name) throw new Error("no new line");
    key(name, "Escape");
    expect(inputs()).toHaveLength(0);
    expect(closed).toBe(0);
    act(() => {
      addButton().click();
    });
    const [, value] = inputs();
    if (!value) throw new Error("no new line");
    type(value, "1");
    key(value, "Enter");
    expect(edits).toEqual([]);
  });

  it("saves an edited field on ↵ and puts it back on Esc", () => {
    show(envRow([{ name: "NODE_OPTIONS", value: "--max-old-space-size=4096" }]));
    const [, value] = inputs();
    if (!value) throw new Error("no row");
    type(value, "--max-old-space-size=6144");
    key(value, "Enter");
    expect(edits).toEqual([["env_vars.0.value", "--max-old-space-size=6144"]]);
    edits.length = 0;
    show(envRow([{ name: "NODE_OPTIONS", value: "--max-old-space-size=6144" }]));
    type(value, "--inspect");
    key(value, "Escape");
    expect(value.value).toBe("--max-old-space-size=6144");
    expect(closed).toBe(0);
    // Nothing to put back: Esc closes Settings as usual.
    key(value, "Escape");
    expect(closed).toBe(1);
    expect(edits).toEqual([]);
  });

  it("marks a row with a problem and removes a row with ×", () => {
    show(
      envRow([
        { name: "NODE_OPTIONS", value: "a" },
        { name: "MAX=JOBS", value: "2", problem: "“MAX=JOBS” can’t be a name" },
      ]),
    );
    const [first, , second] = inputs();
    expect(first?.getAttribute("aria-invalid")).toBeNull();
    expect(second?.getAttribute("aria-invalid")).toBe("true");
    expect(box.textContent).toContain("“MAX=JOBS” can’t be a name");
    const remove = box.querySelector<HTMLButtonElement>('button[aria-label="Remove NODE_OPTIONS"]');
    act(() => {
      remove?.click();
    });
    expect(edits).toEqual([["env_vars.remove", 0]]);
  });
});
