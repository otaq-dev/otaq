import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import { actionsOf, BannerRows, LINE_HANDLERS, RestartBody, ToastStack } from ".";
import { menuKey } from "../../components/logic";
import type { LimitLine } from "../../generated/LimitLine";
import type { MoveWaitingMenu } from "../../generated/MoveWaitingMenu";
import type { ToolAvatarView } from "../../generated/ToolAvatarView";
import { request } from "../../tauri";
import { MoveWaitingList } from "./LimitLines";
import { createOverlayStack } from "../../app/overlays";
import { createSelection, type TerminalFocusRequest } from "../../app/selection";
import { createStore } from "../../app/store";
import type { Banner } from "../../generated/Banner";
import type { Notices } from "../../generated/Notices";
import type { PageEvent } from "../../generated/PageEvent";
import type { Toast } from "../../generated/Toast";
import type { ToastAction } from "../../generated/ToastAction";
import { createPageEvents } from "../../service";
import {
  followNotifications,
  limitLinesOf,
  moveMenuItems,
  onlyRestartAllowed,
  openTarget,
  pressBanner,
  watchNotices,
} from "./notices";

vi.mock("../../tauri", () => ({ request: vi.fn(), subscribeEvents: vi.fn(), windowAction: vi.fn() }));

/** An event stream the test feeds by hand; `open()` resolves the subscription. */
function fakeStream() {
  let send: (event: unknown) => void = () => undefined;
  let open: () => void = () => undefined;
  const events = createPageEvents(
    (onEvent) =>
      new Promise<void>((resolve) => {
        send = onEvent;
        open = resolve;
      }),
  );
  return {
    events,
    open: () => {
      open();
    },
    send: (event: PageEvent) => {
      send(event);
    },
  };
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

const RESTART_NOW: Banner["buttons"][number] = {
  id: "action",
  label: "Restart now",
  icon: "rotate-cw",
  enabled: true,
  primary: false,
};

const banner = (over: Partial<Banner> = {}): Banner => ({
  kind: "service_update_ready",
  tone: "info",
  icon: "refresh-cw",
  title: "Service update ready",
  text: "Restart now (3 agents working, 2 waiting) · or later, when it suits you",
  buttons: [RESTART_NOW],
  dismissable: true,
  tooltip: null,
  lines: [],
  ...over,
});

const notices = (banners: Banner[], only = false): Notices => ({
  banners,
  toasts: [],
  only_restart_allowed: only,
  input_enabled: !only,
  summary_for: null,
});

describe("the notices", () => {
  it("load once the stream is open, then follow each notices event; a newer event wins over the answer", async () => {
    const stream = fakeStream();
    let answer: (value: unknown) => void = () => undefined;
    const ask = vi.fn(() => new Promise<unknown>((resolve) => (answer = resolve)));
    const seen: Notices[] = [];
    const stop = watchNotices((n) => seen.push(n), stream.events, ask);
    stream.open();
    await settle();
    expect(ask).toHaveBeenCalledWith("notices.view");
    stream.send({ kind: "notices", notices: notices([banner()], false) });
    answer(notices([]));
    await settle();
    expect(seen).toEqual([notices([banner()])]);
    stop();
    stream.send({ kind: "notices", notices: notices([]) });
    expect(seen).toHaveLength(1);
  });

  it("say when only Restart works", () => {
    expect(onlyRestartAllowed(undefined)).toBe(false);
    expect(onlyRestartAllowed(notices([], true))).toBe(true);
  });
});

describe("banner buttons", () => {
  const routes = () => {
    const ask = vi.fn<(method: string, params?: unknown) => Promise<unknown>>(() => Promise.resolve(null));
    const stack = createOverlayStack();
    return { ask, stack, routes: { ask, overlays: stack } };
  };

  it("Restart now asks first; Start service starts at once", async () => {
    const r = routes();
    await pressBanner("service_update_ready", "action", r.routes);
    await pressBanner("restart_required", "action", r.routes);
    expect(r.stack.store.get().map((o) => o.id)).toEqual(["restart_service"]);
    expect(r.ask).not.toHaveBeenCalled();
    await pressBanner("service_stopped", "action", r.routes);
    expect(r.ask).toHaveBeenCalledWith("notices.restart");
  });

  it("Resume all after interrupted sessions opens the inbox's confirmation with its ticket list", async () => {
    const r = routes();
    await pressBanner("sessions_interrupted", "action", r.routes);
    expect(r.stack.store.get().map((o) => o.id)).toEqual(["resume_all"]);
    expect(r.ask).not.toHaveBeenCalled();
  });

  it("Review & resume opens the summary sheet; the limit's Resume all and Read the docs are requests", async () => {
    const r = routes();
    await pressBanner("sessions_interrupted", "secondary", r.routes);
    expect(r.stack.store.get().map((o) => o.id)).toEqual(["after_restart"]);
    await pressBanner("usage_limit", "resume", r.routes);
    await pressBanner("permission_warning", "action", r.routes);
    await pressBanner("reconnecting", "action", r.routes);
    await pressBanner("trust_failed", "action", r.routes);
    await pressBanner("trust_note", "action", r.routes);
    expect(r.ask.mock.calls.map((c) => c[0])).toEqual(["notices.resume_after_limit", "notices.open_docs"]);
  });
});

describe("notification clicks", () => {
  const routes = () => ({
    selection: createSelection(),
    focus: createStore<TerminalFocusRequest | null>(null),
    overlays: createOverlayStack(),
  });

  it("open the ticket on its Terminal tab with the terminal focused, or the inbox", () => {
    const r = routes();
    openTarget({ project: "SHOP", ticket: "SHOP-22" }, r);
    expect(r.selection.get()).toMatchObject({ project: "SHOP", ticket: "SHOP-22", tab: "terminal" });
    expect(r.focus.get()).toEqual({ ticket: "SHOP-22", seq: 1 });
    openTarget({ project: "SHOP", ticket: "SHOP-22" }, r);
    expect(r.focus.get()?.seq).toBe(2);
    expect(r.overlays.store.get()).toEqual([]);
    openTarget({ project: null, ticket: null }, r);
    expect(r.overlays.store.get().map((o) => o.id)).toEqual(["inbox"]);
  });

  it("open what the app was started with, then each clicked notification, bringing the window forward", async () => {
    const stream = fakeStream();
    const ask = vi.fn((method: string, params?: unknown): Promise<unknown> => {
      if (method === "notices.open_at_start") return Promise.resolve({ project: "SHOP", ticket: "SHOP-9" });
      const key = (params as { key: string | null }).key;
      return Promise.resolve({ project: key ? "SHOP" : null, ticket: key });
    });
    const opened: unknown[] = [];
    const forward = vi.fn(() => Promise.resolve());
    const stop = followNotifications(stream.events, ask, (t) => opened.push(t.ticket), forward);
    stream.open();
    await settle();
    expect(opened).toEqual(["SHOP-9"]);
    expect(forward).not.toHaveBeenCalled();
    stream.send({
      kind: "service",
      name: "open_from_notification",
      event: { kind: "open_from_notification", key: "SHOP-22" },
    });
    stream.send({
      kind: "service",
      name: "open_from_notification",
      event: { kind: "open_from_notification" },
    });
    stream.send({ kind: "service", name: "ticket_changed", event: { kind: "ticket_changed" } });
    await settle();
    expect(ask).toHaveBeenCalledWith("notices.open_target", { key: "SHOP-22" });
    expect(opened).toEqual(["SHOP-9", "SHOP-22", null]);
    expect(forward).toHaveBeenCalledTimes(2);
    stop();
  });
});

describe("drawn", () => {
  it("banners: one row each, with their buttons and the close button only where it can close", () => {
    const html = renderToStaticMarkup(
      createElement(BannerRows, {
        banners: [
          banner({
            kind: "usage_limit",
            tone: "attention",
            title: "Usage limit reset",
            buttons: [{ id: "resume", label: "Resume all (8)", icon: "play", enabled: true, primary: true }],
            dismissable: false,
          }),
          banner({ buttons: [{ ...RESTART_NOW, label: "Restarting…", enabled: false }] }),
        ],
      }),
    );
    expect(html.match(/class="banner"/g)).toHaveLength(2);
    expect(html).toContain('data-tone="attention"');
    expect(html).toContain("Resume all (8)");
    expect(html).toMatch(/<button[^>]*disabled[^>]*>.*Restarting…/);
    expect(html.match(/aria-label="Dismiss"/g)).toHaveLength(1);
    expect(renderToStaticMarkup(createElement(BannerRows, { banners: [] }))).toBe("");
  });

  it("folder trust: the note is its text alone with a close; the warning has neither", () => {
    const html = renderToStaticMarkup(
      createElement(BannerRows, {
        banners: [
          banner({
            kind: "trust_failed",
            tone: "warning",
            icon: "shield-x",
            title: "Claude Code doesn’t trust 2 projects’ folders:",
            text: "NOTE, SHOP. Their agents will wait on Claude Code’s trust screen.",
            buttons: [],
            dismissable: false,
            tooltip: {
              title: "Claude Code doesn’t trust 2 projects’ folders",
              lines: ["NOTE: why.", "SHOP: why."],
            },
          }),
          banner({
            kind: "trust_note",
            title: "",
            text: "Otaq let Claude Code work in your projects’ folders, as adding a project now does.",
            buttons: [],
          }),
        ],
      }),
    );
    expect(html.match(/class="banner-title"/g)).toHaveLength(1);
    expect(html).toContain('class="tooltip-anchor"');
    expect(html.match(/aria-label="Dismiss"/g)).toHaveLength(1);
    expect(html).not.toMatch(/class="button"/);
  });

  it("toasts: their links, and a ticket's title opens it", () => {
    const toast = (over: Partial<Toast>): Toast => ({
      id: 1,
      level: "success",
      title: "Local merge done · SHOP-18",
      text: "Squashed into main as 9f3e2c1",
      icon: null,
      icon_tone: null,
      link: null,
      ticket: null,
      ...over,
    });
    const html = renderToStaticMarkup(
      createElement(ToastStack, {
        toasts: [
          toast({ ticket: "SHOP-18" }),
          toast({
            id: 2,
            level: "error",
            title: "gh is not logged in",
            text: "Run gh auth login, then retry Open PR",
            link: { kind: "copy", label: "Copy command", text: "gh auth login" },
          }),
        ],
      }),
    );
    expect(html).toContain(
      '<button type="button" class="toast-title toast-open">Local merge done · SHOP-18</button>',
    );
    expect(html).toContain('<span class="toast-title">gh is not logged in</span>');
    expect(html).toContain("Copy command");
    expect(html).toContain('role="alert"');
  });

  it("the epic queue toast: its buttons follow the title and a press goes to Rust", async () => {
    const { request } = await import("../../tauri");
    const undo: ToastAction = { kind: "undo_epic_queue", epic: "SHOP-45", keys: ["SHOP-45.1", "SHOP-45.2"] };
    const toast: Toast = {
      id: 7,
      level: "success",
      title: "Queued 2 tickets. Auto-merge is off for shop-web, so each one waits for your review.",
      text: null,
      icon: null,
      icon_tone: null,
      link: {
        kind: "actions",
        actions: [
          {
            label: "Turn on for epic children",
            action: { kind: "epic_children_auto_merge", project: "SHOP", name: "shop-web", undo: null },
          },
          { label: "Undo", action: undo },
        ],
      },
      ticket: null,
    };
    const html = renderToStaticMarkup(createElement(ToastStack, { toasts: [toast] }));
    expect(html).toMatch(
      /toast-line.*toast-title">Queued 2 tickets.*toast-actions.*Turn on for epic children.*Undo/,
    );
    expect(html).not.toContain("toast-text");
    vi.mocked(request).mockResolvedValue(null);
    actionsOf(toast)?.[1]?.onClick();
    expect(vi.mocked(request)).toHaveBeenCalledWith("notices.toast_action", {
      id: 7,
      label: "Undo",
      action: undo,
    });
  });

  it("the restart dialog's body: the warning callout, then the rest", () => {
    const html = renderToStaticMarkup(
      createElement(RestartBody, {
        dialog: {
          title: "Restart the background service?",
          message: "3 working agents and 2 waiting ones will be interrupted. You can Resume all afterwards.",
          detail: "Waiting tickets keep their questions.",
          confirm: "Restart service",
        },
      }),
    );
    expect(html).toMatch(
      /notices-restart-message.*3 working agents.*notices-restart-detail.*Waiting tickets/,
    );
  });
});

const avatar = (tool: "claude_code" | "codex", letters: string): ToolAvatarView => ({
  tool,
  letters,
  background: `tool-${letters.toLowerCase()}`,
  foreground: `tool-${letters.toLowerCase()}-fg`,
  hover: tool === "claude_code" ? "Claude Code" : "Codex",
});

const MOVE: MoveWaitingMenu = {
  label: "Move waiting to…",
  title: "MOVE 4 WAITING TICKETS TO",
  tool: "claude",
  items: [
    { agent: "standard", name: "Standard", detail: "Codex · gpt-5.5", avatar: avatar("codex", "CX") },
    { agent: "strong-gpt", name: "Strong GPT", detail: "Codex · gpt-5.5-pro", avatar: avatar("codex", "CX") },
  ],
};

const claudeLine = (over: Partial<LimitLine> = {}): LimitLine => ({
  tool: "claude",
  avatar: avatar("claude_code", "CC"),
  tone: "neutral",
  icon: "gauge",
  title: "Claude Code limit reached",
  text: "Resets 3:45 pm · 4 tickets waiting",
  buttons: [
    {
      label: "Resume Claude Code",
      icon: "play",
      action: { kind: "resume_tool", tool: "claude" },
      enabled: false,
      primary: false,
    },
  ],
  move_waiting: MOVE,
  ...over,
});

const codexLine: LimitLine = {
  tool: "codex",
  avatar: avatar("codex", "CX"),
  tone: "warning",
  icon: "triangle-alert",
  title: "Codex isn’t available",
  text: "logged out · 5 tickets moved to Sonnet",
  buttons: [
    {
      label: "Sign in…",
      icon: "plug",
      action: { kind: "sign_in", tool: "codex" },
      enabled: true,
      primary: false,
    },
  ],
  move_waiting: null,
};

const limitBanner = (lines: LimitLine[]): Banner =>
  banner({
    kind: "usage_limit",
    tone: "neutral",
    icon: "gauge",
    title: lines[0]?.title ?? "",
    text: lines[0]?.text ?? "",
    buttons: [{ id: "resume", label: "Resume all", icon: "play", enabled: false, primary: false }],
    dismissable: false,
    lines,
  });

describe("the limit banner per tool", () => {
  it("draws one line per tool with its avatar, Resume <tool>, Sign in… and Move waiting to…", () => {
    const b = limitBanner([claudeLine(), codexLine]);
    expect(limitLinesOf(b)).toHaveLength(2);
    const html = renderToStaticMarkup(createElement(BannerRows, { banners: [b] }));
    expect(html.match(/notices-line"/g)).toHaveLength(2);
    expect(html).toMatch(
      /data-tone="neutral" data-tool="claude".*Claude Code limit reached.*Resume Claude Code/,
    );
    expect(html).toMatch(/data-tone="warning" data-tool="codex".*Codex isn’t available.*Sign in…/);
    expect(html.match(/aria-haspopup="menu"/g)).toHaveLength(1);
    expect(html).toContain("Move waiting to…");
    expect(html).not.toContain("Resume all");
    expect(html).not.toContain('data-tone="attention"');
  });

  it("the one-tool banner stays M22's: no avatar, no lines, no Move waiting to…", () => {
    const one = limitBanner([claudeLine({ avatar: null, move_waiting: null })]);
    expect(limitLinesOf(one)).toEqual([]);
    const html = renderToStaticMarkup(createElement(BannerRows, { banners: [one] }));
    expect(html).not.toContain("notices-line");
    expect(html).not.toContain("Move waiting to…");
    expect(html).not.toContain("tool-avatar");
    expect(html).toContain("Resume all");
  });

  it("Resume <tool> calls limits.resume_tool with its tool; a picked agent calls limits.move_waiting", () => {
    const ask = vi.mocked(request);
    ask.mockReset();
    ask.mockResolvedValue(null);
    LINE_HANDLERS.onAction({ kind: "resume_tool", tool: "claude" });
    LINE_HANDLERS.onAction({ kind: "resume_tool", tool: "codex" });
    LINE_HANDLERS.onMove("claude", "strong-gpt");
    expect(ask.mock.calls).toEqual([
      ["limits.resume_tool", { tool: "claude" }],
      ["limits.resume_tool", { tool: "codex" }],
      ["limits.move_waiting", { tool: "claude", agent: "strong-gpt" }],
    ]);
  });

  it("Move waiting to…'s list: its title, the agents; arrows skip the title, Enter picks, Esc closes", () => {
    const html = renderToStaticMarkup(
      createElement(MoveWaitingList, { menu: MOVE, onChoose: () => undefined, onClose: () => undefined }),
    );
    expect(html).toMatch(/role="menu".*MOVE 4 WAITING TICKETS TO.*CX.*Standard.*Codex · gpt-5.5.*Strong GPT/);
    const items = moveMenuItems(MOVE);
    expect(items.map((i) => i.header)).toEqual([true, false, false]);
    expect(menuKey(items, null, "ArrowDown")).toEqual({ kind: "highlight", index: 1 });
    expect(menuKey(items, 2, "ArrowDown")).toEqual({ kind: "highlight", index: 1 });
    expect(menuKey(items, 1, "ArrowUp")).toEqual({ kind: "highlight", index: 2 });
    expect(menuKey(items, 2, "Enter")).toEqual({ kind: "choose", id: "strong-gpt" });
    expect(menuKey(items, 1, "Escape")).toEqual({ kind: "close" });
  });
});
