// @vitest-environment happy-dom
// An option's note on the card (ATT-17, d0061; T-0375): the compact list stays as it is; hovering an option, or
// focusing it with Tab, shows its note whole in the light hover card. No note, no hover card. The hover card never
// takes focus and never picks; a click still sends the option's own action.
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { TOOLTIP_DELAY } from "../components/Tooltip";
import type { CardActionKind } from "../generated/CardActionKind";
import type { CardOption } from "../generated/CardOption";
import type { TicketCardView } from "../generated/TicketCardView";
import { TicketCard } from ".";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const LONG = "Pros and cons, ".repeat(40).trim();

const option = (number: number, label: string, note: string | null, rec = false): CardOption => ({
  number,
  label,
  recommended: rec ? "Rec." : null,
  picked: rec,
  other: label === "Other…",
  action: { pick_option: number },
  note,
});

// NOTE-1's question as `card_page.rs` serves it.
const NOTE_1: TicketCardView = {
  key: "NOTE-1",
  key_label: "NOTE-1",
  avatar: null,
  pinned: false,
  priority: null,
  flag: { palette: "attn", icon: "message-circle-question-mark", label: "Asks you" },
  title: "Add export",
  situation: null,
  origin: null,
  options: [
    option(1, "Markdown files", "Opens in any editor; one file per note, no attachments.", true),
    option(2, "One ZIP", LONG),
    option(3, "Plain text", null),
    option(4, "Other…", null),
  ],
  footer: null,
  hover: { status: "Asks you", details: [] },
  sleeve: null,
  outline: "attention",
  faded: null,
  fresh: false,
  open_panel: "Open panel",
  open_panel_keys: "↵",
  open_terminal: "Open terminal",
};

let host: HTMLDivElement;
let root: Root;
let actions: CardActionKind[];

beforeEach(() => {
  vi.useFakeTimers();
  actions = [];
  host = document.body.appendChild(document.createElement("div"));
  root = createRoot(host);
  act(() => {
    root.render(
      createElement(TicketCard, {
        card: NOTE_1,
        onAction: (kind) => {
          actions.push(kind);
        },
      }),
    );
  });
});

afterEach(() => {
  act(() => {
    root.unmount();
  });
  host.remove();
  vi.useRealTimers();
});

/** The option buttons 1–4, in order. */
function buttons(): [HTMLButtonElement, HTMLButtonElement, HTMLButtonElement, HTMLButtonElement] {
  const [a, b, c, d] = host.querySelectorAll<HTMLButtonElement>(".card-option");
  if (!a || !b || !c || !d) throw new Error("four options");
  return [a, b, c, d];
}
const bubble = () => host.querySelector<HTMLElement>(".card-hover");

function hover(button: HTMLButtonElement) {
  act(() => {
    button.dispatchEvent(new MouseEvent("mouseover", { bubbles: true, relatedTarget: document.body }));
  });
  act(() => {
    vi.advanceTimersByTime(TOOLTIP_DELAY);
  });
}

function leave(button: HTMLButtonElement) {
  act(() => {
    button.dispatchEvent(new MouseEvent("mouseout", { bubbles: true, relatedTarget: document.body }));
  });
}

describe("an option's note on the card", () => {
  it("the compact list stays as it is: numbers, labels, Rec., no note drawn in the card", () => {
    const html = renderToStaticMarkup(createElement(TicketCard, { card: NOTE_1 }));
    expect(html.match(/class="card-option"/g)).toHaveLength(4);
    expect(html).toContain("Rec.");
    expect(html).not.toContain("card-hover-text");
    // The note is each option's description, kept out of sight in the card.
    const described = [...html.matchAll(/aria-describedby="([^"]+)"/g)].map((m) => m[1]);
    expect(described).toHaveLength(2);
    expect(html).toContain(`id="${described[1] ?? ""}" hidden="">${LONG}</span>`);
  });

  it("hovering an option shows its note whole in the hover card; leaving hides it", () => {
    const [, zip] = buttons();
    hover(zip);
    expect(bubble()?.getAttribute("role")).toBe("tooltip");
    expect(bubble()?.querySelector(".card-hover-text")?.textContent).toBe(LONG);
    leave(zip);
    expect(bubble()).toBeNull();
  });

  it("an option without a note shows no hover card", () => {
    const [, , plain, other] = buttons();
    hover(plain);
    expect(bubble()).toBeNull();
    hover(other);
    expect(bubble()).toBeNull();
  });

  it("Tab focus shows the note; the hover card takes no focus and picks nothing", () => {
    const [markdown, zip] = buttons();
    act(() => {
      markdown.focus();
    });
    act(() => {
      vi.runOnlyPendingTimers();
    });
    expect(document.activeElement).toBe(markdown);
    expect(bubble()?.textContent).toBe("Opens in any editor; one file per note, no attachments.");
    act(() => {
      zip.focus();
    });
    act(() => {
      vi.runOnlyPendingTimers();
    });
    expect(document.activeElement).toBe(zip);
    expect(bubble()?.textContent).toBe(LONG);
    expect(actions).toEqual([]);
    expect(host.querySelectorAll("[data-picked]")).toHaveLength(1);
    expect(buttons()[0].hasAttribute("data-picked")).toBe(true);
  });

  it("a click still sends the option's own action", () => {
    const [, zip] = buttons();
    hover(zip);
    act(() => {
      zip.click();
    });
    expect(actions).toEqual([{ pick_option: 2 }]);
  });
});
