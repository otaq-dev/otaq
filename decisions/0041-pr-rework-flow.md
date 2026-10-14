# 0041. PR rework: drag Approved back to Review, Send back there, and the user pushes every update

- **Status:** Accepted
- **Note (0049):** "PR out of date · Push" for restacked children is deferred with stacking (0049). The rest applies.
- **Date:** 2026-10-09
- **Related:** 0005, 0015, 0017, 0018, 0034

## Context
0034 allows Send back only from Ready for Review, but PR comments arrive while a ticket is in **Approved · PR open**
(0017). It was also undefined who pushes rework to the PR. A push changes GitHub or Azure, so by the user's principles it
needs a click. (Raised by the product owner, review 2.)

## Decision
- **No Send back on Approved.** To rework a ticket whose PR got comments, the user **drags it from Approved back to
  Ready for Review**. The PR stays open, and the card keeps its "PR #481" chip. Then they use the normal **Send back**
  (0034). The agent fetches the PR comments with `otaq review feedback` (0017).
- ~~**The agent never pushes.**~~ Superseded by 0050: the agent pushes through the Push skill, but only after the user's
  click and confirmation dialog. When it resubmits (`otaq review submit`), the ticket lands in **Ready for Review** with
  the PR chip. For a ticket with an open PR, Approve offers **"Push to PR #481"**, which pushes the branch and returns
  the ticket to **Approved · PR open**.
- **Restacked children with open PRs:** after a restack, the card shows **"PR out of date · Push"**. Pushing (a
  force-push of the child's branch) is **always the user's click**, never automatic.
- **Unchanged:** **Mark merged** on a parent still retargets and updates its children's PRs as part of that click (0015, 0017).

## Changelog
- 2026-10-09: created (product owner review 2, user decision)
- 2026-10-09: "PR out of date · Push" deferred with stacking (0049).
- 2026-10-09: "the agent never pushes" superseded by 0050 (pushes only after the user's click and confirmation).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
