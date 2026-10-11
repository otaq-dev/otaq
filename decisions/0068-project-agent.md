# 0068. Project agents (investigate and create tickets), and several Project terminals

- **Status:** Accepted
- **Date:** 2026-10-11
- **Related:** 0016, 0018, 0019 (refined), 0022, 0030, 0031 (refined), 0040, 0045, 0053, 0059, 0060, 0065
- **Supersedes:** "the planner agent" in the out-of-v0.1 lists of 0053 and the SDD (importers stay out: they are the user's
  own skills)

## Context
Grilling session, 2026-10-11. The user wants an agent that helps create tickets: investigate something in the code and file
tickets for it, or import issues from the team's tracker (for example GitHub issues) through the user's own skill. The
Project terminal (0019) is a plain shell where the user can start `claude` by hand; the user wants to keep that as a shell
and add the agent as a separate thing. Both should be openable more than once.

## Decision
**Two separate things per project, each openable several times:**
- **Project terminal:** a plain shell in the project folder, as in 0019. Full `crewboard` access, because it is the user
  typing. Named "Terminal 1", "Terminal 2"…
- **Project agent:** a `claude` session in the project folder (the user's checkout) with the Crewboard skill
  `/crewboard:project-agent`, started by the user's click (opening it is the go-ahead, 0018).

**What a Project agent may do.**
- Read and run code, in the user's checkout and in **ticket worktrees** (for example look at SHOP-18's in-progress code or run
  its tests). It **never edits or commits**. This is a rule taught by the skill, not a lock.
- Its context, `crewboard prime --project`, lists the board: every ticket with status, priority, dependencies, epic, and
  for each started ticket its **worktree path and branch**, plus the user's **Project agent instructions**.
- **Tickets:** create, edit, link and prioritise tickets **that are still in Backlog** (including epics, children and External
  ID). It can't move tickets to Ready for Dev or Dev, edit started tickets, approve, merge, close or delete.
- It can ask a ticket's agent with `crewboard ask <KEY>` (0030 rules for delivery and resuming).
- Importing from trackers lives entirely in the user's skills. Crewboard ships no importers; the CLI covers what an import
  needs (External ID, priority, epic and parent, dependencies, batch create `create -f`).

**Project agent instructions:** a new text setting, global default plus per-project override (like 0040), followed by every
Project agent ("use my /import-github skill", "split into tickets of about one day").

**Naming.** A new Project agent shows "New agent" until the first request. The skill tells it to name itself right after
reading that request with `crewboard agent name "<3–4 words>"` (for example "Checkout slowness"). If it hasn't by the end of
its first turn, Crewboard uses the first words of the request. It names itself once; it renames only when the user asks.
**The user's own rename always wins and is never overwritten.**

**What it created.** Each Project agent keeps a **"Created and changed"** summary ("Created SHOP-35, SHOP-36 · edited SHOP-31
· 2 dependencies") with links, and **Delete** per created ticket (0060 confirmation; those tickets never went to Dev). Cards
get the "created by agent" / "Updated by agent" markers naming the Project agent ("by Checkout slowness"). New tickets appear
in Backlog live with a subtle highlight.

**Lifecycle.**
- Owned by the background service (0022): closing the window doesn't stop them; reopening shows them as they were.
- Closing a Project agent ends it (confirmation if it's mid-turn). Closing a Project terminal ends the shell.
- After a reboot or service restart: shells are gone; Project agents show **"Ended by restart · Resume"**, which resumes the
  same conversation on the user's click (0018).
- A list of the **10 most recent ended Project agents** per project, to reopen (resume) one. Older entries leave the list;
  Claude's own transcripts are never deleted.
- A Project agent waiting on a permission prompt or asking the user is an **inbox item (group 1)** with an OS notification
  (0016). Its tokens count in today's total, shown per agent.
- No limit on how many are open. They don't count toward the agent limit (it counts tickets); the plan-limit hold applies.

**New CLI:** `crewboard prime --project`, `crewboard agent name "<name>"` (Project agent sessions only).

## Alternatives considered
- **One terminal that is both a shell and an agent (tabs):** rejected by the user, they are different tools.
- **Crewboard importers in Rust** (0003 adapters): not needed; the user's skills do it through the CLI.
- **Project agent may edit code:** rejected, code changes go through tickets.

## Changelog
- 2026-10-11: created (grilling session, user decision)
