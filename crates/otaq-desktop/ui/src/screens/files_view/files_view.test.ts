import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";

import { NOTHING_SELECTED, selection } from "../../app/selection";
import type { FilesTreeRow } from "../../generated/FilesTreeRow";
import type { FilesViewPage } from "../../generated/FilesViewPage";
import type { FilesViewPicker } from "../../generated/FilesViewPicker";
import { openFileIn } from "../files/FilesTab";
import { searchFocus } from "../files/logic";
import { FilesViewBody, PickerBody } from "./FilesView";
import { FIRST, goToFile, pickFile, pickerSections, viewSearchFocus, viewStates } from "./logic";

vi.mock("../../tauri", () => ({
  request: vi.fn(() => new Promise(() => undefined)),
  subscribeEvents: vi.fn(() => ({ ready: new Promise(() => undefined), stop: () => undefined })),
}));

afterEach(() => {
  selection.set(NOTHING_SELECTED);
  viewStates.set(new Map());
});

const ignore = () => undefined;

const row = (path: string, extra: Partial<FilesTreeRow> = {}): FilesTreeRow => ({
  name: path.split("/").pop() ?? path,
  path,
  kind: "file",
  depth: 0,
  mark: null,
  quiet: true,
  opens: true,
  tags: [],
  child: null,
  chips: [],
  changed_below: null,
  branch: null,
  struck: false,
  ...extra,
});

const PAGE: FilesViewPage = {
  project: "SHOP",
  base_branch: "main",
  source: { title: "main · every ticket’s changes", subtitle: "the base branch · 8 tickets, 11 files" },
  search: { placeholder: "Search files by name", keys: "⌥⌘P" },
  filters: [
    { filter: "all", label: "All", tooltip: null, detail: null },
    { filter: "changed", label: "Changed 11", tooltip: null, detail: null },
    {
      filter: "needs_you",
      label: "Needs you 6",
      tooltip: "Needs you · 6 files",
      detail: "changed by the 4 tickets that need you",
    },
    { filter: "same_file", label: "Same file 1", tooltip: null, detail: null },
  ],
  filter: "all",
  loading: null,
  rows: [],
  empty: null,
  pick: "Pick a file in the tree to read it, or search for one by name.",
  working: true,
};

const ROWS = [
  row("src", { kind: "folder", quiet: false, changed_below: "8 changed" }),
  row("src/lib/legacy-price.ts", {
    depth: 2,
    quiet: false,
    struck: true,
    mark: { letter: "D", status: "deleted", counts: "−31", uncommitted: null },
    chips: [{ key: "SHOP-19", tone: "working" }],
  }),
  row("src/lib/pricing.ts", {
    depth: 2,
    quiet: false,
    mark: { letter: "M", status: "modified", counts: null, uncommitted: null },
    chips: [
      { key: "SHOP-29", tone: "working" },
      { key: "SHOP-19", tone: "working" },
    ],
  }),
  row("src/lib/vat.ts", {
    depth: 2,
    quiet: false,
    mark: { letter: "M", status: "modified", counts: "+3 −1", uncommitted: null },
    chips: [{ key: "SHOP-36", tone: "working" }],
    branch: "release/2.3",
  }),
  row("README.md"),
];

function body(page: FilesViewPage, rows: readonly FilesTreeRow[] = ROWS): string {
  return renderToStaticMarkup(
    createElement(FilesViewBody, {
      project: "SHOP",
      page,
      rows,
      open: new Set(["src"]),
      file: "src/lib/pricing.ts",
      found: null,
      query: "",
      onQuery: ignore,
      selectedMatch: -1,
      onOpenMatch: ignore,
      onRow: ignore,
      onFilter: ignore,
    }),
  );
}

describe("the project Files view (FIL-03, FIL-04, s28-03)", () => {
  it("draws the tree with chips, marks, folder counts and the other base's tag", () => {
    const out = body(PAGE);
    expect(out).toContain('<span class="files-changed-below">8 changed</span>');
    expect(out).toMatch(/data-struck="true".*legacy-price\.ts/);
    expect(out).toMatch(
      /pricing\.ts.*files-chip" data-tone="working">.*SHOP-29.*files-chip" data-tone="working">.*SHOP-19/,
    );
    expect(out).toContain('<span class="files-branch">release/2.3</span>');
    expect(out).toMatch(/aria-current="true".*pricing\.ts/);
    expect(out).toContain('data-quiet="true"');
  });

  it("draws the filters with their counts and the search box with Go to file's key", () => {
    const out = body(PAGE);
    for (const label of ["All", "Changed 11", "Needs you 6", "Same file 1"])
      expect(out).toContain(`>${label}</button>`);
    expect(out).toMatch(/role="radio" class="segment" aria-checked="true" tabindex="0">All</);
    expect(out).toContain('placeholder="Search files by name"');
    expect(out).toContain("⌥⌘P");
  });

  it("says when it is loading and when nothing is changing (s28-09)", () => {
    const loading = body({ ...PAGE, loading: "Loading the tickets’ changes…" });
    expect(loading).toContain("Loading the tickets’ changes…");
    const empty = body(
      {
        ...PAGE,
        source: { ...PAGE.source, subtitle: "the base branch · no ticket is changing files" },
        empty: {
          title: "No ticket is changing files right now.",
          text: "Start one from the board, or ask the Project agent.",
        },
      },
      [],
    );
    expect(empty).toContain("No ticket is changing files right now.");
    expect(empty).toContain("Start one from the board, or ask the Project agent.");
  });
});

