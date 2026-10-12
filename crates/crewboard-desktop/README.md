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
| `ui/src/components/`                                                                                      | the shared components every screen draws with        |
| `ui/src/screens/`                                                                                         | one folder per screen, and the screens index         |
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

**Testing a screen.** `registerScreens({ inbox })` gives fresh registries with only that screen; `registered(app)`
lists what it filled (see `ui/src/app/registry/registry.test.ts`). The frame renders without a DOM through
`react-dom/server` (`ui/src/app/frame.test.ts`).

## Components

The shared components (_crewboard.pen → 01 · Components_, T-0217) are in `ui/src/components/`; a screen imports them
from the folder, never from a file inside it: `import { Button, ConfirmDialog, StatusPill } from "../../components";`.
They are drawn with the theme's CSS variables only (`components.css`) and take typed props: what a view model says
comes in as its generated type (`PillView`, `PriorityView`, `MenuItem`, `ThemePickerSection`), and the component keeps
only the moment of interaction (UIT-04). Their keyboard and placement rules are plain functions in `logic.ts`, tested
without a DOM.

| Component                                                       | Props, briefly                                                                      |
| --------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `StatusPill`, `MetaChip`, `CountBadge`, `Kbd`                   | a `PillView` (`status.rs`); icon + label + tone; count + tone; the keys' text       |
| `Button`, `IconButton`                                          | label, variant, icon, `compact`, `disabled`, `loading` (ignores presses), `onClick` |
| `PriorityChip`, `PriorityLabel`                                 | a `PriorityView` (`priority.rs`)                                                    |
| `Tabs`, `Segmented`, `Toggle`                                   | items with icon, label, indicator; options; on/off. ←/→ move between them           |
| `Field`, `TextInput`, `TextArea`, `Select`, `Checkbox`, `Radio` | label, hint, error (`fields.rs` says which shows); `Select` takes `MenuItem`s       |
| `Menu`, `AnchoredMenu`, `Popover`, `Tooltip`                    | `MenuItem`s (`menu.rs` rules: arrows skip disabled rows and wrap); an anchor        |
| `Dialog`, `ConfirmDialog`                                       | title, body, confirm label, `destructive` (↵ = Cancel); traps focus, Esc cancels    |
| `Banner`, `Toast`, `StackList`, `StackRow`                      | tone, icon, title, text, actions; the Stack list's row shell                        |
| `ThemePicker`, `useThemePicker`, `pickTheme`                    | the picker's sections (`theme.picker`); a click applies and saves (`theme.pick`)    |

Any screen asks for a confirmation without drawing one: `openOverlay("confirm", { title, body, confirmLabel,
destructive, onConfirm })`; the components screen (`ui/src/screens/components/`) draws it with `ConfirmDialog`.
Menus, popovers and tooltips are fixed-position layers inside their anchor's subtree, so a scrolling parent never cuts
them and the anchor's theme applies.

**The gallery.** Start the app with `CREWBOARD_GALLERY=1` (`CREWBOARD_GALLERY=1 npm run tauri dev`) and the design
gallery opens over it: every foundation and component next to each other, the token swatches of Crewboard Light and
Dark, the Status Legend, the keyboard map, the theme picker (it saves) and a palette switch over the 15 themes (it
doesn't). Esc or its close button leaves it. A new shared component gets a place in it
(`ui/src/screens/components/Gallery.tsx`).

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
