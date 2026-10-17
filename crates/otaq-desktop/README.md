# otaq-desktop

The Otaq desktop app on Tauri 2: a window whose page is React with TypeScript, built with Vite (decisions 0074,
0075; SDD §5.5). Its binary is `otaq-app`, the name the GPUI app had (T-0245), so `otaq` with no arguments
opens it and the release archive keeps its layout (d0072).

> References such as "decision 0075", "d0072", "SDD §5.5", ticket numbers (T-0215), requirement IDs (UIT-02)
> and design frames (`crewboard.pen`) point to the maintainer's design notes, which are not part of this repository.

## Where things are

The frontend sits inside this crate (the implementation choice SDD §5.5 leaves open), so one folder holds the whole app
and every `npm` command runs from here:

| Path                                                                                                      | What                                                 |
| --------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| `Cargo.toml`, `build.rs`, `src/`                                                                          | the Rust side                                        |
| `src/door.rs`                                                                                             | the only Rust module that names Tauri (UIT-02)       |
| `src/window.rs`                                                                                           | the window: title bar, minimum size, its kept place  |
| `tauri.conf.json`, `capabilities/`, `icons/`                                                              | Tauri's config: the CSP, the window's permissions    |
| `package.json`, `package-lock.json`, `.nvmrc`                                                             | the npm packages and the Node version (24)           |
| `tsconfig.json`, `tsconfig.node.json`, `eslint.config.ts`, `types/`, `.prettierrc.json`, `vite.config.ts` | the TypeScript rules (UIT-08) and the build          |
| `ui/index.html`, `ui/src/`                                                                                | the page                                             |
| `ui/src/tauri.ts`                                                                                         | the only TypeScript file that names Tauri (UIT-02)   |
| `ui/src/generated/`                                                                                       | the types shared with Rust, generated with `ts-rs`   |
| `ui/src/app/`                                                                                             | the frame, the registries, the shared selection      |
| `ui/src/components/`                                                                                      | the shared components every screen draws with        |
| `ui/src/cards/`                                                                                           | the Ticket Card and the Epic Card                    |
| `ui/src/screens/`                                                                                         | one folder per screen, and the screens index         |
| `ui/src/terminal/`, `src/terminal.rs`                                                                     | the terminal component every terminal screen uses    |
| `src/api/`                                                                                                | the request table and one file per area              |
| `ui/dist/`                                                                                                | the built page (git-ignored), Tauri's `frontendDist` |

The app icons in `icons/` come from the design's _App Icon 96_ (Command tent, d0076): exported from `crewboard.pen`, put
on a 1024 px canvas (macOS: an 824 px body; Windows and Linux: a tighter crop) and run through `npm run tauri icon`.

## Commands

Use Node 24 (`nvm use` in this folder reads `.nvmrc`). Nothing is installed globally; the Tauri CLI is the
`@tauri-apps/cli` dev dependency.

```sh
npm ci                  # install the locked packages
npm run tauri dev       # the app with the Vite dev server (live reload)
npm run tauri build     # a release build with the page inside the binary: target/release/otaq-app
npm run check           # typecheck, lint, format:check, test (each also runs on its own)
```

## How a screen plugs in

The page is a fixed frame (`ui/src/app/Frame.tsx`, _02 · App shell → Window anatomy_) whose parts are filled by
screens. A screen touches only its own folders plus **one line in the screens index** and, if it calls the service,
**one line in the request table**. Everything it can fill is listed, typed, in the registries (`ui/src/app/registry/`):

| Registry                          | Ids (fixed in)                                                                              | A screen registers                                 |
| --------------------------------- | ------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| `app.slots`                       | `slots.ts`: title bar start/end, header, header search, rail, banners, board, panel, toasts | the component that fills the slot                  |
| `app.overlays`                    | `overlays.ts`: every dialog, popover and drawer, with what it is opened with                | its overlay's component (gets `params`, `close`)   |
| `app.panelTabs`                   | the generated `PanelTab` (Rust `panel::Tab`), in the order of `panel.ts`                    | its tab's content (gets the open `ticket`)         |
| `app.panelSections`               | `panel.ts`: Stack, Messages; `inbox_run`, the ⌘J run bar on top of the panel                | its section (gets the open `ticket`)               |
| `app.settings.global`, `.project` | `settings.ts`: the global and the project sections, in nav order                            | its section (a project section gets the `project`) |
| `app.shortcuts`                   | the generated `KeyAction` (Rust keymap)                                                     | a handler per action, `app.shortcuts.on(...)`      |

