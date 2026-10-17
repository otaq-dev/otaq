import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import { overlays } from "../../app/overlays";
import { createRegistries, registered } from "../../app/registry";
import type { ProcessesView } from "../../generated/ProcessesView";
import type { RunButtonView } from "../../generated/RunButtonView";
import type { RunCardView } from "../../generated/RunCardView";
import type { RunCommandView } from "../../generated/RunCommandView";
import type { ShellTabView } from "../../generated/ShellTabView";
import { INITIAL_STATE } from "../../terminal/state";
import { run } from ".";
import { afterStart, pressOf, suggestionFor } from "./logic";
import { ProcessesBody } from "./Processes";
import { RunCommandBody } from "./RunCommand";
import { RunCard } from "./RunTab";
import { ShellTabBody } from "./ShellTab";

afterEach(() => {
  overlays.store.set([]);
});

const button = (kind: RunButtonView["kind"], extra: Partial<RunButtonView> = {}): RunButtonView => ({
  kind,
  label: kind,
  icon: "play",
  primary: true,
  enabled: true,
  url: null,
  copy: null,
  ...extra,
});

const RUNNING: RunCardView = {
  title: "Running",
  tone: "success",
  icon: "dot",
  port: ":4020",
  command: "pnpm dev --port $PORT",
  message: null,
  buttons: [
    button("open", { label: "Open localhost:4020", url: "http://localhost:4020", icon: "external-link" }),
    button("stop", { label: "Stop", primary: false, icon: "square" }),
  ],
  note: null,
};

const COMMAND: RunCommandView = {
  project: "SHOP",
  title: "What command runs shop-web?",
  intro: "Runs in the ticket’s worktree when you press Run.",
  command_placeholder: "e.g. pnpm dev",
  url_label: "Open URL",
  optional: "optional",
  url_placeholder: "e.g. http://localhost:$PORT",
  url_hint: "Leave empty for backends, CLIs and libraries — then there’s no Open button.",
  ask: {
    label: "Ask the agent",
    tooltip: "Asks Strong (the Project agent’s) to suggest a run command",
    note: "The agent looks at the repo and suggests a command.",
  },
  thinking: null,
  why: null,
  saved: null,
  error: null,
  save: "Save & start",
  can_save: false,
  cancel: "Cancel",
  done: "Done",
  fill: null,
};

const SHELL: ShellTabView = {
  key: "BILL-4",
  label: "Shell · not the agent",
  path: "~/.otaq/wt/billing-api/BILL-4",
  port: "Run port · $PORT=4004",
  session: null,
  live: false,
  problem: null,
  stopped: { title: "Stopped by restart · was open at 08:24", note: "A restart ended this shell." },
  reopen: "Reopen",
  hint: "Typing goes to the shell, not the agent",
  leave: { text: "leave terminal", keys: "⇧⌘`" },
  terminal_label: "BILL-4 · shell",
};

const noop = () => undefined;

