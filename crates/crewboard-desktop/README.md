# crewboard-desktop

The Crewboard desktop app on Tauri 2: a window whose page is React with TypeScript, built with Vite (decisions 0074,
0075; SDD §5.5). Its binary is `crewboard-desktop` until T-0245 gives it the GPUI app's name, `crewboard-app`.

## Where things are

The frontend sits inside this crate (the implementation choice SDD §5.5 leaves open), so one folder holds the whole app
and every `npm` command runs from here:

| Path                                                                                                      | What                                                 |
| --------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| `Cargo.toml`, `build.rs`, `src/`                                                                          | the Rust side                                        |
| `src/door.rs`                                                                                             | the only Rust module that names Tauri (UIT-02)       |
| `tauri.conf.json`, `capabilities/`, `icons/`                                                              | Tauri's config: the window, the CSP, zoom keys off   |
| `package.json`, `package-lock.json`, `.nvmrc`                                                             | the npm packages and the Node version (24)           |
| `tsconfig.json`, `tsconfig.node.json`, `eslint.config.ts`, `types/`, `.prettierrc.json`, `vite.config.ts` | the TypeScript rules (UIT-08) and the build          |
| `ui/index.html`, `ui/src/`                                                                                | the page                                             |
| `ui/src/tauri.ts`                                                                                         | the only TypeScript file that names Tauri (UIT-02)   |
| `ui/src/generated/`                                                                                       | the types shared with Rust, generated with `ts-rs`   |
| `ui/src/app/`                                                                                             | the frame, the registries, the shared selection      |
| `ui/src/screens/`                                                                                         | one folder per screen, and the screens index         |
| `ui/src/terminal/`, `src/terminal.rs`                                                                     | the terminal component every terminal screen uses    |
| `src/api/`                                                                                                | the request table and one file per area              |
| `ui/dist/`                                                                                                | the built page (git-ignored), Tauri's `frontendDist` |

## Commands

Use Node 24 (`nvm use` in this folder reads `.nvmrc`). Nothing is installed globally; the Tauri CLI is the
`@tauri-apps/cli` dev dependency.

```sh
npm ci                  # install the locked packages
npm run tauri dev       # the app with the Vite dev server (live reload)
npm run tauri build     # a release build with the page inside the binary: target/release/crewboard-desktop
npm run check           # typecheck, lint, format:check, test (each also runs on its own)
```

## How a screen plugs in

The page is a fixed frame (`ui/src/app/Frame.tsx`, _02 · App shell → Window anatomy_) whose parts are filled by
screens. A screen touches only its own folders plus **one line in the screens index** and, if it calls the service,
**one line in the request table**. Everything it can fill is listed, typed, in the registries (`ui/src/app/registry/`):

| Registry                          | Ids (fixed in)                                                               | A screen registers                                 |
| --------------------------------- | ---------------------------------------------------------------------------- | -------------------------------------------------- |
| `app.slots`                       | `slots.ts`: title bar start/end, header, rail, banners, board, panel, toasts | the component that fills the slot                  |
| `app.overlays`                    | `overlays.ts`: every dialog, popover and drawer, with what it is opened with | its overlay's component (gets `params`, `close`)   |
| `app.panelTabs`                   | the generated `PanelTab` (Rust `panel::Tab`), in the order of `panel.ts`     | its tab's content (gets the open `ticket`)         |
| `app.panelSections`               | `panel.ts`: Stack, Messages                                                  | its section (gets the open `ticket`)               |
| `app.settings.global`, `.project` | `settings.ts`: the global and the project sections, in nav order             | its section (a project section gets the `project`) |
| `app.shortcuts`                   | the generated `KeyAction` (Rust keymap)                                      | a handler per action, `app.shortcuts.on(...)`      |

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
from `crewboard-view`'s keymap), so no TypeScript file lists a shortcut. While the selection's `focusedTerminal` is set,
every key goes to the terminal except the app's (⌘ on macOS, Ctrl+Shift on Windows and Linux); a terminal component
calls `focusTerminal(session)` on focus and `focusTerminal(null)` on blur. Leave terminal focus blurs it when no screen
handles it. Outside a terminal, bound keys that no handler takes, and keys the webview would act on (reload, print,
find, zoom, devtools, caret browsing, history), are swallowed; a focused text field keeps Enter, Backspace and Esc. The
app started with `CREWBOARD_KEY_LOG=1` shows the last keys and where each went, bottom right.

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
  ref={terminal} // terminal.current?.focus() / .openFind() / .jumpToLatest()
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

## Shared types

The types the page reads are generated from the Rust structs with `ts-rs` (decision 0075), one file per type in
`ui/src/generated/`, and committed; nothing else goes in that folder. After changing an exported Rust type, regenerate
them all with:

```sh
cargo test -p crewboard-desktop regenerate_types -- --ignored
```

`cargo test` (and so `scripts/check.sh`) fails, naming the stale files, when the committed files differ. A new type gets
the `TS` derive (with serde) in its own crate and one line in `export_all` in `src/generated.rs`.

A plain `cargo build` builds the development flavour, which loads the page from the Vite dev server
(`http://localhost:5199`); only the Tauri CLI's build turns on the `custom-protocol` feature that puts `ui/dist` into the
binary. That is also why `cargo clippy` and `cargo test` work on a fresh checkout without `ui/dist`.
