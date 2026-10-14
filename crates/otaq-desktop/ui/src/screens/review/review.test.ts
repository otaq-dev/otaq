import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { createRegistries, registered } from "../../app/registry";
import type { ApproveView } from "../../generated/ApproveView";
import type { ReviewChanged } from "../../generated/ReviewChanged";
import { review } from ".";
import { ResultDialog } from "./Dialogs";
import { afterApproveAnyway, afterChoice, afterMerge, firstStep, isSubmitKey } from "./logic";

const CHANGED: ReviewChanged = {
  title: "SHOP-18 changed since you reviewed it · 1 new commit",
  commits: [{ sha: "4d9e2b1", subject: "fix(checkout): handle missing payment_intent" }],
  files: ["src/app/api/checkout/route.ts"],
  reviewed: "abc",
  note: "Only the new commits are shown. Approve anyway merges them too.",
  anyway: "Approve anyway",
  send_back: "Send back",
};

const VIEW: ApproveView = {
  key: "SHOP-18",
  menu: {
    key: "SHOP-18",
    title: "Approve SHOP-18 Checkout API",
    items: [
      {
        choice: "local_merge",
        number: 1,
        icon: "git-merge",
        title: "Local merge",
        detail: "Squash into main",
        enabled: true,
      },
    ],
    note: "Both open a confirmation step before anything runs.",
  },
  push_to_pr: false,
  confirm: {
    title: "Local merge SHOP-18 Checkout API",
    into: "Into (SHOP-18’s base branch)",
    target: "main",
    repo: "~/code/shop-web",
    locked: false,
    message_label: "Squash commit message",
    message: "SHOP-18: Checkout API",
    note: "The worktree for SHOP-18 is removed unless it has uncommitted changes.",
    conflict: null,
    merge: "Squash & merge",
    cancel: "Cancel",
    hint: "cancel · ⌘↵ merge",
  },
  merging: {
    title: "Local merge SHOP-18 Checkout API",
    steps: ["Rebase onto main"],
    label: "Merging…",
    note: "",
  },
  changed: null,
};

describe("the review screen", () => {
  it("registers both dialogs and ⌘↵ / ⌘⌫", () => {
    const app = createRegistries();
    review(app);
    const r = registered(app);
    expect(r.overlays).toEqual(expect.arrayContaining(["local_merge", "send_back"]));
    expect(r.shortcuts).toEqual(expect.arrayContaining(["approve", "send_back"]));
  });

  it("draws the conflict and uncommitted results with their texts", () => {
    const conflict = renderToStaticMarkup(
      createElement(ResultDialog, {
        ticket: "SHOP-18",
        result: {
          kind: "conflict",
          title: "Rebase onto main stopped — conflict",
          text: "main moved since SHOP-18 was built.",
          files: ["src/lib/stripe/session.ts", "package.json"],
          help: "Send to agent to fix: SHOP-18 returns to Dev",
          worktree: "~/.otaq/wt/shop-web/SHOP-18",
          fix: "Send to agent to fix",
          close: "Close",
        },
        close: () => undefined,
        retry: () => undefined,
        toSendBack: () => undefined,
      }),
    );
    for (const text of [
      "Rebase onto main stopped — conflict",
      "package.json",
      "Send to agent to fix",
      "Close",
    ]) {
      expect(conflict).toContain(text);
    }
    const uncommitted = renderToStaticMarkup(
      createElement(ResultDialog, {
        ticket: "SHOP-19",
        result: {
          kind: "uncommitted",
          title: "SHOP-19 has 2 uncommitted files",
          files: ["src/components/pricing/PriceTable.tsx"],
          text: "Send it back so the agent commits them, or look at them first.",
          view: "View",
          send_back: "Send back",
          cancel: "Cancel",
        },
        close: () => undefined,
        retry: () => undefined,
        toSendBack: () => undefined,
      }),
    );
    for (const text of ["SHOP-19 has 2 uncommitted files", "View", "Send back", "PriceTable.tsx"]) {
      expect(uncommitted).toContain(text);
    }
  });
});

describe("the approve flow", () => {
  it("starts at the confirmation from a button or a drop, at the choice from ⌘↵", () => {
    expect(firstStep(VIEW, false)).toEqual({ kind: "confirm" });
    expect(firstStep(VIEW, true)).toEqual({ kind: "menu" });
    expect(firstStep({ ...VIEW, push_to_pr: true }, false)).toEqual({ kind: "open_pr" });
  });

  it("shows new commits since the review first; Approve anyway goes on", () => {
    const first = firstStep({ ...VIEW, changed: CHANGED }, true);
    expect(first).toEqual({ kind: "changed", changed: CHANGED, checkedFirst: true });
    if (first.kind !== "changed") throw new Error("not changed");
    expect(afterApproveAnyway(first, VIEW, true)).toEqual({ kind: "menu" });
    expect(afterApproveAnyway(first, VIEW, false)).toEqual({ kind: "confirm" });
    expect(afterApproveAnyway(first, { ...VIEW, push_to_pr: true }, false)).toEqual({ kind: "open_pr" });
    expect(afterApproveAnyway({ ...first, checkedFirst: false }, VIEW, false)).toBe("merge");
  });

  it("hands Open PR and Push to PR to their dialog", () => {
    expect(afterChoice("local_merge")).toEqual({ kind: "confirm" });
    expect(afterChoice("open_pr")).toEqual({ kind: "open_pr" });
    expect(afterChoice("push_to_pr")).toEqual({ kind: "open_pr" });
  });

  it("closes after a merge and shows every other answer", () => {
    expect(afterMerge({ kind: "merged", toasts: [] })).toEqual({ kind: "done" });
    expect(afterMerge({ kind: "changed", changed: CHANGED })).toEqual({
      kind: "changed",
      changed: CHANGED,
      checkedFirst: false,
    });
    const refused = {
      kind: "refused",
      title: "Can’t merge into main yet",
      message: "m",
      retry: "Retry",
      close: "Close",
    } as const;
    expect(afterMerge(refused)).toEqual({ kind: "result", result: refused });
  });

  it("merges with ⌘↵ or Ctrl+↵ in the message", () => {
    expect(isSubmitKey({ key: "Enter", metaKey: true, ctrlKey: false })).toBe(true);
    expect(isSubmitKey({ key: "Enter", metaKey: false, ctrlKey: true })).toBe(true);
    expect(isSubmitKey({ key: "Enter", metaKey: false, ctrlKey: false })).toBe(false);
  });
});
