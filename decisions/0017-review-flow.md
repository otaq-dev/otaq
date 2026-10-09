# 0017. Review flow: auth, Diff tab, PR feedback

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0005, 0009, 0012, 0015, 0016

## Decision
**Auth (4a):** reuse the user's `gh` and `az` CLI logins only, and store **no tokens** in v0.1.
- A first-run check covers `gh auth status`, `az account show`, and whether the `azure-devops` extension is installed.
- Pushes use the user's own git credentials. Tokens set through environment variables (`GH_TOKEN`,
  `AZURE_DEVOPS_EXT_PAT`) work automatically, because the CLIs read them.

**Diff tab (4b), v0.1:**
- The diff is against the ticket's own **`base_sha`** (0005), not against main, so a child never shows its
  parent's changes.
- File list, **unified and side-by-side** views (both in v0.1), syntax highlighting, with the agent's
  summary at the top.
- Line comments. **Send back** collects them into one batch for the agent.
- **No partial approval.** If part of a ticket is wrong, the whole ticket is sent back.

**PR feedback (4c):**
- The in-app review is the primary review.
- `get_review_feedback` (0009) combines in-app comments with unresolved PR comments from GitHub or Azure.
- PR feedback **never re-queues a ticket automatically**. The user presses Send back on the ticket, which
  pauses its children (0005). The agent then pulls the PR comments **on demand** through
  `get_review_feedback`.
- ~~A "changes requested" review on the PR creates an inbox item~~. Dropped, because 4d means the factory
  never watches PRs.
- Fixes are pushed to the same branch, which updates the PR, **only by the user's "Push to PR" click** (0041). To rework
  an Approved ticket, drag it back to Ready for Review and Send back (0041).

**Remote PR state (4d): fully manual, no polling.**
- The factory **never watches PRs**. There is no polling and no webhooks.
- After **Open PR**, the ticket waits in Approved with a "PR open" status. The user marks it:
  - **Mark merged**: the ticket moves to Done and its children are restacked onto the base branch, with
    their PRs retargeted (0005, 0015).
  - **Mark closed**: ~~the factory does nothing else~~. The ticket is **closed** (Done · "Closed · not merged";
    reopenable), see 0033.
- **Local merge** is unaffected. The user's Approve click performs the merge, so the children's restack follows
  immediately.

## Changelog
- 2026-10-09: created
- 2026-10-09: Mark closed now closes the ticket (0033).
- 2026-10-09: removed the test-gate result from the Diff tab (0018).
- 2026-10-09: 4d decided (fully manual, no polling). Dropped the automatic inbox item for PR "changes
  requested" reviews, since nothing watches PRs anymore.
