// The sheet's tab bodies until each tab's own ticket lands (T-0464): each shows today's panel content so nothing is
// lost mid-milestone. Session (T-0466): Agent · Run · Shell over today's Terminal, Run and Shell tabs. Activity
// (T-0466): History. An epic's Lead (T-0469): the Lead tab. A ticket that replaces one of these registers its own body
// in place of this file's (Overview is `screens/overview`, T-0465; Files `screens/files`, T-0468).
import { useRegistries } from "../../app/registry";
import type { TicketProps } from "../../app/registry";
import { showTab } from "../../app/selection";
import type { PanelTab } from "../../generated/PanelTab";
import type { SheetTab } from "../../generated/SheetTab";
import { tabIndicator } from "../panel/logic";
import { contentOf } from "./logic";
import { useSheet } from "./Sheet";

/** Today's panel tab `content` for the open ticket, as its screen registered it. */
function Content({ content, ticket }: { content: PanelTab | undefined; ticket: string }) {
  const app = useRegistries();
  const Body = content === undefined ? undefined : app.panelTabs.get(content)?.Component;
  return Body ? <Body ticket={ticket} /> : null;
}

/** Session until T-0466: the switch Agent · Run · Shell, and the one picked. */
export function SessionTab({ ticket }: TicketProps) {
  const sheet = useSheet();
  const session = sheet?.view.sheet.tabs.find((t) => t.tab === "session");
  if (!sheet || !session) return null;
  const shown = contentOf(session, sheet.selected);
  return (
    <div className="sheet-session">
      <div className="segmented sheet-session-switch" role="radiogroup" aria-label={session.label}>
        {session.parts.map((part) => {
          const mark = tabIndicator(part.mark);
          return (
            <button
              key={part.content}
              type="button"
              role="radio"
              className="segment"
              aria-checked={part.content === shown}
              onClick={() => {
                showTab(part.content);
              }}
            >
              {part.label}
              {mark &&
                (mark.kind === "note" ? (
                  <span className="tab-note">{mark.text}</span>
                ) : (
                  <span className="tab-dot" data-kind={mark.kind} />
                ))}
            </button>
          );
        })}
      </div>
      <div className="sheet-session-body" data-content={shown}>
        <Content content={shown} ticket={ticket} />
      </div>
    </div>
  );
}

/** A tab that shows one of today's panel tabs whole (Activity, Lead). */
function single(tab: SheetTab) {
  return function SingleTab({ ticket }: TicketProps) {
    const sheet = useSheet();
    const shown = sheet?.view.sheet.tabs.find((t) => t.tab === tab);
    return <Content content={shown?.contents[0]} ticket={ticket} />;
  };
}

export const ActivityTab = single("activity");
export const LeadSheetTab = single("lead");
