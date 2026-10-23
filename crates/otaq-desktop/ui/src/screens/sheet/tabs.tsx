// The sheet's tab bodies until each tab's own ticket lands (T-0464): each shows today's panel content so nothing is
// lost mid-milestone. Overview (T-0465 replaces it): the action bar or the question with its options, the callouts,
// the choices and the path, the sections, then today's Ticket tab. Session (T-0466): Agent · Run · Shell over
// today's Terminal, Run and Shell tabs. Files (T-0468): today's Diff tab. Activity (T-0466): History. An epic's Lead
// (T-0469): the Lead tab. A ticket that replaces one of these registers its own body in place of this file's.
import { useRegistries } from "../../app/registry";
import type { TicketProps } from "../../app/registry";
import { showTab } from "../../app/selection";
import type { PanelTab } from "../../generated/PanelTab";
import type { SheetTab } from "../../generated/SheetTab";
import { PanelOverview } from "../panel/Panel";
import { tabIndicator } from "../panel/logic";
import { contentOf } from "./logic";
import { useSheet } from "./Sheet";

/** Today's panel tab `content` for the open ticket, as its screen registered it. */
function Content({ content, ticket }: { content: PanelTab | undefined; ticket: string }) {
  const app = useRegistries();
  const Body = content === undefined ? undefined : app.panelTabs.get(content)?.Component;
  return Body ? <Body ticket={ticket} /> : null;
}

export function OverviewTab({ ticket }: TicketProps) {
  const sheet = useSheet();
  const app = useRegistries();
  // An epic's Messages sit with its Ticket content (accepted Q9).
  const Messages = app.panelSections.get("messages")?.Component;
  return (
    <div className="sheet-overview">
      {sheet && <PanelOverview view={sheet.view} actions={sheet.actions} />}
      <div className="sheet-overview-content">
        {sheet?.view.epic && Messages && <Messages ticket={ticket} />}
        <Content content="ticket" ticket={ticket} />
      </div>
    </div>
  );
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

/** A tab that shows one of today's panel tabs whole (Files, Activity, Lead). */
function single(tab: SheetTab) {
  return function SingleTab({ ticket }: TicketProps) {
    const sheet = useSheet();
    const shown = sheet?.view.sheet.tabs.find((t) => t.tab === tab);
    return <Content content={shown?.contents[0]} ticket={ticket} />;
  };
}

export const FilesTab = single("files");
export const ActivityTab = single("activity");
export const LeadSheetTab = single("lead");
