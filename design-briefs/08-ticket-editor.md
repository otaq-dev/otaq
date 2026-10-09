# 08: Ticket editor (create and edit)

Read `00-README.md` and `01-design-system.md` first.

## Context (0019)
Ticket form fields: **Title**, **What and why**, **Done when** (a checklist), **Depends on**, and **Start from and merge
into** (the base branch; the project default, e.g. `develop`, overridable per ticket, e.g. `main` for a hotfix; locked once
the ticket starts, 0051).
Agents can also create and edit tickets (from the Project terminal, or while working on the ticket in Dev), so the
editor must show **who changed what**.

## Design
1. **Create ticket:**
   - Quick-create inline in Backlog (title only)
   - Full form (modal or the panel's Ticket tab)
2. **The form:**
   - Title
   - What and why (multi-line, supports Markdown and code blocks)
   - **Done when** checklist (add, reorder, remove)
   - **Depends on** picker: search tickets **in the same project only** (0024). Show the dependency's status.
     Linking tickets with different base branches is rejected: "SHOP-31 merges into `main`, SHOP-20 into `develop`. Linked
     tickets must use the same base branch." (0051)
     **Cycle prevention:** picking a ticket that would create a loop shows an inline error that explains the
     loop (SHOP-21 → SHOP-20 → SHOP-21).
3. **The Ticket tab** in the side panel (read and edit mode).
4. **Agent edits are visible:** while working in Dev, the agent has updated the description and added two
   "Done when" items. Show an "Updated by agent" marker and a simple before/after view.
5. **Empty "Done when" warning** when dragging to Dev (or clicking Start): a dialog with
   "Start anyway" and "Add criteria". It is a **warning, not a block**.

## States
- Empty form, filled form, validation (missing title)
- A dependency on a ticket in Done vs. in progress
- The cycle error
- Editing while the agent is working (a note that the agent won't see changes until it fetches the ticket again)
- A ticket created by Claude (a "created by agent" origin label), landing in Backlog

## Editing a ticket while its agent works (0048)
- On save, a small note under the Save button: "SHOP-19's agent will be told what changed." The terminal later shows the
  delivered note, or the card shows "1 message waiting" until it can be delivered.

## Base branch and epics (0051)
- An **epic's** editor has the base-branch field (locked once a child has started).
- An **epic child's** editor shows it read-only: "From the epic's branch (SHOP-40)" (0059).
- **External ID** (0059): an optional field on epics and standalone tickets, for example `PAY-123` (the human ticket in the
  team's tracker). Not shown on epic children.
- **Move to epic** dialog: "SHOP-51 will start from the epic's branch" (0059).
