# 0015. Two approve actions: Local merge and pull request

- **Status:** Accepted
- **Note (0049):** The bottom-up rule and stacked PRs are not needed in v0.1: a child starts only after its parent is merged (0049).
- **Date:** 2026-10-09
- **Related:** 0005, 0012, 0013

## Context
Approve originally meant either a local squash merge or a GitHub PR, chosen by a repo setting. The user
wants both available as separate actions, and wants Azure DevOps supported as well as GitHub.

## Decision
- A ticket in Ready for Review has **two approve actions**:
  1. **Local merge:** squash-merge into the base branch locally. When main has moved, see 0038.
  2. **Open PR:** push the branch and open a pull request on **GitHub or Azure DevOps**.
- Both actions respect stack order: they go **bottom-up**, so a child cannot be approved before its parent **is
  approved**. **Opening a PR on the parent counts as approval and unblocks the child** (stacked PRs): the child can then
  Open PR targeting the parent's branch. A child's **Local merge** still needs the parent to be **merged** first, because
  merging the child into the base branch would otherwise bring in the parent's unmerged commits.
- Both ship in **v0.1** (0013).
- ~~PR providers sit behind a provider interface (0012)~~. Superseded by 0050: pushing and opening PRs are done by the
  user-replaceable Push and Open PR skills, after a confirmation dialog. GitLab and others are just another skill.

## Consequences
- **Stacked PRs:** a child's PR targets its parent's branch. When the parent merges, the child's PR is
  retargeted to the base branch after the `--onto` rebase (0005).
- Once a PR is open, the ticket waits in Approved until the user clicks **Mark merged** (then Done and
  restack) or **Mark closed**. The factory does not poll (0017).

## Follow-ups
- Auth, the Diff tab and PR feedback are decided in 0017. Detecting remote merges is still open (0017, 4d).

## Changelog
- 2026-10-09: created
- 2026-10-09: open questions moved to 0017.
- 2026-10-09: an open PR on the parent unblocks the child's approval (stacked PRs). Local merge still needs the parent merged (design review, user decision).
- 2026-10-09: remote merge detection replaced with the manual Mark merged / Mark closed (0017).
- 2026-10-09: bottom-up rule and stacked PRs deferred with stacking (0049).
- 2026-10-09: the provider interface is replaced by the Open PR / Push skills with a confirmation dialog (0050).
- 2026-10-09: Local merge and Open PR use the ticket's base branch (0051).
- 2026-10-09: epic children Local-merge into their epic's branch; Open PR is offered on epics and standalone tickets only (0059).
