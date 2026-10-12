import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { createRegistries, registered } from "../../app/registry";
import type { DiffAnchor } from "../../generated/DiffAnchor";
import type { DiffComments } from "../../generated/DiffComments";
import type { DiffFileRow } from "../../generated/DiffFileRow";
import type { DiffGroup } from "../../generated/DiffGroup";
import type { DiffTabView } from "../../generated/DiffTabView";
import { diff } from ".";
import { DiffTabBody } from "./DiffTab";
import {
  canSave,
  chunks,
  composerKey,
  composerReducer,
  fileListWidth,
  filteredGroups,
  groupFolded,
  rowAnchors,
  saveRequest,
  showsLines,
  showsSideBySide,
  threadsByAnchor,
  toggled,
} from "./logic";

const anchor = (file: string, line: number, side: "old" | "new" = "new"): DiffAnchor => ({
  file,
  line,
  side,
  key: `${file}\u0000${side}\u0000${String(line)}`,
});

const row = (path: string, extra: Partial<DiffFileRow> = {}): DiffFileRow => ({
  path,
  label: path,
  letter: "M",
  status: "modified",
  added: "+1",
  removed: null,
  binary: false,
  note: null,
  viewed: false,
  collapsed: null,
  binary_label: null,
  rename_label: null,
  rows: 5,
  in_group: null,
  ...extra,
});

const VIEW: DiffTabView = {
  key: "SHOP-18",
  round: 2,
  revision: "r1",
  base: "main at a1b2c3d",
  rest: "· 3 files · +412 −38",
  base_prefix: "diff vs",
  viewed: "1 / 3 viewed",
  modes: ["Unified", "Side-by-side"],
  side_by_side: false,
  side_by_side_from: 900,
  summary: {
    title: "Agent summary",
    text: "Added POST /api/checkout.",
    note: "no tests are run by Crewboard",
    open: true,
  },
  files: [
    row("api/checkout/route.ts", {
      letter: "A",
      status: "added",
      added: "+184",
      note: "new file",
      viewed: true,
    }),
    row("package-lock.json", { collapsed: "Generated file: collapsed." }),
    row("public/hero.png", {
      binary: true,
      added: null,
      binary_label: "Binary file changed · 184 KB → 96 KB",
    }),
  ],
  groups: null,
  filter_placeholder: "Filter 3 files",
  empty: null,
  viewed_label: "Viewed",
  load_diff: "Load diff",
};

const COMMENTS: DiffComments = {
  key: "SHOP-18",
  can_comment: true,
  pending: 2,
  pending_label: "2 pending comments",
  send_back_label: "Send back (2)",
  threads: [
    {
      anchor: anchor("api/checkout/route.ts", 9),
      comments: [
        {
          id: "c1",
          author: "You",
          initial: "Y",
          age: "2m ago",
          badge: "pending",
          pending: true,
          editable: true,
          body: "Return { code, message }",
        },
      ],
    },
  ],
  texts: {
    placeholder: "Leave a comment",
    hint: "Markdown · comments are sent to the agent only on Send back",
    add: "Add comment",
    save: "Save",
    cancel: "Cancel",
    reply: "Reply",
    edit: "Edit comment",
    delete: "Delete comment",
    add_on_line: "Comment on this line",
  },
  save_keys: "⌘↵",
};

describe("the Diff tab", () => {
  it("registers itself as the panel's Diff tab", () => {
    const app = createRegistries();
    diff(app);
    expect(registered(app).panelTabs).toContain("diff");
  });

  it("draws the toolbar, the file list and each file's header like the frame", () => {
    const html = renderToStaticMarkup(
      createElement(DiffTabBody, {
        ticket: "SHOP-18",
        view: VIEW,
        comments: COMMENTS,
        width: 1200,
        reload: () => undefined,
      }),
    );
    for (const text of [
      "Agent summary",
      "diff vs",
      "main at a1b2c3d",
      "2 pending comments",
      "1 / 3 viewed",
      "Side-by-side",
      "new file",
      "+184",
      "Generated file: collapsed.",
      "Load diff",
      "Binary file changed · 184 KB → 96 KB",
    ]) {
      expect(html).toContain(text);
    }
    expect(html).toContain('aria-checked="true"');
  });

  it("explains an empty diff", () => {
    const html = renderToStaticMarkup(
      createElement(DiffTabBody, {
        ticket: "SHOP-18",
        view: {
          ...VIEW,
          files: [],
          empty: { title: "No changes", base: "vs main at a1b2c3d", text: "This branch has no changes" },
        },
        comments: undefined,
        width: 660,
        reload: () => undefined,
      }),
    );
    expect(html).toContain("No changes");
    expect(html).toContain("vs main at a1b2c3d");
    expect(html).not.toContain("diff-list");
  });
});