const PICKER: FilesViewPicker = {
  placeholder: "Find a ticket or epic",
  sections: [
    {
      title: "BASE BRANCH",
      rows: [
        { pick: { kind: "base" }, key: null, title: "main · every ticket’s changes", detail: "the default" },
      ],
    },
    {
      title: "EPICS · THE EPIC’S BRANCH",
      rows: [
        {
          pick: { kind: "ticket", key: "SHOP-40" },
          key: null,
          title: "Wishlist · SHOP-40",
          detail: "40.1 merged · 40.2 on top",
        },
      ],
    },
    {
      title: "TICKETS · THEIR WORKTREE",
      rows: [
        {
          pick: { kind: "ticket", key: "SHOP-22" },
          key: "SHOP-22",
          title: "Fix flaky login redirect",
          detail: "Dev · 3 files",
        },
        {
          pick: { kind: "ticket", key: "SHOP-29" },
          key: "SHOP-29",
          title: "Coupon field",
          detail: "Dev · 2 files",
        },
      ],
    },
  ],
  no_worktree: "No worktree yet: SHOP-12 (Grill). A ticket gets its worktree when it reaches Dev.",
};

describe("the picker (FIL-07, s28-04)", () => {
  it("lists the base branch, the epics and the tickets, then the ones with no worktree", () => {
    const out = renderToStaticMarkup(
      createElement(PickerBody, { picker: PICKER, query: "", onQuery: ignore, onPick: ignore }),
    );
    expect(out).toMatch(/BASE BRANCH.*EPICS · THE EPIC’S BRANCH.*TICKETS · THEIR WORKTREE/);
    expect(out).toContain("40.1 merged · 40.2 on top");
    expect(out).toContain("No worktree yet: SHOP-12 (Grill).");
    expect(out).toContain('placeholder="Find a ticket or epic"');
  });

  it("filters by key or title as it is typed, dropping empty sections", () => {
    expect(pickerSections(PICKER, "").length).toBe(3);
    const coupon = pickerSections(PICKER, "coupon");
    expect(coupon.map((s) => s.title)).toEqual(["TICKETS · THEIR WORKTREE"]);
    expect(coupon[0]?.rows.map((r) => r.key)).toEqual(["SHOP-29"]);
    expect(pickerSections(PICKER, "shop-40")[0]?.rows[0]?.title).toBe("Wishlist · SHOP-40");
  });
});

describe("the view's state and keys (FIL-11)", () => {
  it("picking a file opens the folders above it on All", () => {
    const next = pickFile({ ...FIRST, filter: "needs_you" }, "src/lib/pricing.ts");
    expect(next.filter).toBe("all");
    expect(next.file).toBe("src/lib/pricing.ts");
    expect([...next.open]).toEqual(["src", "src/lib"]);
  });

  it("⌥⌘P asks for the view's search box, the picked tab's when one is picked, and leaves an open sheet to its tab", () => {
    const view = viewSearchFocus.get();
    expect(goToFile("SHOP", false)).toBe(true);
    expect(viewSearchFocus.get()).toBe(view + 1);
    viewStates.set(new Map([["SHOP", { ...FIRST, picked: "SHOP-29" }]]));
    const tab = searchFocus.get();
    expect(goToFile("SHOP", false)).toBe(true);
    expect(searchFocus.get()).toBe(tab + 1);
    expect(goToFile("SHOP", true)).toBe(false);
  });

  it("openFileIn opens the ticket's sheet on Files with the file (FIL-05's Open on Files)", () => {
    selection.set({ ...NOTHING_SELECTED, project: "SHOP" });
    openFileIn("SHOP-29", { path: "src/lib/pricing.ts", child: null, mode: "diff" });
    expect(selection.get()).toMatchObject({ ticket: "SHOP-29", tab: "files" });
  });
});
