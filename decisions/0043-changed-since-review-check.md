# 0043. Approve warns when a ticket changed after it entered Review

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0015, 0030, 0038, 0041

## Context
A ticket in Ready for Review or Approved usually keeps a live, idle session. A question from another agent (0030) or the
user's own typing can wake that agent, which might commit more changes after the user reviewed the diff. (Raised by the
product owner, review 2.)

## Decision
- **Question delivery is unchanged (0030).** Agent-to-agent questions are still delivered to any ticket with a live
  session when its agent is idle, including tickets in Review or Approved. The user chose not to route these through
  the inbox.
- **Check at Approve:** when a ticket enters Ready for Review, Crewboard records its branch head. If **new commits**
  appear before Approve (Local merge, Open PR or Push to PR), Approve shows **"SHOP-18 changed since you reviewed it"** and
  a diff of **only the new commits**. The user can approve anyway or Send back.
- Crewboard's own rebases (onto the parent, 0005, or onto main, 0038) don't trigger the warning, because the content the
  user reviewed is unchanged.

## Changelog
- 2026-10-09: created (product owner review 2, user decision)
- 2026-10-09: after a push, Crewboard compares pushed content (not commit IDs) with the reviewed content, because the confirmation dialog can reword commits (0050).
