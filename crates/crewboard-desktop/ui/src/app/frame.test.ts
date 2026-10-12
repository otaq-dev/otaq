import { createElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import { Frame } from "./Frame";
import { openOverlay, overlays } from "./overlays";
import { createRegistries, type Registries, RegistriesContext } from "./registry";
import { NOTHING_SELECTED, selection } from "./selection";

// The frame renders without a DOM: React's server renderer gives its markup (no new test packages).
function render(app: Registries, bannerFallback?: ReactNode): string {
  return renderToStaticMarkup(
    createElement(RegistriesContext, { value: app }, createElement(Frame, { bannerFallback })),
  );
}

const slots = (html: string) => [...html.matchAll(/data-slot="([a-z_]+)"/g)].map((m) => m[1]);

afterEach(() => {
  selection.set(NOTHING_SELECTED);
  overlays.store.set([]);
});

describe("the frame", () => {
  it("has the anatomy's regions, each empty slot showing its name; the panel only with a ticket open", () => {
    const app = createRegistries();
    const closed = render(app);
    expect(slots(closed)).toEqual(["header", "rail", "banners", "board", "overlays", "toasts"]);
    for (const name of ["Header", "Rail", "Board"]) expect(closed).toContain(`>${name}</div>`);
    expect(closed).not.toContain("Ticket panel");

    selection.set({ ...NOTHING_SELECTED, ticket: "SHOP-22" });
    const open = render(app);
    expect(slots(open)).toEqual(["header", "rail", "banners", "board", "panel", "overlays", "toasts"]);
    expect(open).toContain(">Ticket panel</div>");
  });

  it("draws what screens register: slots, and the open overlays that are registered", () => {
    const app = createRegistries();
    app.slots.register("board", { Component: () => createElement("p", null, "the board") });
    app.slots.register("title_bar_end", { Component: () => createElement("p", null, "caption buttons") });
    app.overlays.register("edit_ticket", {
      Component: ({ params }) => createElement("p", null, `editing ${params.ticket}`),
    });
    openOverlay("edit_ticket", { ticket: "SHOP-7" });
    openOverlay("inbox");

    const html = render(app, createElement("p", { role: "alert" }, "service failed"));
    expect(html).toContain("<p>the board</p>");
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
});