Each id has one owner: registering one twice throws. The ids are unions, so a `switch` over them without a `default`
is checked by ESLint's `switch-exhaustiveness-check`. A new overlay id, settings section or slot is one line in its
registry file; a new panel tab or key action comes from Rust (regenerate the types).

**1. The screen's folder.** `ui/src/screens/<area>/index.tsx` exports one `Screen`, a function that registers its
parts; its components and logic sit next to it. `ui/src/screens/_sample/` is the template:

```tsx
import { toggleOverlay } from "../../app/overlays";
import type { Screen } from "../../app/registry";

export const inbox: Screen = (app) => {
  app.overlays.register("inbox", { Component: Inbox });
  app.shortcuts.on("open_inbox", () => {
    toggleOverlay("inbox");
  });
};
```

**2. One line in the screens index**, `ui/src/screens/index.ts`: `export { inbox } from "./inbox";`. The page runs
every screen exported there once at start.

**3. One line in the request table**, if it calls the service: a file `src/api/<area>.rs` with
`pub async fn request(app, method, params)` matching its own full method names (`"inbox.list" => list(app,
params).await`) and ending with `_ => Err(unknown(method))`, plus its name in the `areas!` list of `src/api/mod.rs`.
The page calls it with `request("inbox.list", params)` from `ui/src/tauri.ts`. Its reply types get the `TS` derive and
one line in `export_all` (below).

**Shared state.** Interaction state screens share lives in two small stores; everything else comes from the service
(`useServiceView` in `ui/src/service.ts`) or stays inside the component (UIT-04):

- `ui/src/app/selection.ts`: the project on screen, the ticket open in the panel and its tab, the focused terminal.
  Read it with `useSelection()`; change it with `selectProject`, `openTicket`, `closeTicket`, `showTab`,
  `focusTerminal`. The panel slot shows while a ticket is open.
- `ui/src/app/overlays.ts`: the open overlays, newest last. Any screen opens any overlay by id: `openOverlay("inbox")`,
  `openOverlay("edit_ticket", { ticket: "SHOP-22" })`, `toggleOverlay`, `closeOverlay`. An overlay that isn't
  registered yet draws nothing. `modalOpen()` says whether a modal one holds the app's shortcuts back (`modal.rs`).

**Shortcuts.** T-0215's dispatcher turns a key into a `KeyAction` and calls `app.shortcuts.run(action)`; the newest
handler for that action runs. A handler returns `false` to leave the key to the one registered before it (an open
overlay's Esc before the panel's). A component whose key acts on it while mounted (Find in a focused terminal) uses
`useShortcut(name, handler)` instead of registering at start.

