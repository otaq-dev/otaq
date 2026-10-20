import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import { Frame } from "../../app/Frame";
import { layoutView } from "../../app/layout";
import { openOverlay, overlays } from "../../app/overlays";
import { createRegistries, registered, RegistriesContext } from "../../app/registry";
import type { FlowPreviewView } from "../../generated/FlowPreviewView";
import type { LayoutView } from "../../generated/LayoutView";
import type { PageEvent } from "../../generated/PageEvent";
import type { PageEvents } from "../../service";
import { flowPreview } from ".";
import { type FooterMode, PreviewBody, PreviewFooter } from "./FlowPreview";
import {
  applyPreview,
  askForChanges,
  cancelPreview,
  followProposals,
  NO_CHOICES,
  pickedTool,
  proposedIn,
  withClash,
  withFolder,
  withModel,
} from "./logic";

// `flow_preview.view` for `s18-27`: shop-web's Project agent adds QA and changes Security reviewer.
const CHANGE: FlowPreviewView = {
  project: "SHOP",
  proposal: 3,
  title: "Changes to shop-web’s flow",
  subtitle: "Proposed by the Project agent · nothing changes until you apply it",
  close_label: "Close",
  same: null,
  table: null,
  changes: [
    { icon: "plus", text: "shop-web · stage QA (your check) after Security", detail: null },
    {
      icon: "pencil",
      text: "Security reviewer · model: opus → fable",
      detail: "Used in: shop-web (Security)",
    },
  ],
  agents: null,
  problems: [],
  clashes: [],
  warnings: [],
  applies:
    "The new stage waits for a quiet board: 8 tickets in shop-web are still working, and no new ticket starts meanwhile. Agent changes apply to the next session that starts.",
  agent_waits: "The agent waits for your answer in its terminal",
  cancel: "Cancel",
  ask: {
    label: "Ask for more changes",
    placeholder: "What else should the agent change?",
    send: "Send to the agent",
    back: "Back",
  },
  apply: { label: "Apply", enabled: true, off: null },
  close: null,
  choices: { clashes: { "Security reviewer": "keep_mine" }, skill_folders: {}, models: {} },
};

// `s18-31`: a preset (the user's own: no agent to ask) with its whole table and the amber warning.
const PRESET: FlowPreviewView = {
  ...CHANGE,
  title: "Solo autopilot for billing-api",
  subtitle: "Preset · nothing changes until you apply it",
  changes: [],
  table: {
    headings: ["STAGE", "AGENT · MODEL", "OPTIONS A TICKET CAN PICK"],
    always: "Backlog → Ready for Dev → … → Done: always first and last",
    rows: [
      {
        id: "dev",
        icon: "bot",
        name: "Dev",
        kind: "Agent",
        agent: "Developer",
        agent_muted: false,
        model: "default · Claude Code · sonnet",
        options: "—",
        options_muted: true,
        why: "Most of your sessions end with /work-ticket",
      },
    ],
  },
  agents: { heading: "AGENTS IT ADDS", rows: [], already: "None · already there: Developer · Validator" },
  warnings: [{ level: "amber", text: "Done merges into main without asking you." }],
  applies: null,
  agent_waits: null,
  ask: null,
};

// `s18-31`'s import: problems keep Apply off; a clash with Keep mine picked.
const PROBLEMS: FlowPreviewView = {
  ...PRESET,
  table: null,
  agents: null,
  warnings: [],
  problems: [
    {
      text: "security-review isn’t on this machine",
      fixed: false,
      fix: {
        kind: "folder",
        skill: "security-review",
        label: "Import its folder…",
        picker_title: "x",
        picked: null,
      },
    },
  ],
  clashes: [
    {
      agent: "Security reviewer",
      text: "Security reviewer is in the file and already here, with other instructions.",
      options: [
        { id: "keep_mine", label: "Keep mine", selected: true },
        { id: "take_theirs", label: "Take theirs", selected: false },
        { id: "add_as", label: "Add as “Security reviewer (team.yaml)”", selected: false },
      ],
    },
  ],
  apply: { label: "Apply", enabled: false, off: "Fix the problem above to apply it" },
};

const SAME: FlowPreviewView = {
  ...CHANGE,
  title: "No changes",
  subtitle: "Proposed by the Project agent",
  same: "This is the same as your current flow.",
  changes: [],
  applies: null,
  cancel: null,
  apply: null,
  close: "Close",
};

function footer(view: FlowPreviewView, mode: FooterMode = "buttons", text = ""): string {
  const none = () => undefined;
  return renderToStaticMarkup(
    createElement(PreviewFooter, {
      view,
      mode,
      text,
      onText: none,
      onCancel: none,
      onAsk: none,
      onSend: none,
      onBack: none,
      onApply: none,
    }),
  );
}

