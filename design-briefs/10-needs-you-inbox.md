# 10: "Needs you" inbox

Read `00-README.md` and `01-design-system.md` first.

## Context (0016, 0024)
The single place to see everything that needs the user, **across all projects**. Ordered:
1. **Orange items: Asks you, Needs you, Idle** (0042). That is, **permission prompts, agent questions, idle agents** (SHOP-22, SHOP-23, SHOP-12), including **agent-to-agent questions
   whose target has no live session, only when the project setting "Let finished tickets answer questions" is off** (0030), for example "SHOP-21 asks SHOP-17 (Done): which Stripe API version?".
   The user can answer it directly or Resume the target.
   While a ticket's terminal has focus, that ticket creates no inbox item; it appears when the user leaves the terminal
   with the agent still waiting (0042).
2. **Interrupted sessions and children of a closed parent** (SHOP-25). Restack conflicts are **not** inbox items: the agent
   resolves them automatically (0005).
3. **Ready for Review** (SHOP-18, BILL-5)

Within each group, tickets that block more children come first. A **next-item hotkey** opens the next item
with its terminal focused. It does **not** auto-advance: after answering, the ticket stays open, and the user
presses the hotkey again for the next item (0016). Every item shows its **project
name**.

## Design
1. **The inbox surface:** propose a panel, popover or dedicated view. It must be reachable from anywhere
   with one shortcut.
2. **Inbox item:**
   - Project, ticket number and title, and the reason ("asks: Should fullName be split…?", "permission:
     Allow `rm -rf .next`?", "session interrupted", "ready for review · +412 −38")
   - Age, and the number of children it blocks
3. **The next-item flow:**
   - The user presses the shortcut, the ticket opens with the terminal focused, they answer, and press it again
   - Show a "3 of 7" style progress indicator and the "all clear" moment
4. **Grouping headers** and counts, plus a "Resume all" action on the interrupted group (0018).
5. **Inbox count** in the header and project switcher (brief 02).

## States
- Mixed inbox across 2 projects (sample data)
- One item
- **Empty**: "Nothing needs you": a calm, rewarding state
- An item that resolves outside the inbox (for example, the user answered the prompt from an external
  terminal through `crewboard attach`), so it disappears gracefully
- The plan-limit banner is active: items are **not** duplicated per agent (brief 11)
