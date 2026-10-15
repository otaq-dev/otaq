# 0080. Epics on the board: a violet sleeve on children, an "Epics in flight" strip, and the epic drag queues its children

- **Status:** Accepted
- **Date:** 2026-10-15
- **Related:** 0016, 0039 (partly superseded), 0044, 0047, 0049, 0052, 0058, 0059 (partly superseded), 0063, 0064, 0065,
  0066, 0068, 0077
- **Supersedes:** from 0039, "An epic lives only in Backlog. It can't be dragged to Dev."; from 0059 step 5, "While
  children work, it stays in Backlog with its progress"

## Context
People import their work from GitHub Issues, Jira or Azure Boards. One human ticket (Jira **PAY-123 "Wishlist"**) is
often several agent tickets, so it becomes an epic (SHOP-40, External ID PAY-123) with children SHOP-40.1, 40.2, 40.3
(0059). Two gaps were left:
- **Telling them apart.** A standalone ticket (SHOP-51) and an epic child (SHOP-40.2) differed only by a dot in the key and
  a small chip. With a few epics in flight, the user couldn't see what belongs together, how far each epic is, or what
  ships on its own.
- **Starting an epic.** The epic card couldn't be dragged (0039), so every child had to be queued by hand, and it wasn't
  obvious how to let the app work through an epic until it's finished.

A design exploration in `crewboard.pen` ("2. Epics on the board · exploration B") compared four directions. The user chose
**D4 · Epic tracks** (grilling session, 2026-10-15).

## Decision
**Telling children apart**
- Every epic child's card wears a **violet sleeve**: a band on top with the epic's name and the child's position
  ("Wishlist · 2 of 3"). It replaces the epic chip. The key keeps its dot (SHOP-40.2) for search and the CLI.
- **No sleeve = a standalone ticket**, a delivery unit that ships on its own.
- **Violet** is the existing epic colour (the Epic pill). It also keeps its other meaning, "made by an agent" (the fading
  glow on a ticket a Project agent just created, 0068, and small agent icons). The glow fades and the sleeve stays and
  carries the epic's name, so the two don't compete. No per-epic colours: every other hue already means a status, in 15
  themes.

**The "Epics in flight" strip**
- A strip above the columns has one **track** per epic in flight. Each child is a **stop** placed under the column it's in
  (for example "40.3 waits 40.2" under Ready for Dev, "40.2" under Dev, "40.1" under Done). It shows how far each epic is,
  aligned with the columns.
- It exists only while an epic is in flight. Clicking the strip's header folds it to one line ("Epics in flight 2 · 1
  needs you"). Order: pinned epics first (0047), then tracks with a child that needs the user, then the rest.
- Hovering a stop outlines its card. Clicking a stop selects that child. Clicking a track's name **focuses** the epic (its
  cards stay, the other cards dim so the board keeps its shape; Esc leaves). Double-click or ↵ opens the epic panel.
- A child that needs the user is a turquoise card as always, and its stop turns turquoise too.

**The epic moves as one thing: card → track → card**
- **Before it starts**, the epic is a card in Backlog. Its children are rows inside it, not separate cards, so the Backlog
  duplicate is gone.
- **Drag the epic to Ready for Dev (or Dev):** every never-started child is queued, and the card becomes a track. Children
  then follow the normal rules: dependencies (0049), the optional limit (0016) and the start order (0065). The user's drag
  is the go-ahead, so 0016 and 0066 ("the factory never adds tickets to the queue by itself") still hold.
- **Drag one child row out of the epic card:** only that child is queued, and the epic becomes a track with the others
  shown as "not queued" stops. Dragging the track to Ready for Dev later queues the rest.
- **Every epic queue shows a toast** with **Undo**: "Queued 2 children of SHOP-45 · Undo". Undo is the same as dragging the
  track back: it takes back only children that haven't started; if one already started, the toast says so ("SHOP-45.1
  already started"). No ⌘Z: that stays "undo typing" in text fields.
