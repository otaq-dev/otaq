# 16: Recovery and cleanup

Read `00-README.md` and `01-design-system.md` first.

## Context (0018, 0022)
Crewboard **detects and reports, and never self-heals**. Anything that starts an agent or deletes
something needs a click.
- Closing the app window does NOT stop agents, because the background service keeps them running.
- A reboot or a service crash ends sessions → those tickets become **Interrupted** → **Resume** or **Resume
  all**.
- A start that stopped halfway (worktree created, setup failed…) shows **"Start interrupted, Retry"**.
- A **Cleanup view** lists leftovers that no ticket owns (sessions, worktrees, branches, ports). The user
  selects and confirms. Nothing is deleted automatically.
- A Done ticket's worktree is removed automatically, **except** when it has uncommitted changes. Then it
  is flagged in Cleanup.

## Design
1. **The app opening after a reboot:**
   - A summary ("4 sessions were interrupted")
   - The interrupted tickets in the inbox
   - Resume (per ticket) and Resume all
2. **Reopening the app normally:** agents still running, with a quiet "reconnected" message.
3. **Start interrupted (SHOP-28):** the card and panel with one action, **Retry start**, which re-runs the whole start (0018).
   **Resume failed:** offer Retry and **Start from scratch** (a new session in the same worktree, with a confirmation).
4. **The Cleanup view:**
   - Grouped lists: orphaned sessions, worktrees, branches, ports, and "Done ticket with uncommitted changes"
   - Checkboxes, details (size, path, age), and a confirmation dialog listing what will be deleted
   - The empty state: "Nothing to clean up"
5. **Where Cleanup lives:** propose an entry point (settings, a header menu, or a small indicator when there is something to clean).

## After a reboot, by state (0045)
- Only Dev tickets become Interrupted. Orange ones keep their question (SHOP-23), and answering from the inbox resumes and delivers the answer.
- Review and Approved tickets (SHOP-18, SHOP-26) are untouched and not in Resume all.
- Run and Shell show "stopped by restart".
- Interrupted Terminal tab: a **"Before the restart"** view of the agent's last messages and actions (read from Claude's saved
  conversation), plus a "conversation no longer available" variant.
- History lists the ticket's Claude session IDs.

## States
- Resume in progress, resumed, and resume failed (with an error and a retry)
- Cleanup with 0, 3 and 20 items
- The deletion confirmation, then a success toast
