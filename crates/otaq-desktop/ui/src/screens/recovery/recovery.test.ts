import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { createRegistries, registered } from "../../app/registry";
import type { AfterRestartRow } from "../../generated/AfterRestartRow";
import type { AfterRestartSummary } from "../../generated/AfterRestartSummary";
import type { CleanupConfirm } from "../../generated/CleanupConfirm";
import type { CleanupPage } from "../../generated/CleanupPage";
import type { Notices } from "../../generated/Notices";
import type { ScratchConfirm } from "../../generated/ScratchConfirm";
import { recovery } from ".";
import { SummaryBody } from "./AfterRestart";
import { CleanupBody, CleanupConfirmDialog } from "./Cleanup";
import { setUnchecked, startFolded, summaryToOpen, toggleFold } from "./logic";
import { ScratchBody } from "./StartFromScratch";

const nothing = () => undefined;

const notices = (summary_for: number | null): Notices => ({
  banners: [],
  toasts: [],
  only_restart_allowed: false,
  input_enabled: true,
  summary_for,
});

function row(key: string, extra: Partial<AfterRestartRow> = {}): AfterRestartRow {
  return {
    key,
    project: "shop-web",
    title: "Upgrade to Next.js 15",
    state: "interrupted",
    headline: null,
    detail: "was Agent Working · 311k",
    checked: true,
    resuming: `${key} · Resuming…`,
    resuming_detail: "Reattaching claude --resume in ~/.otaq/wt/shop-web/" + key,
    ...extra,
  };
}

const SUMMARY: AfterRestartSummary = {
  title: "8 sessions were interrupted",
  text: "Only Dev tickets that were working become Interrupted.",
  rows: [
    row("SHOP-25"),
    row("SHOP-19", { state: "resumed", headline: "SHOP-19 · Resumed", detail: "Agent Working again" }),
    row("BILL-4", {
      state: "failed",
      headline: "BILL-4 · Couldn’t resume",
      detail: "Claude has no saved conversation",
    }),
  ],
  more: {
    count: "+3",
    keys: "SHOP-30, SHOP-40.4, SHOP-40.2",
    detail: "was working · expand to resume one by one",
    checked: true,
    resumable: ["SHOP-30", "SHOP-40.4", "SHOP-40.2"],
  },
  agents: {
    title: "PROJECT AGENTS · NOT IN RESUME SELECTED · TERMINALS ARE GONE",
    rows: [
      {
        id: "a1",
        project: "shop-web",
        tag: "agent",
        name: "Checkout slowness",
        detail: "Ended by restart · same conversation",
        resumable: true,
      },
    ],
    discarded: "A “New agent” that had no request yet is discarded, not listed.",
  },
  notes_title: "NOT IN RESUME SELECTED",
  notes: ["Turquoise tickets keep their question — SHOP-22."],
  resume: "Resume",
  retry: "Retry",
  scratch: "Start from scratch",
  foot: "They stay in your inbox if you close this.",
  later: "Later",
  resume_selected: "Resume selected (4)",
  selected: ["SHOP-25", "SHOP-30", "SHOP-40.4", "SHOP-40.2"],
};

const CLEANUP: CleanupPage = {
  title: "Cleanup",
  count: "3 items",
  text: "Leftovers that no ticket owns. Review and select — nothing is deleted automatically.",
  groups: [
    {
      group: "worktrees",
      title: "Worktrees",
      icon: "folder-git-2",
      count: "1",
      all_checked: true,
      starts_folded: false,
      select_all: "select all",
      rows: [
        {
          target: { group: "worktrees", id: "/h/wt/shop-web/SHOP-14" },
          title: "~/.otaq/wt/shop-web/SHOP-14",
          detail: "no ticket owns it",
          size: "412 MB",
          age: "9 d",
          checked: true,
          warning: false,
        },
      ],
    },
    {
      group: "kept_worktrees",
      title: "Done tickets with uncommitted changes",
      icon: "triangle-alert",
      count: "3",
      all_checked: false,
      starts_folded: true,
      select_all: "select all",
      rows: [],
    },
  ],
  empty: null,
  footer: "1 selected · 412 MB",
  delete: "Delete selected…",
  confirm: {
    title: "Delete 1 item?",
    text: "This can’t be undone.",
    lines: [{ icon: "folder-git-2", text: "~/.otaq/wt/shop-web/SHOP-14 · 412 MB (remove worktree)" }],
    confirm: "Delete 1 item",
  },
};