function body(view: FlowPreviewView): string {
  return renderToStaticMarkup(
    createElement(PreviewBody, {
      view,
      choices: NO_CHOICES,
      onChoices: () => undefined,
      onError: () => undefined,
    }),
  );
}

/** Button labels in order, with whether each is disabled. */
function buttons(html: string): string[] {
  return [...html.matchAll(/<button type="button" class="button"([^>]*)>(.*?)<\/button>/g)].map(
    (m) => `${(m[2] ?? "").replace(/<[^>]*>/g, "")}${(m[1] ?? "").includes('disabled=""') ? " (off)" : ""}`,
  );
}

/** A request stand-in that records what it was asked. */
function recorder() {
  const asked: { method: string; params: unknown }[] = [];
  const ask = (method: string, params: unknown = null) => {
    asked.push({ method, params });
    return Promise.resolve(null);
  };
  return { asked, ask };
}

afterEach(() => {
  overlays.store.set([]);
});

describe("the flow preview screen", () => {
  it("registers its overlay and the follower in the main area", () => {
    const app = createRegistries();
    flowPreview(app);
    const r = registered(app);
    expect(r.overlays).toEqual(["flow_preview"]);
    expect(r.slots).toEqual(["main_area"]);
  });

  it("is drawn over the main area, not the window: the pane stays beside it", () => {
    const app = createRegistries();
    app.slots.register("pane", { Component: () => createElement("p", null, "the pane") });
    app.overlays.register("flow_preview", {
      Component: ({ params }) => createElement("p", null, `preview of ${params.project}`),
    });
    layoutView.set({
      classes: [{ class: "w1440", below: null }],
      pane_defaults: { w1280: 320, w1440: 400, w1920: 460 },
      pane_min: 320,
      pane_max: 560,
      strip: 28,
      tabs: [{ tab: "board", label: "Board", icon: "columns-3" }],
      tab_row: false,
      tab_keys: "⇧⌘B",
      files_empty: "",
      show_pane: "Show the pane",
      pane_resize: "Pane width",
      pane_keys: "⌘B",
    } satisfies LayoutView);
    openOverlay("flow_preview", { project: "API" });
    const html = renderToStaticMarkup(createElement(RegistriesContext, { value: app }, createElement(Frame)));
    expect(html).toContain(
      '<div class="frame-main-layer" data-slot="main_overlays"><p>preview of API</p></div>',
    );
    expect(html).toContain('<div class="frame-layer" id="overlay-host" data-slot="overlays"></div>');
    expect(html).toContain("<p>the pane</p>");
  });
});

describe("the buttons", () => {
  it("a Project agent's proposal: Cancel · Ask for more changes · Apply, and the agent waits", () => {
    const html = footer(CHANGE);
    expect(buttons(html)).toEqual(["Cancel", "Ask for more changes", "Apply"]);
    expect(html).toContain("The agent waits for your answer in its terminal");
  });

  it("a preset (the user's own): Cancel · Apply, no Ask, no waiting line", () => {
    const html = footer(PRESET);
    expect(buttons(html)).toEqual(["Cancel", "Apply"]);
    expect(html).not.toContain("waits for your answer");
  });

  it("problems left: Apply is off and says why", () => {
    const html = footer(PROBLEMS);
    expect(buttons(html)).toEqual(["Cancel", "Apply (off)"]);
    expect(html).toContain('title="Fix the problem above to apply it"');
  });

  it("nothing differs: Ask for more changes · Close", () => {
    expect(buttons(footer(SAME))).toEqual(["Ask for more changes", "Close"]);
  });

  it("Ask for more changes takes a line; Send waits for it", () => {
    const empty = footer(CHANGE, "asking");
    expect(empty).toContain('placeholder="What else should the agent change?"');
    expect(buttons(empty)).toEqual(["Back", "Send to the agent (off)"]);
    expect(buttons(footer(CHANGE, "asking", "also skip Review for docs tickets"))).toEqual([
      "Back",
      "Send to the agent",
    ]);
  });

  it("while an answer is on its way, nothing else can be pressed", () => {
    expect(buttons(footer(CHANGE, "busy"))).toEqual([
      "Cancel (off)",
      "Ask for more changes (off)",
      "Apply (off)",
    ]);
  });
});

describe("the answers", () => {
  it("Apply sends the proposal with the picks the view gives back", async () => {
    const { asked, ask } = recorder();
    await applyPreview(CHANGE, ask);
    expect(asked).toEqual([
      {
        method: "flow_preview.apply",
        params: { project: "SHOP", proposal: 3, choices: CHANGE.choices },
      },
    ]);
  });

  it("Cancel drops the proposal", async () => {
    const { asked, ask } = recorder();
    await cancelPreview(CHANGE, ask);
    expect(asked).toEqual([{ method: "flow_preview.cancel", params: { project: "SHOP" } }]);
  });

  it("Ask for more changes sends the trimmed line, and nothing while it is empty", async () => {
    const { asked, ask } = recorder();
    expect(askForChanges(CHANGE, "   ", ask)).toBeNull();
    await askForChanges(CHANGE, "  keep Review for API tickets ", ask);
    expect(asked).toEqual([
      {
        method: "flow_preview.ask",
        params: { project: "SHOP", proposal: 3, text: "keep Review for API tickets" },
      },
    ]);
  });
});

