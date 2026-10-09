# 0024. Multiple projects, one board per project

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0005, 0007, 0016, 0020, 0023

## Decision
- The app manages **multiple projects**. The user **switches between projects in the dashboard**, and each
  project has its **own board**.
- **One project = one git repository.**
- **Shared across all projects:**
  - The **"Needs you" inbox** (0016), so a stuck agent in another project is never missed.
  - The optional **`max_concurrency`** and back-pressure settings (0016, off by default), because it is one machine.
  - **Usage tracking and the plan-limit banner** (0020), because it is one Claude plan.
- **Per project:** board, tickets, dependencies, settings (0023), worktrees.
- **Dependencies stay within one project.** A ticket can't depend on a ticket in another project.
- The project switcher shows, for each project, how many agents are working and how many items need the
  user. Inbox items are labeled with their project.

## Changelog
- 2026-10-09: created