**Keys.** The dispatcher (`ui/src/app/keys.ts`, installed by `App`) takes the §21.3 map from Rust (`keymap.map`, built
from `otaq-view`'s keymap), so no TypeScript file lists a shortcut. While the selection's `focusedTerminal` is set,
every key goes to the terminal except the app's (⌘ on macOS, Ctrl+Shift on Windows and Linux); a terminal component
calls `focusTerminal(session)` on focus and `focusTerminal(null)` on blur. Leave terminal focus blurs it when no screen
handles it. Outside a terminal, bound keys that no handler takes, and keys the webview would act on (reload, print,
find, zoom, devtools, caret browsing, history), are swallowed; a focused text field keeps Enter, Backspace and Esc. The
app started with `OTAQ_KEY_LOG=1` shows the last keys and where each went, bottom right.

**Testing a screen.** `registerScreens({ inbox })` gives fresh registries with only that screen; `registered(app)`
lists what it filled (see `ui/src/app/registry/registry.test.ts`). The frame renders without a DOM through
`react-dom/server` (`ui/src/app/frame.test.ts`).

## The terminal component

`ui/src/terminal/Terminal.tsx` is the one terminal of the app (T-0216, UIT-05, SES-09): the Terminal tab, the Shell
tab, Project terminals and Project agents all show a session with it. It is **xterm.js 6** with its WebGL, fit, search
and web-links add-ons, inside the frame (it needs the registries' context for its Find shortcut):

```tsx
import { Terminal, type TerminalHandle } from "../../terminal/Terminal";

const terminal = useRef<TerminalHandle>(null);
<Terminal
  session={session.id} // another id attaches anew
  label="SHOP-19 · agent terminal" // its accessible name
  readOnly={false} // true: output only, nothing typed is sent (Run tab)
  onState={setTerminalState} // phase, focused, scrolledUp, newLines, fontSize: for the header and hint strips
  ref={terminal} // terminal.current?.focus() / .openFind() / .jumpToLatest() / .changeFontSize(±1)
/>;
```

It fills the box its host gives it: the find bar, the terminal in the theme's terminal colours, the "142 new lines"
pill while scrolled up, and the "Reconnecting… agents keep running" strip. The strips around it (session state, path,
attach command, the hint line under it) are the host's, drawn from `onState` (`TerminalState` in `state.ts`).

- **Output.** It attaches over its own door channel (`attachTerminal` in `ui/src/tauri.ts` → `attach_terminal` in
  `src/door.rs` → `terminal::attach`). Rust subscribes to the service's events before it asks for the attach, sends the
  stored scrollback (`replay`), then live output cut to start exactly where the shown output ends (`live`), so no byte
  shows twice or goes missing; and the `phase` (`attaching`, `live`, `reconnecting`, `ended`, `no_session`). After a
  reconnect, or when live output went missing, Rust attaches again and the terminal redraws from the new replay. It
  detaches on unmount (`terminal.detach`).
- **Input.** What is typed or pasted goes out with `terminal.input`, one request at a time and in order with the
  resizes (`createSender` in `input.ts`). The terminal's own answers to the program (cursor reports, colours, mouse
  reports) are sent as replies, so they don't touch the service's unsent-input flag (d0048); answers to questions in a
  replay aren't sent at all. Input is off unless the phase is `live`, so while reconnecting the scrollback stays and
  nothing is typed (SVC-04).
- **Focus.** On focus it calls `focusTerminal(session)` and tells the service (`terminal.focus`, passed on for an
  agent's session only, ATT-03); on blur `focusTerminal(null)`. Keys go to xterm.js untouched except the app's own
  (T-0215) and the terminal's own from the Rust map (`keymap.map`'s `terminal_keys`): copy and paste on Ctrl+Shift+C /
  V on Windows and Linux (on macOS the Edit menu's ⌘C / ⌘V reach xterm.js), and ⌘+ / ⌘− (Ctrl+Shift+= / −) for this
  terminal's font size until the setting changes. ⌘F / Ctrl+Shift+F opens the find bar (`useShortcut`).
- **Settings.** `terminal.settings` gives the font size (8–32) and the renderer, asked again on `settings_changed`. The
  renderer (`terminal_renderer` in the global settings: WebGL or DOM; DOM by default on Linux, WebGL on macOS and
  Windows, Q37) picks xterm.js's renderer for every terminal; only a WebGL context that fails to start falls back to
  DOM, because a broken driver that draws garbage raises no error. Its row in Settings → Appearance is T-0234's (Q38).
- **Theme.** The colours are `terminalColors(...)` (T-0213), read again whenever the frame swaps `data-theme`.
- **Links.** A clicked `http(s)` link opens in the system browser (`terminal.open_link`).

## Components

The shared components (_crewboard.pen → 01 · Components_, T-0217) are in `ui/src/components/`; a screen imports them
from the folder, never from a file inside it: `import { Button, ConfirmDialog, StatusPill } from "../../components";`.
They are drawn with the theme's CSS variables only (`components.css`) and take typed props: what a view model says
comes in as its generated type (`PillView`, `PriorityView`, `MenuItem`, `ThemePickerSection`), and the component keeps
only the moment of interaction (UIT-04). Their keyboard and placement rules are plain functions in `logic.ts`, tested
without a DOM.

| Component                                                       | Props, briefly                                                                       |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `StatusPill`, `MetaChip`, `CountBadge`, `Kbd`                   | a `PillView` (`status.rs`); icon + label + tone; count + tone; the keys' text        |
| `Button`, `IconButton`                                          | label, variant, icon, `compact`, `disabled`, `loading` (ignores presses), `onClick`  |
| `PriorityChip`, `PriorityLabel`                                 | a `PriorityView` (`priority.rs`)                                                     |
| `Tabs`, `Segmented`, `Toggle`                                   | items with icon, label, indicator; options; on/off. ←/→ move between them            |
| `Field`, `TextInput`, `TextArea`, `Select`, `Checkbox`, `Radio` | label, hint, error (`fields.rs` says which shows); `Select` takes `MenuItem`s        |
| `Menu`, `AnchoredMenu`, `Popover`, `Tooltip`                    | `MenuItem`s (`menu.rs` rules: arrows skip disabled rows and wrap); an anchor         |
| `Dialog`, `ConfirmDialog`                                       | title, body, confirm label, `destructive` (↵ = Cancel); traps focus, Esc cancels     |
| `Banner`, `Toast`, `StackList`, `StackRow`                      | tone, icon, title, text, actions; the Stack list's row shell                         |
| `ThemePicker`, `useThemePicker`, `pickTheme`                    | the picker's sections (`theme.picker`); a click applies and saves (`theme.pick`)     |
| `OtaqMark`                                                      | `size` (px), optional `label`; the Command tent (d0076), thicker strokes below 24 px |

Any screen asks for a confirmation without drawing one: `openOverlay("confirm", { title, body, confirmLabel,
destructive, onConfirm })`; the components screen (`ui/src/screens/components/`) draws it with `ConfirmDialog`.
Menus, popovers and tooltips are fixed-position layers inside their anchor's subtree, so a scrolling parent never cuts
them and the anchor's theme applies.

**The gallery.** Start the app with `OTAQ_GALLERY=1` (`OTAQ_GALLERY=1 npm run tauri dev`) and the design
gallery opens over it: every foundation and component next to each other, the token swatches of Otaq Light and
Dark, the Status Legend, the keyboard map, the theme picker (it saves) and a palette switch over the 15 themes (it
doesn't). Esc or its close button leaves it. A new shared component gets a place in it
(`ui/src/screens/components/Gallery.tsx`).

## Cards

The Ticket Card and the Epic Card (_04 · Ticket card variants_, T-0218; Ticket Card v2 since T-0322, SHL-07) are in
`ui/src/cards/`: `import { EpicCard, TicketCard, stateOfLook } from "../../cards";`. They draw a `TicketCardView` /
`EpicCardView`, which `otaq_view::view_models::card_page` builds from the card view models with every text already
worded: the head flag (the highest of the flag order, ≤ 12 characters), the situation line (its edge colour, mono or
not, the hovers of the lead's markers), the origin line, the footer (tokens, last signal, one action), the hover's full
status, the quick actions, the compact answer options, "Add ticket to epic". The board passes the moment of interaction
as a `CardState` (selected, chain highlighted or dimmed, dragging, hovered). `look.ts` turns each union of the view
(tone, outline, epic child state, gallery look) into theme colours with an exhaustive switch; a variant added in Rust
fails the linter and `cards.test.ts` until it has a drawing. The gallery shows every fixture of `card_fixtures.rs`
(`gallery.cards`) at the card width and at 1280.

## The side panel

The ticket side panel (_05 · Ticket side panel_, T-0222) is `ui/src/screens/panel/`, served by `src/api/panel.rs`:
`panel.view` gives the open ticket's header, action bar, "…" menu, answer options, queued notice, conflict strip, Start
interrupted callout and tab strip, every text worded in Rust. Buttons that talk to the service go back through
`panel.action` (Start, Retry start, Resume, Reopen, Mark merged, Open PR in browser), which shows a refusal as a toast;
the rest open a tab or an overlay (`logic.ts`, `actionRoute` / `menuRoute`). The panel keeps only its width and
expanded state (`panelLayout`, per window) and the picked answer.

- **A tab or section:** register it (`app.panelTabs.register("diff", …)`, `app.panelSections.register("stack", …)`);
  it gets `{ ticket }` and draws in the panel's body, which is empty until then.
- **Edit:** the bar's Edit and the menu's Edit ticket show the Ticket tab and add a `ticketEditRequests` entry
  (`ui/src/app/selection.ts`) the Ticket tab answers by opening its form.
- **Dialogs opened from the panel:** `close_ticket` and `submit_for_review` are the panel's; `local_merge`, `send_back`,
  `start_from_scratch`, `delete_ticket` and `move_to_epic` are opened with `{ ticket }`, `close_epic` with `{ epic }`,
  each drawn by its own screen.

## The Terminal tab

The panel's Terminal tab (_06 · Terminal tab_, T-0224) is `ui/src/screens/terminal_tab/`, served by
`src/api/terminal_tab.rs`: `terminal_tab.view` gives the agent session to attach (the running one, else the newest),
the header strip's texts (worktree, `$PORT`, the attach command, "1 message waiting · Deliver now", font size and find
tooltips), the "Instructions: shop-web" link, the end marker of an ended session and, for an Interrupted ticket whose
session is gone, "Before the restart" (REC-02). The state label and the hint line depend on the terminal's phase and
focus, which only the page knows, so the view carries one `strips` entry per phase and focus, worded by
`otaq-view`'s `terminal_strip.rs`, and the tab picks the one its terminal reports (`logic.ts`). Deliver now goes
through `terminal_tab.deliver_now`, which answers like `panel.answer` (done, or the unsent-text question).

- **Focus requests:** a `terminalFocusRequests` entry for the shown ticket (a notification click, the bar's Open
  terminal / Answer in terminal) focuses the terminal once it is live, for at most five seconds.
- **T-0237:** the Agent / Validator switch goes at the start of the header strip (marked in `TerminalTab.tsx`).

## The inbox and ⌘J

The Needs-you inbox (_10 · Needs-you inbox_, T-0230) is `ui/src/screens/inbox/`, served by `src/api/inbox.rs`. The
inbox model (`otaq-view`'s `inbox.rs`, resolved rows fading out) and the ⌘J run (`inbox_run.rs`) live in the
app's Rust side between requests; `inbox_page.rs` words the page: `inbox.view` (the head, the usage-limit row, the
groups, each row's answer box and buttons, the empty state, the foot; `refresh_ms` while a row fades),
`inbox.answer` (an option number or text, answered like `panel.answer`: done or the unsent-text question),
`inbox.act` (a row's button, sent back as the `InboxRowAction` it came with), `inbox.next`, `inbox.run_bar`,
`inbox.exit_run` and `inbox.resume_all_confirm`.

- **Overlays:** `inbox` (⇧⌘I and the header button; non-modal, under the button), `resume_all` (Resume all's
  confirmation with its tickets, also opened by the "sessions were interrupted" banner) and `all_clear`.
- **⌘J** works from anywhere, a focused terminal included: the next item opens on its Terminal tab with the
  terminal focused (`requestTerminalFocus`), and the run bar (panel section `inbox_run`) shows on top of that ticket's
  panel until the run ends. It never moves on by itself (ATT-07).
- **New item kinds** (validation, Auto-merge, Project agents: T-0239, T-0241) add their buttons as new
  `InboxRowAction` variants in `inbox_page.rs`; `describeAction` in `logic.ts` has no `default`, so the linter asks
  for each new kind.

## Terminals

The drawer (_17 · Project terminals_, T-0242, T-0331) is `ui/src/screens/terminals/`, served by `src/api/drawer.rs`. It
holds Project terminals only (d0087; Project agents live in the pane): `drawer.view` gives the shown project's tabs
(terminals by number, from `otaq-view`'s `project_terminal.rs`, each with its hint and ended line) and the tab to show.
Opening a tab goes through Rust, which keeps each project's last-used tab: `drawer.last_terminal` (the rail's Terminals,
⇧⌘T) and `drawer.new` (+, a new terminal) each answer the tab to show, which the page passes to `requestTab`
(`logic.ts`). The drawer is the `terminals` overlay, fixed across the bottom of the window; while it is open it pads the
frame so the board ends above it.

## Banners and toasts

The banner area and the toasts (_crewboard.pen → 11 · Banners, toasts & OS notifications_, T-0231) are kept on the
Rust side, in `src/notices.rs`: which banners show (at most two rows, one per situation), their wording and buttons,
the toasts and their time-outs. The page draws `notices.view` and each `notices` event (`ui/src/screens/notices/`).

- **A toast from a screen:** `request("notices.toast", { level, title, text, link, ticket })` (a `NewToast`), with
  wording from a view model or the service's reply. `link` is a `copy` link (`Copy command`) or an `open` link to a
  ticket; `ticket` makes the toast's title open that ticket.
- **A toast for a service event:** one function in `EVENT_TOASTS` (`src/notices.rs`) per event kind, turning the
  event into a `NewToast` or passing.
- **Restart required:** `onlyRestartAllowed(useStore(noticesStore))` from the notices screen says when everything
  except Restart is disabled.
- **Notification clicks** bring the window forward and land in the selection store: the project and the ticket on its
  Terminal tab, plus a `terminalFocusRequests` entry (`ui/src/app/selection.ts`) the Terminal tab answers by focusing
  the terminal; a grouped notification opens the inbox.

## Search, the card menu and deleting

The board screen (`ui/src/screens/board/`, T-0220, T-0221) also fills the `header_search` slot: the header draws that
field in place of its search entry. The filter is kept per project while the app runs (`filter.ts`) and sent with
`board.view` and `board.search`; Rust parses the tokens and words the field. The card's right-click menu asks
`board.menu` for its items and `board.menu_choose` for what a choice does: Pin, Unpin and the priorities are sent at
once, the others open their dialog by overlay id (`delete_ticket`, `close_ticket`, `move_to_epic`, `close_epic`).
**Every place that deletes a ticket** (the card menu, the panel's "…" menu, the Project agent's summary) opens the one
delete confirmation: `openOverlay("delete_ticket", { ticket })`.

## Settings

Settings (⌘,, the `settings` overlay, T-0234) lives in `ui/src/screens/settings/`. `settings.view` sends the nav and
the open page (global or one project) as sections of rows, each row worded with its control
(`otaq_view::view_models::settings_page`); `settings.edit` takes one `field` and its value, applies it with that
module's rules and answers with the fresh view. The screen registers one component per section id it draws
(`ui/src/app/registry/settings.ts`); a later section is added in `settings_page.rs` and registered under its id the same
way (T-0240: `validation`, `project_agent_instructions`). The slow parts load on their own: `settings.tools`,
`settings.provider`, `settings.branches`.

## Shared types

The types the page reads are generated from the Rust structs with `ts-rs` (decision 0075), one file per type in
`ui/src/generated/`, and committed; nothing else goes in that folder. After changing an exported Rust type, regenerate
them all with:

```sh
cargo test -p otaq-desktop regenerate_types -- --ignored
```

`cargo test` (and so `scripts/check.sh`) fails, naming the stale files, when the committed files differ. A new type gets
the `TS` derive (with serde) in its own crate and one line in `export_all` in `src/generated.rs`.

A plain `cargo build` builds the development flavour, which loads the page from the Vite dev server
(`http://localhost:5199`); only the Tauri CLI's build turns on the `custom-protocol` feature that puts `ui/dist` into the
binary. That is also why `cargo clippy` and `cargo test` work on a fresh checkout without `ui/dist`.
