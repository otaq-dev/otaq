// The demo's sample data, as in the canvas's Demo.dc.html script (decision 0102): two projects, their tickets, the
// named agents and the four Project agents. Texts are copied exactly; refresh them by hand when the canvas changes.

export type LineClass = "u" | "a" | "tool" | "o" | "d" | "ok" | "w" | "e" | "q" | "h" | "sp" | "tb";

/** One line of a terminal: its kind (the drawn `.tl` class) and its text. */
export interface Line {
  c: LineClass;
  t: string;
}

export const L = (c: LineClass, t: string): Line => ({ c, t });

export type ToolId = "CC" | "CX" | "CP" | "OC";
export type AgentName = "Mira" | "Theo" | "Pip" | "Vera" | "Otto";

export interface NamedAgent {
  tool: ToolId;
  toolName: string;
  model: string;
  use: string;
}

export const AGENTS: Record<AgentName, NamedAgent> = {
  Mira: { tool: "CC", toolName: "Claude Code", model: "opus", use: "hard features and tricky bugs" },
  Theo: { tool: "CX", toolName: "Codex", model: "gpt-5-codex", use: "everyday tickets" },
  Pip: { tool: "CC", toolName: "Claude Code", model: "haiku", use: "docs, typos and small config" },
  Vera: { tool: "CP", toolName: "Copilot", model: "gemini-2.5-pro", use: "checks everyone’s work" },
  Otto: { tool: "OC", toolName: "OpenCode", model: "any model", use: "whatever model you connect" },
};

export const AGENT_NAMES: readonly AgentName[] = ["Mira", "Theo", "Pip", "Vera", "Otto"];

export type ProjectKey = "NOTE" | "SHOP";

export interface Project {
  key: ProjectKey;
  name: string;
  branch: string;
  path: string;
  setup: string;
  run: string;
  instr: string;
}

export const PROJECTS: readonly Project[] = [
  {
    key: "NOTE",
    name: "pocket-notes",
    branch: "main",
    path: "~/code/pocket-notes",
    setup: "npm ci",
    run: "npm run dev -- --port $PORT",
    instr: "Run npm test before you submit. Keep output plain when it’s piped.",
  },
  {
    key: "SHOP",
    name: "shop-web",
    branch: "main",
    path: "~/code/shop-web",
    setup: "pnpm install",
    run: "pnpm dev --port $PORT",
    instr: "Use pnpm. Never edit prisma/migrations by hand.",
  },
];

export function project(key: ProjectKey): Project {
  const p = PROJECTS.find((x) => x.key === key);
  if (!p) throw new Error(`no project ${key}`);
  return p;
}

/** A diff line: its class (`hunk`, `add-l`, `del-l` or none), its line number and its text. */
export type DiffLine = readonly [c: "" | "hunk" | "add-l" | "del-l", n: string, t: string];

export interface DiffFile {
  name: string;
  a: number;
  d: number;
  lines: readonly DiffLine[];
}

export const NOTE4_DIFF: readonly DiffFile[] = [
  {
    name: "src/color.js",
    a: 14,
    d: 0,
    lines: [
      ["hunk", "", "@@ -0,0 +1,14 @@"],
      ["add-l", "1", "const useColor = () =>"],
      ["add-l", "2", "  process.stdout.isTTY && !process.env.NO_COLOR;"],
      ["add-l", "3", ""],
      ["add-l", "4", "const dim = (text) =>"],
      ["add-l", "5", "  useColor() ? `\\x1b[2m${text}\\x1b[22m` : text;"],
      ["add-l", "6", ""],
      ["add-l", "7", "module.exports = { useColor, dim };"],
    ],
  },
  {
    name: "src/cli.js",
    a: 4,
    d: 2,
    lines: [
      ["hunk", "", "@@ -1,6 +1,7 @@ const store = require('./store');"],
      ["", "1", "const store = require('./store');"],
      ["add-l", "2", "const { dim } = require('./color');"],
      ["hunk", "", "@@ -41,7 +42,8 @@ function ls(notes) {"],
      ["", "42", "  for (const note of notes) {"],
      ["del-l", "43", "    console.log(`${note.id}  ${note.text}`);"],
      ["add-l", "43", "    const id = dim(String(note.id).padStart(3));"],
      ["add-l", "44", "    console.log(`${id}  ${note.text}`);"],
      ["", "45", "  }"],
    ],
  },
  {
    name: "README.md",
    a: 3,
    d: 0,
    lines: [
      ["hunk", "", "@@ -18,3 +18,6 @@ ## Usage"],
      ["add-l", "21", "## Colors"],
      ["add-l", "22", ""],
      ["add-l", "23", "Set NO_COLOR to turn colors off."],
    ],
  },
];

