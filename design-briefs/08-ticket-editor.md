# 08: Ticket editor (create and edit)

Read `00-README.md` and `01-design-system.md` first.

## Context (0019)
Ticket form fields: **Title**, **What and why**, **Priority** (0065), **Depends on**, and **Start from and merge
into** (the base branch; the project default, e.g. `develop`, overridable per ticket, e.g. `main` for a hotfix; locked once
the ticket starts, 0051).
Agents can also create and edit tickets (from a Project agent or Project terminal, or while working on the ticket in Dev), so the
editor must show **who changed what**.

## Design
1. **Create ticket:**
   - Quick-create inline in Backlog (title only)
   - Full form (modal or the panel's Ticket tab)
2. **The form:**
   - Title
   - What and why (multi-line, supports Markdown and code blocks)
   - **Priority**: 1 Urgent · 2 High · 3 Normal (default) · 4 Low (0065). Epic children show the epic's priority unless set.
   - **Depends on** picker: search tickets **in the same project only** (0024). Show the dependency's status.
     Linking tickets with different base branches is rejected: "SHOP-31 merges into `main`, SHOP-20 into `develop`. Linked
     tickets must use the same base branch." (0051)
     **Cycle prevention:** picking a ticket that would create a loop shows an inline error that explains the
     loop (SHOP-21 → SHOP-20 → SHOP-21).
3. **The Ticket tab** in the side panel (read and edit mode).
4. **Agent edits are visible:** while working in Dev, the agent has updated the description. Show an "Updated by agent"
   marker (naming a Project agent when one made it, "by Checkout slowness") and a simple before/after view.
5. ~~Empty "Done when" warning~~: **removed** (0067). There is no "Done when" field; criteria are ordinary text in the
   description, for example under a "Done when" heading the user writes. Nothing warns about a thin description.

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

## M12 additions (design update K; decisions 0063, 0065, 0067)
- Remove the **Done when** checklist (and its add row) from the full form and the Ticket tab; existing items now appear in the
  description under a "Done when" heading.
- Add **Priority** to quick-create (optional), the full form and the Ticket tab.
- Editing the description while the ticket validates: the note under Save says "SHOP-29's validation will be cancelled and its
  agent told what changed." (0063)
