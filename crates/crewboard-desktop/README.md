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
