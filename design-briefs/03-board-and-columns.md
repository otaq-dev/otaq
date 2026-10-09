# 03: Board and columns

Read `00-README.md` and `01-design-system.md` first. Cards themselves are designed in brief 04. Here you design
the board, the columns and movement between them.

## Context
Columns (0004, 0036): **Backlog, Dev (Queued + Agent Working), Ready for Review, Approved, Done**.
That's 5 columns. No Needs Input column: those tickets stay in Dev, highlighted and sorted to the top (0037). There is no Investigate or Ready for Dev (0036). There is no Changes Requested column: Send back is an action (0034). The user mostly works with 5–10 agents,
so up to about 30 to 40 cards are visible at once.

## Design
1. **The board layout** with all 5 columns, populated with the `shop-web` sample data.
   - Propose how to handle the width: horizontal scroll, collapsible columns (for example Backlog and Done
     collapsed to a narrow strip with a count), or grouping (for example "Dev" = Queued + Agent Working shown as
     one column with two sections). Show your recommendation and one alternative.
2. **Column header:** name, card count, and a column-specific hint where useful (for example Queued: "starts
   when dependencies are ready").
3. **Drag and drop:**
   - Picking up a card (lifted style), dragging over columns, and the drop indicator
   - **Allowed vs. not-allowed drop targets.** For example:
     - A user can drag Backlog → Dev (Queued), and Ready for Review → Approved. Sending back is an action, not a drop target.
     - A user can also drag **Dev → Backlog** to **Park** a ticket (confirmation; work and conversation kept, 0052).
     - A user cannot drag into Agent Working or Ready for Review, because only agents move tickets (the user's own "Submit for review"
       is a menu action, 0052)
       there.
   - Show the "not allowed" feedback.
   - Dropping a ticket with an empty "Done when" list into Queued shows a **warning, not a block** (0019; the
     dialog itself is in brief 08).
   - Dragging into **Approved** must ask which approve action: **Local merge** or **Open PR** (0015). Show this
     choice popover. (No bottom-up rule is needed: a child only starts after its parents are merged, 0049.) Show the
     disabled or explained state.
4. **Dependency highlight on hover or selection** (0006): hovering over or selecting a card highlights its whole
   chain (parents and children) across columns, dims the rest, and draws arrows **only for that chain**.
   Use SHOP-18 → SHOP-20 ← SHOP-19 → … SHOP-21.
5. **Empty states:** an empty column, and an empty board for a new project.
6. **Filtering and search:** propose a lightweight search or filter (by text, status, or "needs me") that fits
   the shell.

## States to show
- A normal busy board (`shop-web` sample)
- A drag in progress over an allowed column and over a forbidden column
- The approve-choice popover, and the bottom-up "blocked by parent SHOP-18" state
- The chain highlight active
- A very full column (12+ cards, scrolling)
- 1280 px and 1920 px widths

## Out of scope
The graph view (boxes and arrows) is "maybe later" (0006).
