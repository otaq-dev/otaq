# 0076. Product name: Otaq

- **Status:** Accepted
- **Date:** 2026-10-15
- **Related:** 0028 (superseded), 0055, 0057, 0071, 0072, 0077

## Context
0028 named the product Crewboard and asked for a trademark check before launch, noting that "Crewboard" sits close to
**CrewAI**, a well-known multi-agent framework in the same space. The user chose a new name before v0.1 is released,
while only the user runs it and every identity string can still change at no cost to anyone else.

The name comes from the Turkic *otağ*, the commander's tent where reports came in, people were heard and decisions went
out. That is the board's job: agents work out in the field, and anything that needs you comes back to the tent. The
product itself doesn't change.

## Decision
The product is **Otaq**. The About line stays "The board where you run your crew of agents" (0071, as in the design).

**How far the rename goes (user decision, 2026-10-15): everything, now.** The mapping (dated records written before this
decision still use the old names):

| Crewboard | Otaq |
|---|---|
| Crewboard (product, window title, copy) | Otaq |
| `crewboard` command (`crewboard daemon`, `crewboard ask human`, …) | `otaq …` |
| `crewboard-app` binary | `otaq-app` |
| crates `crewboard-cli` / `-core` / `-desktop` / `-view` | `otaq-cli` / `-core` / `-desktop` / `-view` |
| `CREWBOARD_*` environment variables (`CREWBOARD_TICKET`, `CREWBOARD_HOME`, …) | `OTAQ_*` |
| data folder `~/.crewboard/` (0055), `crewboard.db` | `~/.otaq/`, `otaq.db` |
| ticket branch prefix `crewboard/` (`crewboard/SHOP-18`) | `otaq/` (`otaq/SHOP-18`) |
| plugin `crewboard`, `/crewboard:work-ticket` | plugin `otaq`, `/otaq:work-ticket` |
| theme `crewboard` (Crewboard Light / Dark, 0069, 0070) | theme `otaq` (Otaq Light / Dark) |
| Tauri `productName` "Crewboard", bundle ID `io.github.owner.crewboard` | "Otaq", `io.github.owner.otaq` |
| GitHub `<owner>/crewboard`, `crewboard-<version>-<target>.tar.gz`, `Crewboard.app` (0072) | `<owner>/otaq`, `otaq-<version>-<target>.tar.gz`, `Otaq.app` |
| "The Crewboard Authors" (0071) | "The Otaq Authors" |

The code keeps reading the product name, command, plugin namespace and environment-variable prefix from **one constant**
(0028), so a later rename stays cheap.

**Existing data (user decision): no migration.** Otaq starts fresh in `~/.otaq/`. It doesn't read `~/.crewboard/`,
doesn't fall back to `CREWBOARD_HOME`, and doesn't ship a `crewboard` → `otaq` command shim. The old folder is left
untouched for the user to delete. Because no old database is ever read, there is no `crewboard` → `otaq` theme alias either.

**Bundle identifier (user decision): `io.github.owner.otaq`, now.** macOS treats it as a new app: notification
permission is asked once more and the app-support folder starts empty. `owner` stays a placeholder until the publisher
is decided.

**Repository (user decision): `<owner>/otaq`.** 0072's release URLs, archive names and install folders follow the mapping
above.

**Author (user decision): "The Otaq Authors"** in `LICENSE`, `Cargo.toml` `authors` and the plugin's `author.name`; the
About screen shows "© 2026 The Otaq Authors · MIT License". No personal name appears.

**Logo: "Command tent"**, from frame `1. Otaq · Command tent` (`PBIqo`) in the design file. A tent outline with two
board-column lines under the roof and a turquoise flag on the ridge: the board under one roof, and the flag it raises when a
ticket needs you. It replaces the placeholder "Moving card" mark everywhere: the app icon, the About screen and
notifications.
- **App icon:** node `App Icon 96` (`wLWnz`): the white tent on a midnight-indigo glass squircle (`#2C3366` → `#0F1230`),
  flag `#8EF0E4` → `#14B8A6`.
- **In-app mark:** the tent lines in the text color (`currentColor`), the flag in a fixed brand turquoise (`#0E9C8E` on
  light surfaces, `#14B8A6` on dark). The flag is **not** the needs-you token (0077), so the logo looks the same in every
  theme.

The design file keeps its name, `crewboard.pen` (0057). It lives in pen.dev, and renaming it would break its links.

## Alternatives considered
- **Rename only the visible name:** the command, folder, branches and plugin would carry the old name for good, and every
  agent prompt would mix the two.
- **Move `~/.crewboard/` on first start:** recommended, but the user had only two tickets and chose a clean start. Moving
  the folder would also have had to rewrite stored worktree paths and run `git worktree repair`.
- **Keep the old bundle ID until the publisher is decided:** it saves one permission prompt now but means a second identity
  change later.

## Consequences
- One rename pass across code, docs and the design (done in the design first). The older decisions and the SQL migrations'
  comments are reworded too (user decision; no database needs keeping, since there is no migration). The dated records
  keep the old name: the `CHANGES-*`, `AUDIT-*` and `REVIEW-*` briefs, `tickets/review-*.md`, `tickets/progress.md`,
  `tickets/v0.1-tickets.md`, `tickets/questions.md`, the handcheck records and the milestone prompts m12–m14.
- The trademark check of 0028 now applies to "Otaq" (open question in the index).

## Changelog
- 2026-10-15: created (user decisions on scope, existing data, bundle ID, repository and author)
- 2026-10-15: older decisions and migration comments reworded to Otaq as well; 0028's file renamed (user decision).
