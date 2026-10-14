# 07: Diff tab, review, approve and send back

Read `00-README.md` and `01-design-system.md` first.

## Context (0015, 0017)
- The diff is shown **against the ticket's own base** (where it left `main`, `base_sha`),
  so a child never shows its parent's changes.
- Both **Unified** and **Side-by-side** views are in v0.1.
- The **agent's summary** is shown at the top, and since M12 the **validation report** that let the ticket through (0063).
- **Line comments**: the user writes comments on lines, and **Send back** bundles them into one message to
  the agent.
- **No partial approval.** If part of it is wrong, the whole ticket goes back.
- **Two approve actions:** **Local merge** (squash into the base branch) and **Open PR** (GitHub or Azure
  DevOps, chosen by the project). (No bottom-up rule: children start only after their parents are merged, 0049.)
- After Open PR, the ticket waits in Approved as **PR open** until the user clicks **Mark merged** (Done, and
  tickets waiting for it can start) or **Mark closed**. Otaq never checks GitHub or Azure by itself.

## Design
1. **Diff tab layout** for SHOP-18 (9 files, +412 −38):
   - File list (added, modified, deleted, renamed), per-file counts, and a "viewed" checkmark
   - Agent summary at the top (collapsible)
   - Unified ↔ Side-by-side toggle, syntax highlighting
   - Base indicator: "diff vs `main` at `a1b2c3d`" (where the ticket left its base branch)
2. **Line comments:** add, edit, delete, a comment thread on a line, and a pending-comment counter.
3. **Send back flow:**
   - Review the bundled comments, plus an optional general note
   - The ticket moves straight back to Dev, and the agent continues in the same session with a "Sent back" marker (0034)
4. **Approve flow:**
   - **Local merge:** confirmation showing the target branch and the squash commit message (editable), then a
     success state, and any waiting children starting
   - **Open PR** (0050): the Open PR skill first proposes; then a **confirmation dialog** with editable **branch name**
     (live validation: valid, not taken), **one field per commit message**, **PR title and description**, the target (`main`),
     and **Push & open PR** / **Cancel**. Show the "skill is preparing…" state, the dialog, pushing, and success (PR number and link).
   - **Push to PR**: the same dialog with the branch name locked and only new commits editable.
   - **Uncommitted files refusal** (0050): "SHOP-19 has 2 uncommitted files" with **View** and **Send back**, on Local merge,
     Open PR and Push to PR.
5. **PR open state** (SHOP-26): the PR link, **Mark merged**, **Mark closed**, and a short note that
   Otaq does not watch the PR.
6. **Sending back a ticket that has a PR** (BILL-5 on Azure DevOps): explain that PR comments are fetched for the
   agent when sent back.

## Changed since review (0043)
- Approve on SHOP-18 shows a warning: "SHOP-18 changed since you reviewed it · 1 new commit", with a diff of only the new
  commit (for example a null check added while it answered SHOP-20's question). The actions are Approve anyway and Send back.

## PR rework (0041)
- Drag an Approved · PR open ticket (SHOP-26) back to Ready for Review. The PR chip stays. Then Send back.
- After rework, Approve shows **"Push to PR #481"**, which pushes and returns the ticket to Approved · PR open.

## Main has moved (0038)
- A **"conflicts with main"** flag on Review cards and in the panel (read-only check after every merge).
- Local merge: "Rebasing onto main…" → success, or a conflict state with **Send to agent to fix**.
- Refusal: "shop-web has uncommitted changes on main. Clean up and retry." Otaq never stashes.

## States
- A large diff (50+ files), a binary file, a renamed file, an empty diff
- Merge conflict during the local merge (shown as an error state, with what to do next)
- Not logged in to `gh` or `az` (error with the fix command, `gh auth login`)

## Reviewing and delivering an epic (0059)
- When all children of epic SHOP-40 (External ID PAY-123) are merged into its local branch, the epic card moves to Ready for
  Review. The Diff tab shows the **whole feature** (epic branch vs its base), grouped by child (one squash commit each).
- Approve on the epic: **Open PR** (the dialog proposes `feature/PAY-123-wishlist`, one commit per child, title "PAY-123
  Wishlist") or **Local merge** into the base branch. There is no Send back on an epic: the panel explains "Add or reopen a
  child ticket to change it".
- Epic children have only **Local merge** (into the epic branch). They have no Open PR.
- The base-diverged hold: "`develop` and `origin/develop` have diverged. Push or pull, then retry."

## M12 additions (design update K; decisions 0063, 0064)
- **How it got here:** the review shows "Validated · round 2" (SHOP-18), or "Validation skipped by you", "Submitted by you",
  "Failed 3 times · moved by you" (the last one clearly says it did not pass).
- **Validation report** next to the diff (collapsible like the agent summary): what was checked and passed ("`pnpm test`
  passed", "scope matches the description"), and the **notes**: "Order confirmation email: covered by SHOP-21" (links to the
  ticket) and "Not covered by any ticket: refunds endpoint" with a **Create ticket** action (prefilled Backlog ticket, same
  epic). For a "Failed 3 times · moved by you" ticket, the last round's findings.
- **Auto-merge** (shop-web: Epic children only): an epic child that passes is Local-merged without the confirmation and lands in
  Done as "Merged automatically" (SHOP-40.1). A conflict goes back to the agent automatically ("Merge conflict · fixing",
  SHOP-40.4). A manual Local merge keeps "Send to agent to fix".
- After a **Send back**, the agent's next submit is validated again, including the review comments.