describe("the diff's interaction rules", () => {
  it("falls back to unified below 900 px and narrows the file list", () => {
    expect(showsSideBySide(true, 1200, 900)).toBe(true);
    expect(showsSideBySide(true, 660, 900)).toBe(false);
    expect(showsSideBySide(false, 1200, 900)).toBe(false);
    expect(fileListWidth(1200)).toBe(280);
    expect(fileListWidth(700)).toBe(200);
  });

  it("filters long lists and folds the generated folder until clicked", () => {
    const files = [row("src/app/page.tsx"), row("src/lib/money.ts"), row("yarn.lock")];
    const groups: DiffGroup[] = [
      { name: "src/app/", folded_label: "src/app/", files: [0], starts_folded: false },
      { name: "src/lib/", folded_label: "src/lib/", files: [1], starts_folded: false },
      { name: "generated", folded_label: "generated · collapsed", files: [2], starts_folded: true },
    ];
    expect(filteredGroups(groups, files, "MONEY").map((g) => g.name)).toEqual(["src/lib/"]);
    expect(filteredGroups(groups, files, "  ")).toHaveLength(3);
    const generated = groups[2];
    if (!generated) throw new Error("no group");
    expect(groupFolded(generated, new Set())).toBe(true);
    expect(groupFolded(generated, toggled(new Set(), "generated"))).toBe(false);
    expect(toggled(new Set(["a"]), "a").size).toBe(0);
  });

  it("shows lines only for open text files", () => {
    const collapsed = row("big.ts", { collapsed: "Large change: collapsed." });
    expect(showsLines(collapsed, new Set())).toBe(false);
    expect(showsLines(collapsed, new Set(["big.ts"]))).toBe(true);
    expect(showsLines(row("logo.png", { binary: true }), new Set(["logo.png"]))).toBe(false);
    expect(showsLines(row("a.ts"), new Set())).toBe(true);
  });

  it("cuts long files into chunks", () => {
    expect(chunks(250, 120)).toEqual([
      [0, 120],
      [120, 240],
      [240, 250],
    ]);
    expect(chunks(0)).toEqual([]);
  });

  it("finds threads by anchor and side-by-side anchors once", () => {
    const threads = threadsByAnchor(COMMENTS);
    expect(threads.get(anchor("api/checkout/route.ts", 9).key)?.comments[0]?.id).toBe("c1");
    expect(threadsByAnchor(undefined).size).toBe(0);
    const context = anchor("a.ts", 4);
    expect(rowAnchors(context, context)).toEqual([context]);
    expect(rowAnchors(anchor("a.ts", 3, "old"), anchor("a.ts", 3))).toHaveLength(2);
    expect(rowAnchors(null, undefined)).toEqual([]);
  });
});

describe("the comment box", () => {
  const at = anchor("lib/cart.ts", 15);

  it("adds a new comment, replies in a thread, and edits a pending one", () => {
    const fresh = composerReducer(null, { kind: "open", anchor: at });
    if (!fresh) throw new Error("not open");
    expect(canSave(fresh, "  ")).toBe(false);
    expect(saveRequest(fresh, " Use the shared formatter ", "SHOP-18")).toEqual({
      method: "diff.comment_add",
      params: {
        key: "SHOP-18",
        file: "lib/cart.ts",
        line: 15,
        side: "new",
        body: "Use the shared formatter",
      },
    });
    const reply = composerReducer(null, { kind: "open", anchor: at, replyTo: "c1" });
    if (!reply) throw new Error("not open");
    expect(saveRequest(reply, "ok", "SHOP-18")).toEqual({
      method: "diff.comment_reply",
      params: { id: "c1", body: "ok" },
    });
    const edit = composerReducer(reply, { kind: "edit", anchor: at, id: "c2", body: "old text" });
    if (!edit) throw new Error("not open");
    expect(edit.initial).toBe("old text");
    expect(saveRequest(edit, "new text", "SHOP-18")).toEqual({
      method: "diff.comment_edit",
      params: { id: "c2", body: "new text" },
    });
  });

  it("saves once, shows a refusal, and closes when saved or cancelled", () => {
    const open = composerReducer(null, { kind: "open", anchor: at });
    const saving = composerReducer(open, { kind: "saving" });
    expect(saving?.saving).toBe(true);
    expect(canSave(saving, "text")).toBe(false);
    const failed = composerReducer(saving, { kind: "failed", message: "SHOP-18 isn't in review" });
    expect(failed?.error).toBe("SHOP-18 isn't in review");
    expect(canSave(failed, "text")).toBe(true);
    expect(composerReducer(failed, { kind: "saved" })).toBeNull();
    expect(composerReducer(open, { kind: "cancel" })).toBeNull();
    expect(composerReducer(null, { kind: "saving" })).toBeNull();
  });

  it("saves with ⌘↵ or Ctrl+↵ and cancels with Esc", () => {
    expect(composerKey({ key: "Enter", metaKey: true, ctrlKey: false })).toBe("save");
    expect(composerKey({ key: "Enter", metaKey: false, ctrlKey: true })).toBe("save");
    expect(composerKey({ key: "Enter", metaKey: false, ctrlKey: false })).toBeNull();
    expect(composerKey({ key: "Escape", metaKey: false, ctrlKey: false })).toBe("cancel");
  });
});
