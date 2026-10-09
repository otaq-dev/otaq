> **REMOVED by decision 0036.** There is no Investigate step or column. Investigating is part of Dev. Do not
> design this brief. It is kept for history only.

# 09: Investigation (column and result view)

Read `00-README.md` and `01-design-system.md` first.

## Context (0019)
The optional **Investigate** column sits between Backlog and Ready for Dev:
1. The user drags a ticket to Investigate, and an agent session starts.
2. The agent reads the code, posts an **investigation result** (findings and questions), can ask questions
   (the ticket moves to **Needs Input**), and **updates the ticket**.
3. When it's finished, the ticket moves to **Ready for Dev**. **Nothing is built** until the user drags it to Dev.
4. Moving to Dev **continues the same session**, so the agent remembers what it learned.

Investigating does not wait for dependencies.

## Design
1. **The Investigation tab** in the side panel, for SHOP-12 "Add dark mode toggle":
   - Summary, findings ("Settings page is `src/app/settings/page.tsx`. Colors are hard-coded in 14 files"), and
     relevant files (clickable)
   - **Questions for the user**, each with an answer box or a "answer in terminal" link, and an answered/unanswered state
   - Ticket changes proposed or made by the agent (links to brief 08's before/after view)
2. **Card and column states** for Investigate (also in brief 04): investigating, has questions, finished.
3. **Hand-off moment:** the finished investigation sits in Ready for Dev with an "investigated ✓" marker. Show
   the drag to Dev, and the message that the same session continues.

## States
- Investigation running (no result yet)
- A result with 0 questions vs. 3 questions (1 answered)
- The agent asked a question and is waiting (Needs Input)
- Re-investigate (the user drags it back to Investigate after editing the ticket)