describe("the picks", () => {
  it("clashes, folders and local models are kept until Apply", () => {
    let c = withClash(NO_CHOICES, "Security reviewer", "add_as");
    c = withFolder(c, "security-review", "/tmp/skills/security-review");
    c = withModel(c, "Security reviewer", "codex");
    expect(pickedTool(c, "Security reviewer")).toBe("codex");
    c = withModel(c, "Security reviewer", "codex", "gpt-5.5");
    expect(c).toEqual({
      clashes: { "Security reviewer": "add_as" },
      skill_folders: { "security-review": "/tmp/skills/security-review" },
      models: { "Security reviewer": "codex:gpt-5.5" },
    });
    expect(withModel(c, "Security reviewer", "default").models["Security reviewer"]).toBe("default");
    expect(pickedTool(NO_CHOICES, "Security reviewer")).toBeNull();
  });
});

describe("what it shows", () => {
  it("a change: its lines with Used in, and when it applies", () => {
    const html = body(CHANGE);
    expect(html).toContain("shop-web · stage QA (your check) after Security");
    expect(html).toContain("<small>Used in: shop-web (Security)</small>");
    expect(html).toContain('data-tone="info"');
    expect(html).toContain("The new stage waits for a quiet board");
  });

  it("a whole flow: the table with why lines, the agents already there, the amber warning", () => {
    const html = body(PRESET);
    expect(html).toContain("OPTIONS A TICKET CAN PICK");
    expect(html).toContain("Backlog → Ready for Dev → … → Done: always first and last");
    expect(html).toContain("<strong>Dev</strong><small>Agent</small>");
    expect(html).toContain("default · Claude Code · sonnet");
    expect(html).toContain("Most of your sessions end with /work-ticket");
    expect(html).toContain("None · already there: Developer · Validator");
    expect(html).toMatch(/data-tone="amber".*Done merges into main without asking you\./);
  });

  it("problems in red with their fix, and the clash with Keep mine picked", () => {
    const html = body(PROBLEMS);
    expect(html).toMatch(/data-tone="red".*security-review isn’t on this machine.*Import its folder…/);
    expect(html).toMatch(
      /<input type="radio" class="radio" name="[^"]+" checked=""\/><span>Keep mine<\/span>/,
    );
    expect(html).toContain("Add as “Security reviewer (team.yaml)”");
  });

  it("the same flow: one green line", () => {
    const html = body(SAME);
    expect(html).toMatch(/data-tone="ok".*This is the same as your current flow\./);
  });
});

describe("when it opens", () => {
  const event = (change: Record<string, unknown>, project = "API"): PageEvent => ({
    kind: "service",
    name: "flow_changed",
    event: { kind: "flow_changed", project, change },
  });

  it("a proposal opened in any project names that project; other flow events don't open it", () => {
    expect(proposedIn(event({ what: "proposed", proposal: 3 }))).toBe("API");
    expect(proposedIn(event({ what: "proposal_closed", proposal: 3 }))).toBeNull();
    expect(proposedIn(event({ what: "waiting" }))).toBeNull();
    expect(proposedIn({ kind: "resync" })).toBeNull();
  });

  it("follows the event stream, and opens the newest open proposal when it opens or resyncs", async () => {
    let listener: ((e: PageEvent) => void) | undefined;
    let stopped = false;
    const events: PageEvents = {
      listen(l) {
        listener = l;
        return {
          ready: Promise.resolve(),
          stop: () => {
            stopped = true;
          },
        };
      },
    };
    const opened: string[] = [];
    const asked: string[] = [];
    const ask = (method: string) => {
      asked.push(method);
      return Promise.resolve(["API", "NOTE"]);
    };
    const stop = followProposals(events, (p) => opened.push(p), ask);
    await Promise.resolve();
    await Promise.resolve();
    expect(opened).toEqual(["API"]);
    listener?.(event({ what: "proposed", proposal: 4 }, "SHOP"));
    listener?.(event({ what: "applied", version: 2 }, "SHOP"));
    expect(opened).toEqual(["API", "SHOP"]);
    listener?.({ kind: "resync" });
    await Promise.resolve();
    await Promise.resolve();
    expect(opened).toEqual(["API", "SHOP", "API"]);
    expect(asked).toEqual(["flow_preview.open", "flow_preview.open"]);
    stop();
    expect(stopped).toBe(true);
  });
});