/** The diff every other ticket shows once it has changes. */
export function genericDiff(title: string): DiffFile[] {
  return [
    {
      name: "src/cli.js",
      a: 18,
      d: 4,
      lines: [
        ["hunk", "", "@@ -12,6 +12,20 @@"],
        ["", "12", "function main(argv) {"],
        ["del-l", "13", "  const [cmd, ...rest] = argv;"],
        ["add-l", "13", "  const [cmd, ...rest] = parseArgs(argv);"],
        ["add-l", "14", "  // " + title],
        ["", "15", "  switch (cmd) {"],
      ],
    },
    {
      name: "test/cli.test.js",
      a: 22,
      d: 0,
      lines: [
        ["hunk", "", "@@ -40,0 +41,22 @@"],
        ["add-l", "41", "test('" + title.replace(/'/g, "") + "', () => {"],
        ["add-l", "42", "  expect(run(argv)).toMatchSnapshot();"],
        ["add-l", "43", "});"],
      ],
    },
  ];
}

export type Column = "backlog" | "ready" | "dev" | "review" | "done";
export type TicketStatus =
  "queued" | "starting" | "working" | "attn" | "error" | "validating" | "review" | "merging" | "done";

export type Answer =
  | "imp-open"
  | "imp-closed"
  | "note8"
  | "note8b"
  | "shop22"
  | "shop22b"
  | "shop25"
  | "shop-create"
  | "shop-no";

export interface Option {
  label: string;
  rec: string;
  act: Answer;
}

/** What a ticket's agent waits for: a question, a permission prompt, or an interrupted session. */
export interface TicketAsk {
  kind: "ask" | "perm" | "err";
  short: string;
  title: string;
  opts: readonly Option[];
}

export interface HistoryEntry {
  at: string;
  t: string;
}

export type Priority = 1 | 2 | 3 | 4;

export interface Ticket {
  k: string;
  p: ProjectKey;
  title: string;
  col: Column;
  st: TicketStatus;
  prio: Priority;
  agent: AgentName;
  log: readonly Line[];
  waits: readonly string[];
  acc: readonly string[];
  hist: readonly HistoryEntry[];
  round: number;
  tokens: string;
  desc?: string;
  epic?: string;
  epicPos?: string;
  age?: string;
  ask?: TicketAsk | null;
  decisions?: number;
  decision?: string;
  adds?: number;
  dels?: number;
  diffKey?: "NOTE-4";
  conflict?: boolean;
  fresh?: boolean;
  by?: string;
  stage?: "worktree" | "setup";
}

type TicketInit = Omit<Ticket, "log" | "waits" | "acc" | "hist" | "round" | "tokens" | "prio" | "agent"> &
  Partial<Pick<Ticket, "log" | "waits" | "acc" | "hist" | "round" | "tokens" | "prio" | "agent">>;

/** A ticket with the canvas's defaults (`T()` in its script). */
export function T(o: TicketInit): Ticket {
  return { log: [], waits: [], acc: [], hist: [], round: 1, tokens: "", prio: 3, agent: "Theo", ...o };
}

export function initialTickets(): Ticket[] {
  return [
    T({
      k: "NOTE-6",
      p: "NOTE",
      title: "Sync notes through a GitHub gist",
      col: "backlog",
      st: "queued",
      prio: 4,
      agent: "Mira",
      desc: "Keep notes in step across two laptops by syncing the notes file to a private GitHub gist. Uses the gh login that’s already on the machine.",
      acc: [
        "notes sync pushes and pulls the notes file",
        "A conflict keeps both versions and says so",
        "Works without a token file: it uses gh auth",
      ],
    }),
    T({
      k: "NOTE-11",
      p: "NOTE",
      title: "Shell completions for bash and zsh",
      col: "backlog",
      st: "queued",
      prio: 4,
      agent: "Pip",
      desc: "notes completions bash|zsh prints a completion script for the subcommands and their flags.",
      acc: ["Completes subcommands", "Completes -n with notebook names"],
    }),
    T({
      k: "NOTE-12",
      p: "NOTE",
      title: "Fuzzy matching in notes search",
      col: "backlog",
      st: "queued",
      prio: 3,
      desc: "notes search should find “grocery” when you type “grcry”. Exact matches stay on top.",
      acc: ["Exact matches first", "Typos of up to 2 letters still match"],
    }),
    T({
      k: "NOTE-13.4",
      p: "NOTE",
      title: "Document notebooks in the README",
      col: "backlog",
      st: "queued",
      prio: 3,
      agent: "Pip",
      epic: "Notebooks",
      epicPos: "4 of 4",
      waits: ["NOTE-13.2", "NOTE-13.3"],
      desc: "A short README section on notebooks: creating one, adding to it, listing.",
      acc: ["Examples match the real output"],
    }),
    T({
      k: "NOTE-13.3",
      p: "NOTE",
      title: "notes ls -n and notes notebooks",
      col: "ready",
      st: "queued",
      prio: 2,
      agent: "Mira",
      epic: "Notebooks",
      epicPos: "3 of 4",
      waits: ["NOTE-13.2"],
      desc: "List the notes of one notebook with -n, and list the notebooks with their note counts.",
      acc: ["notes ls -n work shows only that notebook", "notes notebooks shows each notebook and its count"],
    }),
    T({
      k: "NOTE-9",
      p: "NOTE",
      title: "Pin notes to the top of notes ls",
      col: "ready",
      st: "queued",
      prio: 3,
      waits: ["NOTE-8"],
      desc: "notes pin <id> keeps a note at the top of notes ls. It builds on notes edit, which NOTE-8 adds.",
      acc: ["Pinned notes are listed first", "notes unpin <id> undoes it"],
    }),
    T({
      k: "NOTE-3",
      p: "NOTE",
      title: "Show each note’s age in notes ls",
      col: "ready",
      st: "queued",
      prio: 3,
      agent: "Pip",
      desc: "Add an age column (“3d”, “2h”) to notes ls so old notes stand out.",
      acc: ["Ages under a minute show “now”", "The column lines up"],
    }),
    T({
      k: "NOTE-13.2",
      p: "NOTE",
      title: "notes add -n <notebook>",
      col: "dev",
      st: "working",
      prio: 2,
      agent: "Mira",
      epic: "Notebooks",
      epicPos: "2 of 4",
      waits: ["NOTE-13.1"],
      tokens: "38k tokens",
      age: "active 6s ago",
      desc: "Add a note to a named notebook. Notes without -n go to the default notebook, which NOTE-13.1 set up.",
      acc: [
        "notes add -n work “text” stores the note in work",
        "An unknown notebook is created on first use",
      ],
      log: [
        L("d", "Claude Code · Mira · opus · ~/.otaq/wt/pocket-notes/NOTE-13.2"),
        L("tool", "otaq show NOTE-13.2"),
        L(
          "a",
          "Adding -n <notebook> to notes add. Notes without -n go to the default notebook, as NOTE-13.1 set up.",
        ),
        L("tool", "Read src/store.js"),
        L("tool", "Edit src/cli.js  +24 −3"),
        L("tool", "Bash npm test"),
        L("o", "running 11 tests…"),
      ],
    }),
    T({
      k: "NOTE-8",
      p: "NOTE",
      title: "notes edit <id> <text>",
      col: "dev",
      st: "attn",
      prio: 2,
      agent: "Theo",
      tokens: "51k tokens",
      age: "asked 2m ago",
      ask: {
        kind: "ask",
        short: "asks: open $EDITOR when no text is given?",
        title: "The agent asks you",
        opts: [
          { label: "Open $EDITOR with the current text", rec: "Recommended", act: "note8" },
          { label: "Fail with a usage message", rec: "", act: "note8b" },
        ],
      },
      desc: "Change the text of a note in place. Keeps the id and the date it was created.",
      acc: [
        "notes edit 4 “new text” replaces the text",
        "The id and date stay the same",
        "An unknown id fails with a clear message",
      ],
      log: [
        L("d", "Codex · Theo · gpt-5-codex · ~/.otaq/wt/pocket-notes/NOTE-8"),
        L("tool", "otaq show NOTE-8"),
        L("tool", "Read src/cli.js, src/store.js"),
        L(
          "a",
          "notes edit 4 “new text” now replaces the text of note 4 and keeps its id and date. Added to src/cli.js with a test.",
        ),
        L("a", "One thing the ticket doesn’t settle:"),
        L("tool", "otaq ask human"),
        L(
          "q",
          "Should notes edit 4 with no text open $EDITOR with the current text, or fail with a usage message?",
        ),
        L("d", "Recommended: open $EDITOR. git commit does the same, so it’s what people expect."),
      ],
    }),
    T({
      k: "NOTE-10",
      p: "NOTE",
      title: "Undo the last delete",
      col: "dev",
      st: "validating",
      prio: 3,
      agent: "Theo",
      tokens: "64k tokens",
      desc: "notes undo restores the last deleted note, with its id.",
      acc: ["Restores the note and its id", "Only the last delete can be undone"],
      log: [
        L("d", "Codex · Theo · gpt-5-codex"),
        L("tool", "Edit src/store.js  +31 −2"),
        L("tool", "Bash npm test"),
        L("ok", "✓ 12 tests pass"),
        L("tool", "otaq submit"),
        L("a", "Submitted NOTE-10. Waiting for validation."),
        L("sp", ""),
        L("d", "Validator · Vera · Copilot · gemini-2.5-pro · round 1"),
        L("d", "Checking 2 acceptance criteria against the change…"),
      ],
    }),
    T({
      k: "NOTE-4",
      p: "NOTE",
      title: "Colorize output when stdout is a terminal",
      col: "review",
      st: "review",
      prio: 4,
      agent: "Mira",
      tokens: "25k tokens",
      decisions: 1,
      adds: 21,
      dels: 2,
      diffKey: "NOTE-4",
      decision:
        "The ticket also asks to dim the age column, but notes ls doesn’t show ages yet (NOTE-3 adds them). So only the id is dimmed, and NOTE-3 can wrap its column in the same dim(). The other option: add the age column in this ticket too.",
      desc: "Dim the less important columns of notes ls when output goes to a terminal, so the note text stands out. Piped output stays plain.",
      acc: [
        "Dims the id column on a terminal",
        "Plain when piped or when NO_COLOR is set",
        "The README mentions NO_COLOR",
      ],
      hist: [
        { at: "10:02 am", t: "Moved to Dev by you" },
        { at: "10:02 am", t: "Worktree created · setup npm ci (6s)" },
        { at: "10:11 am", t: "Submitted by the agent · 1 decision recorded" },
        { at: "10:12 am", t: "Validation round 1 passed" },
      ],
      log: [
        L("a", "I’ve submitted NOTE-4 for review."),
        L("sp", ""),
        L(
          "o",
          "notes ls now dims the id column when output goes to a terminal. Piped, or with NO_COLOR set, it stays plain.",
        ),
        L("o", "· Code: a new src/color.js with useColor() and dim(); src/cli.js uses dim() on the id."),
        L("o", "· Docs: the README says that NO_COLOR turns colors off."),
        L("o", "· Checks: the 4 tests pass, including new ones for the terminal and NO_COLOR cases."),
        L("sp", ""),
        L("h", "Decision for you (NOTE-4#1)"),
        L(
          "o",
          "The ticket also asks to dim the age column, but notes ls doesn’t show ages yet; NOTE-3 adds them. So I only dimmed the id. NOTE-3 can wrap its column in the same dim().",
        ),
        L("sp", ""),
        L("d", "✻ Done in 43s"),
        L("ok", "✓ Validator · round 1 passed: 3 of 3 acceptance checks met"),
      ],
    }),
    T({
      k: "NOTE-5",
      p: "NOTE",
      title: "Export notes to Markdown",
      col: "review",
      st: "review",
      prio: 3,
      round: 2,
      conflict: true,
      agent: "Theo",
      tokens: "88k tokens",
      adds: 64,
      dels: 9,
      desc: "notes export writes all notes to one Markdown file, grouped by day.",
      acc: ["One heading per day", "Writes to stdout unless -o is given"],
      log: [
        L("a", "Submitted NOTE-5 again after round 1’s finding: export now writes to stdout by default."),
        L("ok", "✓ Validator · round 2 passed"),
        L(
          "w",
          "Conflicts with main: NOTE-2 changed src/cli.js after this ticket started. Otaq rebases before merging, and the agent resolves conflicts on its own.",
        ),
      ],
    }),
    T({
      k: "NOTE-2",
      p: "NOTE",
      title: "Add notes search <text>",
      col: "done",
      st: "done",
      prio: 2,
      agent: "Theo",
      log: [L("ok", "Merged into main · squash 4f1c9e2")],
    }),
    T({
      k: "NOTE-13.1",
      p: "NOTE",
      title: "Store notes in notebooks",
      col: "done",
      st: "done",
      prio: 2,
      agent: "Mira",
      epic: "Notebooks",
      epicPos: "1 of 4",
      log: [L("ok", "Merged into otaq/NOTE-13 · squash 9a07b1d")],
    }),
    T({
      k: "NOTE-1",
      p: "NOTE",
      title: "Note ids are reused after a delete",
      col: "done",
      st: "done",
      prio: 2,
      agent: "Mira",
      log: [L("ok", "Merged into main · squash c22d810")],
    }),
    T({
      k: "NOTE-7",
      p: "NOTE",
      title: "Read the notes file path from $NOTES_FILE",
      col: "done",
      st: "done",
      prio: 4,
      agent: "Pip",
      log: [L("ok", "Merged into main · squash 1b8e3aa")],
    }),

    T({
      k: "SHOP-31",
      p: "SHOP",
      title: "Lazy-load product images below the fold",
      col: "backlog",
      st: "queued",
      prio: 3,
      desc: "Only the first row of product images loads eagerly.",
      acc: ["Largest Contentful Paint doesn’t get worse"],
    }),
    T({
      k: "SHOP-33",
      p: "SHOP",
      title: "Remove the unused coupon banner",
      col: "backlog",
      st: "queued",
      prio: 4,
      agent: "Pip",
      desc: "The coupon banner component isn’t rendered anywhere since the spring sale.",
      acc: ["Component, styles and tests removed"],
    }),
    T({
      k: "SHOP-21",
      p: "SHOP",
      title: "Show saved cards at checkout",
      col: "ready",
      st: "queued",
      prio: 2,
      agent: "Mira",
      waits: ["SHOP-18"],
      desc: "Signed-in customers pick a saved card instead of typing it again. Needs the Checkout API from SHOP-18.",
      acc: ["Lists saved cards", "A new card can still be added"],
    }),
    T({
      k: "SHOP-22",
      p: "SHOP",
      title: "Migrate orders to the new status enum",
      col: "dev",
      st: "attn",
      prio: 1,
      agent: "Mira",
      tokens: "72k tokens",
      ask: {
        kind: "perm",
        short: "permission: Allow Bash(pnpm prisma migrate dev)?",
        title: "Permission prompt",
        opts: [
          { label: "Allow once", rec: "", act: "shop22" },
          { label: "Deny", rec: "", act: "shop22b" },
        ],
      },
      desc: "Replace the free-text order status with an enum, and migrate existing orders.",
      acc: ["A migration converts existing rows", "Unknown statuses become “pending” and are logged"],
      log: [
        L("d", "Claude Code · Mira · opus · ~/.otaq/wt/shop-web/SHOP-22"),
        L("tool", "Edit prisma/schema.prisma  +9 −1"),
        L("a", "Schema updated. Creating the migration next."),
        L("q", "Allow Bash(pnpm prisma migrate dev --name order-status)?"),
      ],
    }),
    T({
      k: "SHOP-19",
      p: "SHOP",
      title: "Cache the product list for 60 seconds",
      col: "dev",
      st: "working",
      prio: 3,
      agent: "Theo",
      tokens: "29k tokens",
      age: "active 3s ago",
      desc: "The product list is the same for everyone for a minute at a time.",
      acc: ["Cached for 60 seconds", "Admin edits clear the cache"],
      log: [
        L("d", "Codex · Theo · gpt-5-codex"),
        L("tool", "Read app/products/page.tsx"),
        L(
          "a",
          "Using the framework’s fetch cache with revalidate: 60, and a cache tag that admin edits clear.",
        ),
        L("tool", "Edit lib/products.ts  +12 −4"),
      ],
    }),
    T({
      k: "SHOP-25",
      p: "SHOP",
      title: "Fix flaky login redirect",
      col: "dev",
      st: "error",
      prio: 2,
      agent: "Mira",
      tokens: "17k tokens",
      ask: {
        kind: "err",
        short: "session interrupted",
        title: "Session interrupted",
        opts: [{ label: "Resume the session", rec: "Continues the same conversation", act: "shop25" }],
      },
      desc: "After signing in, about 1 in 20 people land on the home page instead of the page they came from.",
      acc: ["The return URL survives the OAuth round trip"],
      log: [
        L("d", "Claude Code · Mira · opus"),
        L("tool", "Read app/auth/callback/route.ts"),
        L("e", "Session interrupted: the computer went to sleep while the agent was working."),
      ],
    }),
    T({
      k: "SHOP-18",
      p: "SHOP",
      title: "Checkout API",
      col: "review",
      st: "review",
      prio: 1,
      agent: "Mira",
      tokens: "182k tokens",
      decisions: 2,
      adds: 412,
      dels: 38,
      decision:
        "Prices are recomputed on the server at checkout and the client total is ignored. The other option was to trust the cart total and only check it, which is faster but lets a stale cart through.",
      desc: "POST /api/checkout takes a cart, checks stock, creates the order and returns a payment intent.",
      acc: [
        "Out-of-stock items fail with 409 and name the item",
        "Totals are computed on the server",
        "Covered by integration tests",
      ],
      log: [
        L("a", "Submitted SHOP-18 for review."),
        L("o", "· POST /api/checkout with stock checks and server-side totals."),
        L("o", "· 14 integration tests, all passing."),
        L("h", "2 decisions recorded (see the Ticket tab)"),
        L("ok", "✓ Validator · round 1 passed: 3 of 3 acceptance checks met"),
      ],
    }),
    T({
      k: "SHOP-12",
      p: "SHOP",
      title: "Sign in with GitHub",
      col: "done",
      st: "done",
      prio: 2,
      agent: "Mira",
      log: [L("ok", "Merged through pull request #48")],
    }),
    T({
      k: "SHOP-14",
      p: "SHOP",
      title: "Order confirmation email",
      col: "done",
      st: "done",
      prio: 3,
      agent: "Theo",
      log: [L("ok", "Merged into main · squash 7d3a910")],
    }),
  ];
}

export type ProjectAgentStatus = "attn" | "working" | "idle" | "new";

export interface ProjectAgent {
  id: string;
  p: ProjectKey;
  name: string;
  st: ProjectAgentStatus;
  tokens: string;
  created: readonly string[];
  lines: readonly Line[];
  ask?: { q: string; opts: readonly Option[] } | null;
}

export function initialAgents(): ProjectAgent[] {
  return [
    {
      id: "a1",
      p: "NOTE",
      name: "Import GitHub issues",
      st: "attn",
      tokens: "14k tokens",
      created: [],
      lines: [
        L("u", "❯ /import-github label:ready"),
        L("tool", "Bash gh issue list --label ready --state open"),
        L("o", "#31  Show a hint when the notes file is missing"),
        L("o", "#34  Read notes from stdin"),
        L("a", "2 open issues have the ready label. 3 closed issues have it too."),
      ],
      ask: {
        q: "Import closed issues too?",
        opts: [
          { label: "Only open issues", rec: "Recommended", act: "imp-open" },
          { label: "Include closed ones", rec: "", act: "imp-closed" },
        ],
      },
    },
    { id: "a2", p: "NOTE", name: "New agent", st: "new", tokens: "0 tokens", created: [], lines: [] },
    {
      id: "a3",
      p: "SHOP",
      name: "Checkout slowness",
      st: "attn",
      tokens: "96k tokens",
      created: [],
      lines: [
        L("u", "❯ Why is checkout slow? Make tickets for what you find."),
        L("tool", "Read app/checkout/page.tsx"),
        L("tool", "Bash node scripts/profile-checkout.mjs"),
        L("o", "GET /api/cart       412 ms"),
        L("o", "GET /api/shipping   1.9 s   3 carrier calls, one after another"),
        L("o", "hydration           640 ms"),
        L("a", "Most of the time is the shipping quote. I’d make 3 tickets:"),
        L("sp", ""),
        L("tb", "#  Title                                  Agent"),
        L("tb", "1  Fetch carrier quotes in parallel        Theo"),
        L("tb", "2  Cache shipping quotes per cart, 5 min   Theo"),
        L("tb", "3  Load the reviews widget after paint     Pip"),
      ],
      ask: {
        q: "Create these 3 tickets in Backlog?",
        opts: [
          { label: "Create them", rec: "", act: "shop-create" },
          { label: "Not yet", rec: "", act: "shop-no" },
        ],
      },
    },
    { id: "a4", p: "SHOP", name: "New agent", st: "new", tokens: "0 tokens", created: [], lines: [] },
  ];
}
