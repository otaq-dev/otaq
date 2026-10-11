# 18: Project agents

Read `00-README.md` and `01-design-system.md` first. Project terminals (plain shells) are in brief 17; both live in the same
place in the app.

## Context (0068)
A **Project agent** is a real interactive `claude` session in the project folder, opened by the user, for **investigating
something and creating tickets with Claude's help**: "why is checkout slow? make tickets for what you find", or "import the
GitHub issues labelled `ready`" through the user's own `/import-github` skill. Crewboard ships no importers.
- **Several per project**, owned by the background service (survive closing the window).
- It **reads and runs code** in the user's checkout and in ticket worktrees, but **never edits or commits**.
- It **creates, edits, links and prioritises Backlog tickets only**. It can't start, move, approve, merge, close or delete
  tickets. It can ask a ticket's agent a question (`crewboard ask SHOP-18`).
- It follows the user's **Project agent instructions** (settings, brief 15).
- It **names itself** after the first request ("Checkout slowness"); until then it is "New agent". The user can rename it,
  and the user's name always wins.

## Design
1. **Opening a Project agent** from the same place as Project terminals ("+" → **New Project agent**). Opening it is the go-ahead
   for its tokens; no extra confirmation.
2. **The agent view:** the real terminal (brief 06 rules: every key goes to Claude while focused), with a header showing the
   name, state (working / waiting for you / idle / ended), tokens for this agent, and **Rename**.
3. **Naming:** the "New agent" state; the name appearing after the first request; renaming inline; a user-renamed agent keeps
   its name.
4. **"Created and changed" summary** per agent: "Created SHOP-35, SHOP-36 · edited SHOP-31 · 1 dependency", each key a link;
   **Delete** next to each created ticket (the 0060 confirmation). Propose where it sits (a strip above the terminal, a side
   list, or a collapsible section).
5. **Live board feedback:** tickets it creates appear in Backlog in real time with a subtle highlight; their cards say "created
   by Project agent 'Checkout slowness'"; edited cards show "Updated by agent" naming it.
6. **Several agents and terminals together:** "Checkout slowness", "Import GitHub issues", "New agent", "Terminal 1",
   "Terminal 2": how the user tells them apart and switches (a tab strip, a list, or both).
7. **Recently ended:** a list of the 10 most recent ended Project agents for this project ("Footer audit · yesterday"), each
   with **Resume** (continues the same conversation).
8. **Attention:** a Project agent waiting on a permission prompt or asking the user shows on its tab and in the inbox (group 1,
   brief 10), with an OS notification; none while its terminal has focus.

## States
- New agent (no request yet)
- Working: investigating ("Checkout slowness" running a profiler in SHOP-19's worktree)
- Importing: "Import GitHub issues" creating SHOP-37–SHOP-39 with External IDs GH-212, GH-215, GH-219
- Waiting for you: a permission prompt, and a question ("Import closed issues too?")
- A refused action: the agent tried to move SHOP-35 to Ready for Dev: "Project agents can only change Backlog tickets"
- Closing an agent that is mid-turn (confirmation)
- Ended normally; **Ended by restart · Resume**
- The recently-ended list, empty and with entries
- Light and dark

## Decided after design update K (2026-10-11)
- Header button **"Terminals & agents"** (⇧⌘T; orange dot when an agent waits); **⇧⌘A / Ctrl+Shift+A** opens a new Project agent.
- A "New agent" closed before its first request is **discarded** (no confirmation, not in Recently ended), and it disappears
  after a restart.
- Project agents ask the user with `crewboard ask human` (options allowed, "Other…" as everywhere, 0061).
