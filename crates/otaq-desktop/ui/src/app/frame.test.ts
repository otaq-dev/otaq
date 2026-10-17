import { createElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import type { LayoutView } from "../generated/LayoutView";
import { Frame, FrameBody } from "./Frame";
import {
  INITIAL_LAYOUT,
  layoutShortcuts,
  layoutView,
  paneShows,
  paneWidth,
  type ShellLayout,
  shellLayout,
  sizeClass,
  tabCounts,
  togglePane,
  toggleExpanded,
  withDrag,
} from "./layout";
import { openOverlay, overlays } from "./overlays";
import { createRegistries, type Registries, RegistriesContext } from "./registry";
import { NOTHING_SELECTED, selection } from "./selection";

// `layout.view` as Rust sends it (`layout.rs`).
const VIEW: LayoutView = {
  classes: [
    { class: "w1280", below: 1360 },
    { class: "w1440", below: 1680 },
    { class: "w1920", below: null },
  ],
  pane_defaults: { w1280: 320, w1440: 400, w1920: 460 },
  pane_min: 320,
  pane_max: 560,
  strip: 28,
  tabs: [
    { tab: "board", label: "Board", icon: "columns-3" },
    { tab: "files", label: "Files", icon: "folder-tree" },
  ],
  tab_keys: "⇧⌘B · ⇧⌘O",
  files_empty: "The Files view comes in a later update.",
  show_pane: "Show the pane",
  pane_resize: "Pane width",
  pane_keys: "⌘\\",
};

// The frame renders without a DOM: React's server renderer gives its markup (no new test packages).
function render(app: Registries, bannerFallback?: ReactNode): string {
  return renderToStaticMarkup(
    createElement(RegistriesContext, { value: app }, createElement(Frame, { bannerFallback })),
  );
}

/** The frame's body at a window width, with `layout`. */
function body(windowWidth: number, layout: ShellLayout, ticketOpen = false): string {
  return renderToStaticMarkup(
    createElement(
      RegistriesContext,
      { value: createRegistries() },
      createElement(FrameBody, { view: VIEW, layout, windowWidth, ticketOpen }),
    ),
  );
}

const slots = (html: string) => [...html.matchAll(/data-slot="([a-z_]+)"/g)].map((m) => m[1]);
const paneStyle = (html: string) => /class="frame-pane-body" style="width:(\d+)px"/.exec(html)?.[1];

afterEach(() => {
  selection.set(NOTHING_SELECTED);
  overlays.store.set([]);
  shellLayout.set(INITIAL_LAYOUT);
  layoutView.set(undefined);
  tabCounts.set({});
});

describe("the frame", () => {
  it("has rail | pane | main with the tabs, each empty slot showing its name; the panel only with a ticket open", () => {
    const app = createRegistries();
    // Before `layout.view` answers: no pane and no tabs yet.
    expect(slots(render(app))).toEqual(["header", "rail", "banners", "board", "overlays", "toasts"]);

    layoutView.set(VIEW);
    const closed = render(app);
    expect(slots(closed)).toEqual(["header", "rail", "banners", "pane", "board", "overlays", "toasts"]);
    for (const name of ["Header", "Rail", "Board"]) expect(closed).toContain(`>${name}</div>`);
    expect(closed).toContain('aria-selected="true"><span class="icon"');
    expect(closed).toContain('<span class="frame-tab-label">Board</span>');
    expect(closed).toContain('<span class="frame-tabs-keys">⇧⌘B · ⇧⌘O</span>');
    // The header row is the window's drag region, children included (T-0214).
    expect(closed).toContain('<div class="frame-header" data-slot="header" data-tauri-drag-region="deep">');
    expect(closed).not.toContain("Ticket panel");

    selection.set({ ...NOTHING_SELECTED, ticket: "SHOP-22" });
    const open = render(app);
    expect(slots(open)).toEqual([
      "header",
      "rail",
      "banners",
      "pane",
      "board",
      "panel",
      "overlays",
      "toasts",
    ]);
    expect(open).toContain(">Ticket panel</div>");
  });

  it("draws what screens register: slots, and the open overlays that are registered", () => {
    const app = createRegistries();
    app.slots.register("board", { Component: () => createElement("p", null, "the board") });
    app.slots.register("pane", { Component: () => createElement("p", null, "the pane") });
    app.slots.register("title_bar_end", { Component: () => createElement("p", null, "caption buttons") });
    app.overlays.register("edit_ticket", {
      Component: ({ params }) => createElement("p", null, `editing ${params.ticket}`),
    });
    openOverlay("edit_ticket", { ticket: "SHOP-7" });
    openOverlay("inbox");
    layoutView.set(VIEW);
    tabCounts.set({ board: "26" });

    const html = render(app, createElement("p", { role: "alert" }, "service failed"));
    expect(html).toContain("<p>the board</p>");
    expect(html).toContain("<p>the pane</p>");
    expect(html).toContain(
      '<span class="frame-tab-label">Board</span><span class="frame-tab-count">26</span>',
    );
    expect(html).not.toContain(">Board</div>");
    expect(html).toMatch(/<p>caption buttons<\/p><\/div><div class="frame-rail" data-slot="rail"/);
    expect(html).toContain(
      '<div class="frame-layer" id="overlay-host" data-slot="overlays"><p>editing SHOP-7</p></div>',
    );
    expect(html).toContain('<p role="alert">service failed</p>');
  });

  it("gives the banner area to the banners screen once it registers", () => {
    const app = createRegistries();
    app.slots.register("banners", { Component: () => createElement("p", null, "banner") });
    const html = render(app, "service failed");
    expect(html).toContain("<p>banner</p>");
    expect(html).not.toContain("service failed");
  });

  it("the Files tab shows its quiet line until M24, in place of the board", () => {
    const html = body(1440, { ...INITIAL_LAYOUT, tab: "files" });
    expect(slots(html)).toEqual(["header", "rail", "banners", "pane", "files"]);
    expect(html).toContain('<p class="frame-files-empty">The Files view comes in a later update.</p>');
    expect(html).toMatch(
      /aria-selected="true"><span class="icon"[^>]*><\/span><span class="frame-tab-label">Files/,
    );
  });

  it("the pane is 320 / 400 / 460 px at 1280 / 1440 / 1920", () => {
    expect(paneStyle(body(1280, INITIAL_LAYOUT))).toBe("320");
    expect(paneStyle(body(1440, INITIAL_LAYOUT))).toBe("400");
    expect(paneStyle(body(1920, INITIAL_LAYOUT))).toBe("460");
  });

  it("a hidden pane shows the 28 px strip and stays mounted", () => {
    const shown = body(1440, INITIAL_LAYOUT);
    expect(shown).not.toContain("frame-pane-strip");
    const hidden = body(1440, { ...INITIAL_LAYOUT, paneHidden: true });
    expect(hidden).toContain('class="frame-pane-strip" style="width:28px" aria-label="Show the pane"');
    expect(hidden).toContain('class="frame-pane-body" style="width:400px" hidden=""');
    expect(paneShows({ ...INITIAL_LAYOUT, paneHidden: true }, false)).toBe("strip");
  });

  it("the expanded panel hides the pane, strip included, while a ticket is open", () => {
    const expanded = { ...INITIAL_LAYOUT, panelExpanded: true };
    const html = body(1440, expanded, true);
    expect(html).toContain('data-slot="pane" data-shows="none"');
    expect(html).not.toContain("frame-pane-strip");
    expect(html).toContain('<div class="frame-main" data-expanded="true">');
    expect(paneShows({ ...expanded, paneHidden: true }, true)).toBe("none");
    // With no ticket open the pane comes back as it was.
    expect(paneShows(expanded, false)).toBe("pane");
    expect(body(1440, expanded, false)).not.toContain("data-expanded");
  });
});

describe("the layout's state", () => {
  it("the size class splits at the midpoints, from Rust's table", () => {
    const cases: [number, string][] = [
      [1280, "w1280"],
      [1359, "w1280"],
      [1360, "w1440"],
      [1440, "w1440"],
      [1679, "w1440"],
      [1680, "w1920"],
      [1920, "w1920"],
    ];
    for (const [width, cls] of cases) expect(sizeClass(width, VIEW.classes)).toBe(cls);
    expect(sizeClass(1440, [])).toBeNull();
  });

  it("⇧⌘B / ⇧⌘O switch the tabs and ⌘\\ hides and shows the pane", () => {
    const app = createRegistries();
    const stop = layoutShortcuts(app);
    expect(app.shortcuts.run({ action: "show_files" })).toBe(true);
    expect(shellLayout.get().tab).toBe("files");
    app.shortcuts.run({ action: "show_board" });
    expect(shellLayout.get().tab).toBe("board");
    app.shortcuts.run({ action: "toggle_pane" });
    expect(shellLayout.get().paneHidden).toBe(true);
    app.shortcuts.run({ action: "toggle_pane" });
    expect(shellLayout.get().paneHidden).toBe(false);
    stop();
    expect(app.shortcuts.run({ action: "show_files" })).toBe(false);
  });

  it("each size class keeps its own dragged pane width, back and forth; a drag stays within 320–560", () => {
    shellLayout.update((l) => ({ ...l, paneWidths: withDrag(l.paneWidths, "w1440", 500) }));
    const at = (width: number) => paneStyle(body(width, shellLayout.get()));
    expect(at(1440)).toBe("500");
    expect(at(1280)).toBe("320");
    expect(at(1440)).toBe("500");
    expect(paneWidth({ ...INITIAL_LAYOUT, paneWidths: { w1920: 900 } }, "w1920", VIEW)).toBe(560);
    expect(paneWidth({ ...INITIAL_LAYOUT, paneWidths: { w1920: 100 } }, "w1920", VIEW)).toBe(320);
    // Double-click: back to the class's default.
    shellLayout.update((l) => ({ ...l, paneWidths: withDrag(l.paneWidths, "w1440", null) }));
    expect(at(1440)).toBe("400");
  });

  it("the switches flip the page state", () => {
    togglePane();
    toggleExpanded();
    expect(shellLayout.get()).toMatchObject({ paneHidden: true, panelExpanded: true });
  });
});