describe("the Run & Shell screen", () => {
  it("registers the Run and Shell tabs, the run-command dialog and the processes menu", () => {
    const app = createRegistries();
    run(app);
    const r = registered(app);
    expect(r.panelTabs).toEqual(["run", "shell"]);
    expect(r.overlays).toEqual(["processes", "run_command"]);
  });

  it("draws the Running card with Open and Stop", () => {
    const html = renderToStaticMarkup(createElement(RunCard, { card: RUNNING, busy: false, onPress: noop }));
    for (const text of ["Running", ":4020", "pnpm dev --port $PORT", "Open localhost:4020", "Stop"]) {
      expect(html).toContain(text);
    }
    expect(html).toContain('data-tone="success"');
    const backend = renderToStaticMarkup(
      createElement(RunCard, {
        card: {
          ...RUNNING,
          buttons: [RUNNING.buttons[1] ?? button("stop")],
          note: "No Open URL set for billing-api.",
        },
        busy: false,
        onPress: noop,
      }),
    );
    expect(backend).toContain("No Open URL set for billing-api.");
    expect(backend).not.toContain("Open localhost");
  });

  it("draws the run-command dialog's steps", () => {
    const empty = renderToStaticMarkup(
      createElement(RunCommandBody, {
        view: COMMAND,
        command: "",
        openUrl: "",
        onCommand: noop,
        onOpenUrl: noop,
        onAsk: noop,
      }),
    );
    for (const text of [
      "Ask the agent",
      "The agent looks at the repo",
      "Open URL",
      "optional",
      "Leave empty for backends",
    ]) {
      expect(empty).toContain(text);
    }
    const thinking = renderToStaticMarkup(
      createElement(RunCommandBody, {
        view: {
          ...COMMAND,
          ask: null,
          thinking: { title: "The agent is reading the repo…", lines: ["· Read(package.json)"] },
        },
        command: "",
        openUrl: "",
        onCommand: noop,
        onOpenUrl: noop,
        onAsk: noop,
      }),
    );
    expect(thinking).toContain("The agent is reading the repo…");
    expect(thinking).toContain("· Read(package.json)");
    expect(thinking).not.toContain("Ask the agent");
    const saved = renderToStaticMarkup(
      createElement(RunCommandBody, {
        view: { ...COMMAND, ask: null, saved: "Saved for shop-web. Running SHOP-19 on :4020…" },
        command: "pnpm dev --port $PORT",
        openUrl: "",
        onCommand: noop,
        onOpenUrl: noop,
        onAsk: noop,
      }),
    );
    expect(saved).toContain("Saved for shop-web.");
    expect(saved).toContain("disabled");
  });

  it("draws the Shell strip and a shell a restart stopped", () => {
    const html = renderToStaticMarkup(
      createElement(ShellTabBody, { view: SHELL, state: INITIAL_STATE, onState: noop, onReopen: noop }),
    );
    for (const text of [
      "Shell · not the agent",
      "~/.otaq/wt/billing-api/BILL-4",
      "Run port · $PORT=4004",
      "Stopped by restart · was open at 08:24",
      "Reopen",
      "Typing goes to the shell, not the agent",
    ]) {
      expect(html).toContain(text);
    }
    expect(html).not.toContain("leave terminal");
  });

  it("lists every running process with Stop all", () => {
    const view: ProcessesView = {
      title: "Running processes",
      stop_all: "Stop all",
      stop: "Stop",
      empty: "Nothing runs.",
      rows: [
        {
          project_name: "billing-api",
          key: "BILL-4",
          title: "Invoice PDF export",
          port: ":4004",
          uptime: "12 min",
          url: null,
          open: null,
        },
        {
          project_name: "shop-web",
          key: "SHOP-19",
          title: "Price table",
          port: ":4020",
          uptime: "3 min",
          url: "http://localhost:4020",
          open: "Open :4020",
        },
      ],
    };
    const html = renderToStaticMarkup(createElement(ProcessesBody, { view, onOpenTicket: noop }));
    for (const text of ["Running processes", "Stop all", "billing-api", "BILL-4", ":4004", "Open :4020"]) {
      expect(html).toContain(text);
    }
    expect(html.match(/Open :4020/g)).toHaveLength(1);
  });
});

describe("Run's interaction rules", () => {
  it("routes each button and asks for the command when there is none", () => {
    expect(pressOf(button("run"))).toEqual({ kind: "request", method: "run.start" });
    expect(pressOf(button("retry"))).toEqual({ kind: "request", method: "run.start" });
    expect(pressOf(button("stop"))).toEqual({ kind: "request", method: "run.stop" });
    expect(pressOf(button("open", { url: "http://localhost:4020" }))).toEqual({
      kind: "open",
      url: "http://localhost:4020",
    });
    expect(pressOf(button("open", { url: "http://localhost:4020", enabled: false }))).toEqual({
      kind: "none",
    });
    expect(pressOf(button("copy_lsof", { copy: "lsof -i :4020" }))).toEqual({
      kind: "copy",
      text: "lsof -i :4020",
    });
    expect(pressOf(button("edit_command"))).toEqual({ kind: "edit_command" });

    afterStart({ outcome: "needs_command", project: "SHOP" }, "SHOP-19");
    expect(overlays.store.get()).toEqual([
      { id: "run_command", params: { project: "SHOP", ticket: "SHOP-19" } },
    ]);
  });

  it("takes only its project's Ask the agent events", () => {
    const event = {
      kind: "service" as const,
      name: "run_suggestion",
      event: {
        kind: "run_suggestion",
        project: "SHOP",
        update: { state: "progress", line: "Read(package.json)" },
      },
    };
    expect(suggestionFor(event, "SHOP")).toEqual({ state: "progress", line: "Read(package.json)" });
    expect(suggestionFor(event, "BILL")).toBeUndefined();
    expect(suggestionFor({ kind: "resync" }, "SHOP")).toBeUndefined();
  });
});