describe("the after-restart sheet", () => {
  it("opens by itself once per service start", () => {
    expect(summaryToOpen(undefined, null)).toBeNull();
    expect(summaryToOpen(notices(null), null)).toBeNull();
    expect(summaryToOpen(notices(42), null)).toBe(42);
    expect(summaryToOpen(notices(42), 42)).toBeNull();
    expect(summaryToOpen(notices(43), 42)).toBe(43);
  });

  it("draws rows, what each Resume did, the folded row and the notes", () => {
    const html = renderToStaticMarkup(
      createElement(SummaryBody, {
        view: SUMMARY,
        resuming: new Set(["SHOP-25"]),
        onCheck: nothing,
        onResume: nothing,
        onRetry: nothing,
        onUnfold: nothing,
        onLater: nothing,
        onResumeAgent: nothing,
      }),
    );
    for (const text of [
      "8 sessions were interrupted",
      "SHOP-25 · Resuming…",
      "SHOP-19 · Resumed",
      "BILL-4 · Couldn’t resume",
      "Start from scratch",
      "+3",
      "SHOP-30, SHOP-40.4, SHOP-40.2",
      "NOT IN RESUME SELECTED",
      "PROJECT AGENTS",
      "Checkout slowness",
      "Ended by restart · same conversation",
      "is discarded, not listed",
      "Resume selected (4)",
      "Later",
    ]) {
      expect(html).toContain(text);
    }
    expect(html).toContain('data-state="failed"');
  });

  it("ticks and unticks keys together", () => {
    const off = setUnchecked(new Set(), ["SHOP-30", "SHOP-31"], false);
    expect([...off]).toEqual(["SHOP-30", "SHOP-31"]);
    expect([...setUnchecked(off, ["SHOP-30"], true)]).toEqual(["SHOP-31"]);
  });
});

describe("cleanup", () => {
  it("draws groups, folded ones with select all, the footer and the confirmation", () => {
    const folded = startFolded(CLEANUP);
    expect([...folded]).toEqual(["kept_worktrees"]);
    expect([...toggleFold(folded, "kept_worktrees")]).toEqual([]);
    const html = renderToStaticMarkup(
      createElement(CleanupBody, {
        page: CLEANUP,
        folded,
        error: null,
        onFold: nothing,
        onToggle: nothing,
        onToggleGroup: nothing,
        onDelete: nothing,
        onClose: nothing,
      }),
    );
    for (const text of [
      "3 items",
      "~/.otaq/wt/shop-web/SHOP-14",
      "412 MB",
      "select all",
      "1 selected · 412 MB",
    ]) {
      expect(html).toContain(text);
    }
    expect(html).toContain("Delete selected…");
    const confirm = renderToStaticMarkup(
      createElement(CleanupConfirmDialog, {
        confirm: CLEANUP.confirm as CleanupConfirm,
        deleting: false,
        onConfirm: nothing,
        onCancel: nothing,
      }),
    );
    expect(confirm).toContain("(remove worktree)");
    expect(confirm).toContain("Delete 1 item");
  });

  it("says when there is nothing to clean up, without a Delete button", () => {
    const html = renderToStaticMarkup(
      createElement(CleanupBody, {
        page: {
          ...CLEANUP,
          count: "0 items",
          groups: [],
          footer: null,
          confirm: null,
          empty: { title: "Nothing to clean up", text: "Every session belongs to a ticket." },
        },
        folded: new Set<string>(),
        error: null,
        onFold: nothing,
        onToggle: nothing,
        onToggleGroup: nothing,
        onDelete: nothing,
        onClose: nothing,
      }),
    );
    expect(html).toContain("Nothing to clean up");
    expect(html).not.toContain("Delete selected…");
  });
});

describe("start from scratch", () => {
  it("says what is kept and what is lost", () => {
    const view: ScratchConfirm = {
      title: "Start BILL-4 Invoice PDF export from scratch?",
      text: "The previous conversation can’t be restored.",
      kept_title: "Kept",
      kept: ["Branch otaq/BILL-4 and its commits"],
      lost_title: "Lost",
      lost: "The previous conversation",
      note: "The History tab records this.",
      confirm: "Start from scratch",
    };
    const html = renderToStaticMarkup(createElement(ScratchBody, { view, error: null }));
    expect(html).toContain("Kept");
    expect(html).toContain("Branch otaq/BILL-4 and its commits");
    expect(html).toContain("Lost");
  });
});

describe("the screen", () => {
  it("registers its three overlays", () => {
    const app = createRegistries();
    recovery(app);
    const ids = registered(app).overlays;
    expect(ids).toEqual(expect.arrayContaining(["after_restart", "start_from_scratch", "cleanup"]));
  });
});