- **A child added while the whole epic is queued** (by hand, or by an agent's `otaq create --parent SHOP-40`) **joins the
  queue on its own**. Its stop is flagged "new · added by SHOP-40.2" (or "by you"). The user can drag it back to Backlog.
  After a single-child start, new children stay in Backlog.
- **Drag the track back to Backlog:** never-started children are taken out of the queue, and running children keep going.
  If nothing had started, the track turns back into the Backlog card. **If every child has started**, the drop asks for
  one confirmation, then **parks every running child** (0052's Park, for all of them at once), and the track turns back
  into the Backlog card. Its rows show **"Parked · has work"**. Dragging the epic to Ready for Dev again **resumes** the
  parked children (same conversations, 0052) and queues any never-started ones; dragging one parked row resumes only that
  child.
- **When the last child merges**, the track ends and the epic becomes a card in **Ready for Review** (0059 step 3). Adding
  or reopening a child after that turns it back into a track. The new child follows the rules above.
- **Auto-merge off:** when an epic is queued and the project's Auto-merge is Off (0064), the same toast says so: "Queued 3
  tickets. Auto-merge is off for shop-web, so each one waits for your review." It offers **Turn on for epic children**
  next to Undo,
  which sets the project's Auto-merge to "Epic children only". *Short-term: expected to change when the work moves away
  from worktrees.*

**Done, pins and keys**
- **Merged children fold** in Done into one line per epic as soon as they merge ("Wishlist · 4 of 6 merged"). Clicking it
  shows the cards. Closed children stay as their own cards.
- **Pinning an epic** (0047) carries over to the whole epic: its track is first in the strip, and its children start first
  from Ready for Dev, as if each were pinned. The sleeve shows a small pin; the child cards don't get their own pin icons.
  Pinning a single child works as before.
- **Keys** (0058): **⇧⌘G / Ctrl+Shift+G** (also in the terminal column) focuses the epic of the selected card; Esc leaves.
  Queueing an epic uses Start / move to Dev (**⇧⌘D**), as for a ticket. Folding the strip has no shortcut.

**Unchanged:** one level only; a child never changes epic; an epic has no agent, terminal or worktree of its own; children
merge locally into the epic branch and have no Open PR; Backlog never starts by itself; epics and standalone tickets are the
delivery units.

## Alternatives considered
- **Flat board with tagged children (D1):** the epic card would stay in Backlog, so dragging it would be a command that springs
  back, and unqueueing would be menu-only. Its sleeve is kept.
- **Swimlanes (D2):** clear, but a second board view with its own drag rules, and a lot of height. Kept as a possible later
  view.
- **Epic as one moving card (D3):** rejected. An epic in Dev would hold merged, working and waiting children, which breaks
  "one column = one state" that the scheduler, the agent count and the inbox rely on.
- **A per-epic shape or colour** (exploration A): rejected; the sleeve with the name is enough, and colours are taken.
- **New children wait in Backlog** (the design's suggestion): rejected. An epic could stall waiting for a click the user
  didn't know was needed.
- **Refusing the drag back when everything has started:** rejected in favour of "Park all".
- **⇧⌘E to fold the strip and ⌘E to focus an epic** (the design's keys): ⇧⌘E is Expand panel, and ⌘E's terminal form
  (Ctrl+Shift+E) is too.

## Consequences
- A new board part (the strip and its tracks), a new card part (the sleeve), draggable child rows on the epic card, a
  folded Done line, and a bulk Park.
- The epic chip on child cards and the "Epics can't be dragged" rule (SDD §7.3, §7.4) go away.
- The epic filter stays and is the same as focusing an epic: an `epic:` filter dims the other cards instead of hiding
  them. Other filter words still hide.
- Design (update Q): `crewboard.pen` → "01 · Components · Epics on the board (0080)", the strip on "03 · Board — shop-web ·
  1440" (light and dark), "03 · Board & columns" → the "(0080)" rows (flows 1–7, states 8–13, 1280 × 800), and "04 · Ticket
  card variants" → the "(0080)" rows. The exploration pages are background only. `design-briefs/CHANGES-2026-10-15-q.md`
  has the frame map and the audit.

## Changelog
- 2026-10-15: created (grilling session, user decision)
- 2026-10-15: design audit of update Q (user decisions): strip order is pinned → needs-you (wording fix, matches 0047);
  after Park all the epic card's rows show "Parked · has work" and dragging the epic resumes them; every epic queue shows
  a toast with Undo (no ⌘Z).
- 2026-10-15: focusing an epic (and the `epic:` filter) dims the other cards instead of hiding them, as flow 6 draws (M16
  planning, Q55, user decision).
