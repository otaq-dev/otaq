# 02: App shell and project switcher

Read `00-README.md` and `01-design-system.md` first.

## Context
Otaq manages **several projects** (one git repo each), with **one board per project** (0024). The user
switches between projects in the dashboard. Some things are **global across all projects**: the "Needs
you" inbox, usage tracking and the plan-limit banner, and the optional agent limits (0016, 0020, 0024).

## Design
1. **Window layout**, the overall frame:
   - Window chrome for macOS (traffic lights) and Windows (caption buttons)
   - A **header or top bar**
   - The **project switcher**
   - The main area (the board, see brief 03)
   - The **side panel** slot on the right (brief 05)
   - A place for **banners** (brief 11)
2. **Header contents:**
   - The current project name and repo path or branch (base branch, for example `main`)
   - A **Needs-you inbox** button with a global count (brief 10)
   - **Usage today:** total tokens across all projects, for example "1.84M tokens today". An optional dollar
     estimate appears only if enabled in settings (0020).
   - **Plan meter** (0020): "5h · 82% · week · 40%" with reset times on hover; amber near the limit. Pro/Max only, hidden
     for API-key users.
   - The **optional agent limits** control. Off by default (0016). When enabled, it shows "3 / 5 agents" and
     can be adjusted inline.
   - New ticket, Project terminal (brief 17), Settings
3. **Project switcher:**
   - It shows, for every project, its name and key (`shop-web · SHOP`, 0029), the number of **agents working**, and the number of
     **items needing the user**.
   - The current project is highlighted, and it can be keyboard-driven (quick switcher).
   - Add project, which leads into the onboarding flow (brief 14).
   - A project with nothing happening (for example `otaq`) should look quiet.
   - Propose the form: a sidebar, a top dropdown, or a tab strip. Explain your choice for 3 to 10 projects.

## States
- One project vs. many (3, and 10 with long names)
- No projects yet (the first-run empty state, which links to brief 14)
- The background service is not reachable: the app is reconnecting, and agents keep running in the
  background (0022). Show a non-alarming "reconnecting…" state.
- Another project needs attention while you're looking at this one. The switcher badge must make that obvious.
- The usage-limit banner is active (brief 11) and pushes or overlays the content.

## Deliverables
The full window in light and dark, at 1440×900, 1280×800 and 1920×1080. The switcher in every state.

## M12 additions (design update K; decision 0068)
- The header's **Project terminal** button opens the surface where the project's **Project terminals and Project agents** live
  (brief 17, 18), with "+" → New terminal / New Project agent.
- The project switcher counts Project agents waiting for the user like other needs-you items.
- The button's name is **"Terminals & agents"** (tooltip ⇧⌘T), with a turquoise dot when a Project agent waits for the user.
