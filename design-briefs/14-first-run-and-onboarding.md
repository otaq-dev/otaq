# 14: First run and onboarding

Read `00-README.md` and `01-design-system.md` first.

## Context (0021, 0023, 0011)
- The target user is a developer. The app is not code-signed, which is handled by the OS, not by us.
- Crewboard **never installs or updates** other tools. It shows a **checklist** with the command to fix each:
  - **git** (required)
  - **claude** CLI (required)
  - **gh** (optional, for GitHub PRs) and **az** with the `azure-devops` extension (optional, for Azure DevOps PRs)
  - Logged-in status for gh and az
- **Zero config:** adding a project needs only a folder that is a git repo. Nothing is written into the repo.
  The settings can be filled in later.
- The permission-mode warning (brief 11) can appear here.

## Design
1. **Welcome:** one screen, what Crewboard does, with no marketing fluff.
2. **Theme ("Pick a look", 0069, 0070):** the 15 theme cards, grouped under "Dark" and "Light" headings (labels only),
   each a small preview with a one-line tagline; one card is picked. No mode switch. The first launch preselects
   Crewboard Dark or Crewboard Light to match the OS. The window recolors as soon as a card is clicked. "Change it anytime in
   Settings → Appearance."
3. **Tool checklist:** each tool found (with its version), missing (with the copyable install command for the
   current OS, for example `brew install gh` on macOS or `winget install GitHub.cli` on Windows), or optional and
   skipped. A "Re-check" button.
4. **Add your first project:** a folder picker. Validation: not a git repo, or already added. Base branch detected.
   A **Project key** field is prefilled from the repo name (for example `SHOP`) and editable, with the hint "Used in
   ticket IDs like SHOP-18. Can't be changed later." (0029)
5. **Done:** the empty board with a hint to create the first ticket, open a Project agent ("let Claude create tickets") or a
   Project terminal. One line mentions **Auto-merge** (off by default; turn it on in project settings) (0064, 0068).

## States
- Everything installed
- `claude` missing (blocking: explain that agents can't start without it)
- `gh` and `az` missing (non-blocking)
- The selected folder is not a git repo
- Adding a second project later (a shorter flow from the project switcher)
