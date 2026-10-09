# 12: Dependencies (beads-style chains end to end)

Read `00-README.md` and `01-design-system.md` first. This brief is a **storyboard**: show the flows over time.
The individual pieces appear in briefs 03, 04 and 05.

## Context (0006, 0049)
- Tickets can depend on any number of other tickets (or an epic, 0046) in the same project.
- **Like beads:** a ticket is ready only when **every dependency is Done · Merged**. Closed never counts (0044).
- Every ticket branches from the base branch (`main`) when it starts, so it already contains its parents' merged work.
- **Stacking is not in v0.1** (0049): no Paused children, no restacks, no stacked PRs, no bottom-up approve rule.

## Storyboards (use the Payments chain: SHOP-17 → SHOP-18 → SHOP-20 ← SHOP-19, then SHOP-21 after SHOP-20)
1. **Planning a chain:** create SHOP-18, SHOP-19 and SHOP-20 with dependencies. The Stack list shows the tree, and the cards
   show `⛓` badges.
2. **The chain runs:** the user drags all of them to Dev. SHOP-18 and SHOP-19 work in parallel. SHOP-20 sits in Queued with
   **"waits for SHOP-18, SHOP-19 to merge"**, and the line updates as each parent moves ("SHOP-18 in Review").
3. **One parent merged:** the user approves SHOP-18 with Local merge. SHOP-20 still waits: "waits for SHOP-19 to merge".
4. **All parents merged:** SHOP-19 is merged, so SHOP-20 starts from the new `main` (toast: "SHOP-20 started · all
   dependencies merged").
5. **PR variant:** SHOP-18 is approved with Open PR. SHOP-20 waits ("waits for SHOP-18 · PR #482 open"). The user clicks
   **Mark merged**, Crewboard fetches `main`, and SHOP-20 starts.
6. **Closed parent:** SHOP-19 is closed instead. SHOP-20 shows "parent SHOP-19 closed", with the inbox item offering
   Reopen parent / Remove dependency (0033, 0044).

## Also design
- The **Stack list** in detail: deep chains (5 levels), multiple parents, a long title, and every status.
- The **chain highlight** on the board (hover or selection).
