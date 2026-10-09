# 13: Run, Open URL and Shell (formerly "Previews")

> **Updated by decision 0035.** "Preview" is now **Run**, and it works for any kind of project (web, backend, CLI,
> library, mobile). **Open URL** is optional per project. A **Shell** tab gives a plain terminal in the worktree. Read
> "dev command" as **run command** and "Start preview" as **Run** throughout this brief.

Read `00-README.md` and `01-design-system.md` first.

## Context (0026)
- Every ticket gets **two ports** (0026): the agent's `$PORT` (for example 4019) and a separate **Run port** (for example 4020),
  which is `$PORT` inside Run and Shell and is what Open URL uses. Show the Run port in the Run tab.
- Previews run **only on click**: **Start preview** runs the project's dev command in the ticket's worktree,
  **Open** opens `localhost:<port>` in the user's normal browser, and **Stop** stops it. Reaching Done also
  stops it.
- There is no proxy and no auto-start. There is no embedded browser.
- **First use in a project:** a dialog asks "What command starts this project?" with an **Ask Claude** button.
  Claude looks at the repo and suggests a command (for example `pnpm dev --port $PORT`), and the user confirms
  or edits it. It is saved in the project settings.

## Design
1. The **Preview tab** in the side panel: status, port, Start / Open / Stop, and the dev-server output log (monospace).
2. Preview controls on the card and in the panel header (small).
3. The **first-time dev-command dialog**, with Ask Claude:
   - Empty
   - Claude thinking
   - Claude's suggestion shown with an explanation
   - The user editing it
   - Saved
4. A list of **all running previews** (propose where: a header menu or settings), with ports, so the user can
   stop them.

## States
- Not started, starting, running (`:4019`), stopped
- **Failed to start** (the command exited), with the log and an "edit command" link
- The port is already in use by something else (error with a hint)
- A ticket reaching Done while its preview is running (it stops automatically, and a toast says so)
