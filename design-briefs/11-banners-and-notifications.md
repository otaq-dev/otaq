# 11: Banners, warnings, toasts and OS notifications

Read `00-README.md` and `01-design-system.md` first.

## Context
With 5–10 agents, noise is the enemy. Rule: **one banner per situation, never one per agent**. OS
notifications go out **only for items that block an agent** (0016).

## Design each message, with its placement, wording, actions and dismiss behavior
1. **Plan usage limit reached** (0020): a global banner. "Usage limit reached · resets at 3:00 pm · no new
   tickets will start". The action after the reset is **Resume all**. Nothing resumes automatically. Show:
   - The limit just hit (8 agents stopped)
   - Waiting for the reset (with a countdown)
   - After the reset (Resume all highlighted)
   - Session vs weekly limit ("resets Mon 12:00am"), and "resets at an unknown time"
   - The safety net: an Idle inbox item with **"This is the usage limit"**, which turns the Idle items into this banner
2. **Permission-mode warning** (0011, 0016): shown on first run if the user's Claude Code settings prompt for
   permissions. Warning only: "With 5–10 agents, permission prompts will stop agents often", with links to the
   docs on allowlists and auto mode. Dismissable, and it never changes anything.
3. **Background service reconnecting** (0022): calm. "Reconnecting… agents keep running."
4. **Background service stopped** (reboot or crash): it links to the recovery flow (brief 16).
4b. **Service update ready** (0022): after the user rebuilds Crewboard, the running service is older than the app.
    "Service update ready · Restart now (3 agents working, 2 waiting)". Calm, not urgent; the user picks the moment.
    Variant: **Restart required** (the app can't talk to the old service; only the Restart button works). After the
    restart, Dev tickets are Interrupted with Resume all (brief 16).
5. **Toasts** for completed actions: PR opened (#481), Local merge done, 2 waiting tickets started, ticket created by
   agent, run started.
6. **Error toasts and inline errors:** `gh` not logged in (with the `gh auth login` hint), merge conflict,
   setup command failed (with the "Start interrupted, Retry" link).
7. **OS notifications** (macOS, Windows, Linux styles): a permission prompt waiting, an agent question, a
   session interrupted. Clicking opens the ticket with the terminal focused.

## States
- Stacking rules: what happens if the usage banner and the reconnecting banner are both active
- Light and dark

## M12 additions (design update K; decisions 0063, 0064, 0068)
- **OS notifications** also for: a validator waiting on a permission prompt, "Validation failed 3 times", "Validation ended
  without a result", an Auto-merge stop, and a Project agent waiting on a prompt or question. None for failed rounds that go
  back to the agent, or for auto-merges.
- **Toasts:** "SHOP-40.1 merged automatically into the Wishlist branch", "Validation cancelled: SHOP-29's code changed",
  "Created SHOP-41 from a validation note".
