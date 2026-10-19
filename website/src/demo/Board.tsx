import type { DragEvent } from "react";

import { Ic, S10 } from "./Ic";
import type { DemoStore } from "./state";
import type { BoardView, CardView, ColumnView, Moment } from "./view";

interface BoardProps {
  v: BoardView;
  store: DemoStore;
  moment: Moment;
  setMoment: (m: Moment | ((m: Moment) => Moment)) => void;
}

/** The epic strip, the columns and the folded Done line. */
export function Board({ v, store, moment, setMoment }: BoardProps) {
  return (
    <>
      {v.epic && (
        <div className="epics">
          <span className="ep-t">Epics in flight · 1</span>
          <div className="ep">
            <span className="ep-n">
              <Ic name="layers" />
              {v.epic.name}
            </span>
            <span className="ep-l">
              <span className="mono">{v.epic.progress}</span> merged ·{" "}
              <Ic name="bot" cls="s12" style={{ verticalAlign: "-2px" }} /> {v.epic.lead}
            </span>
            <div className="stops">
              {v.epic.stops.map((s) => (
                <button
                  key={s.k}
                  className="stop"
                  type="button"
                  onClick={() => {
                    store.openTicket(s.k);
                  }}
                  style={{ color: s.color }}
                >
                  <Ic name={s.icon} cls="s12" />
                  {s.label}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
      <div className="cols">
        {v.columns.map((col) => (
          <BoardColumn key={col.id} col={col} store={store} moment={moment} setMoment={setMoment} />
        ))}
        {!v.columns.some((c) => c.id === "done") && (
          <button
            className="donerail"
            type="button"
            onClick={() => {
              store.showDone();
            }}
          >
            <Ic name="circle-check" style={{ color: "var(--done)" }} />
            <span>Done</span>
            <span className="cnt" style={{ writingMode: "horizontal-tb" }}>
              {v.doneCount}
            </span>
          </button>
        )}
      </div>
    </>
  );
}

function BoardColumn({ col, store, moment, setMoment }: { col: ColumnView } & Omit<BoardProps, "v">) {
  const over = (e: DragEvent) => {
    if (!moment.dragKey) return;
    e.preventDefault();
    if (moment.dropCol !== col.id) setMoment((m) => ({ ...m, dropCol: col.id }));
  };
  const drop = (e: DragEvent) => {
    e.preventDefault();
    const k = moment.dragKey;
    setMoment((m) => ({ ...m, dragKey: null, dropCol: null }));
    if (k) store.move(k, col.id);
  };
  return (
    // A drop target for the mouse; from the keyboard, Move to Dev in the ticket's panel does the same.
    // eslint-disable-next-line jsx-a11y/no-static-element-interactions -- drag and drop only; the panel is the keyboard path
    <div className={("colm " + col.cls).trim()} onDragOver={over} onDrop={drop}>
      <div className="col-h">
        <Ic name={col.icon} style={{ color: col.color }} />
        {col.title}
        <span className="n">{col.count}</span>
      </div>
      <div className="col-s">{col.sub}</div>
      <div className="col-b">
        {col.coach && (
          <div className="coach">
            <Ic name="mouse" style={{ color: "var(--ac)" }} />
            <span>
              <b>Try it:</b> drag NOTE-6 to Dev and watch an agent start.
            </span>
            <button
              className="ibtn"
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                store.hideCoach();
              }}
              aria-label="Dismiss"
            >
              <Ic name="x" cls="s12" />
            </button>
          </div>
        )}
        {col.sections.map((sec) => (
          <Section key={sec.title ?? "cards"} sec={sec} store={store} moment={moment} setMoment={setMoment} />
        ))}
      </div>
    </div>
  );
}

function Section({
  sec,
  store,
  moment,
  setMoment,
}: { sec: ColumnView["sections"][number] } & Omit<BoardProps, "v">) {
  return (
    <>
      {sec.title && (
        <div className="sec-h">
          <Ic name={sec.icon} cls="s12" style={{ color: sec.color }} />
          {sec.title} <span className="cnt">{sec.cards.length}</span>
        </div>
      )}
      {sec.cards.map((c) => (
        <Card key={c.k} c={c} store={store} moment={moment} setMoment={setMoment} />
      ))}
      {sec.cards.length === 0 && <div className="ph">{sec.emptyText}</div>}
    </>
  );
}

function Card({ c, store, moment, setMoment }: { c: CardView } & Omit<BoardProps, "v">) {
  const open = (e: { stopPropagation(): void }) => {
    e.stopPropagation();
    store.openTicket(c.k);
  };
  return (
    // The whole card opens the ticket for the mouse, as drawn; its title is the button that does it from the keyboard.
    // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- the title button inside is the keyboard path
    <div
      className={("cd " + c.cls).trim()}
      draggable={c.movable}
      onDragStart={(e) => {
        try {
          e.dataTransfer.setData("text/plain", c.k);
          e.dataTransfer.effectAllowed = "move";
        } catch {
          // Some browsers refuse data on a drag; the demo keeps the dragged key itself.
        }
        setMoment((m) => ({ ...m, dragKey: c.k, hoverKey: null }));
      }}
      onDragEnd={() => {
        setMoment((m) => ({ ...m, dragKey: null, dropCol: null }));
      }}
      onMouseEnter={() => {
        if (!moment.dragKey) setMoment((m) => ({ ...m, hoverKey: c.k }));
      }}
      onMouseLeave={() => {
        setMoment((m) => ({ ...m, hoverKey: null }));
      }}
      onClick={open}
    >
      <div className="cd-top">
        <span className="key">{c.k}</span>
        {c.showPrio && (
          <span className={"prio p" + String(c.prio)}>
            <Ic name={c.prioIcon} style={S10} />P{c.prio}
          </span>
        )}
        {c.epicPos && (
          <span className="epchip">
            <Ic name="layers" style={S10} />
            {c.epicPos}
          </span>
        )}
        <span style={{ flex: 1 }}></span>
        <span className={"av av-" + c.tool} title={c.agentLabel}>
          {c.tool}
        </span>
      </div>
      <button className="cd-ti" type="button" onClick={open} data-focus-key={"card:" + c.k}>
        {c.title}
      </button>
      {c.pill && (
        <span className={"pill " + c.pill.cls}>
          <Ic name={c.pill.icon} cls={c.pill.spin ? "spin" : ""} />
          {c.pill.label}
        </span>
      )}
      {c.conflict && (
        <div className="warnbox">
          <Ic name="git-compare" cls="s12" style={{ marginTop: 1 }} />
          <span>Conflicts with main · the agent rebases before merging</span>
        </div>
      )}
      {c.metas.map((m) => (
        <div key={m.i + m.t} className={("meta " + m.cls).trim()}>
          <Ic name={m.i} cls="s12" />
          <span>{m.t}</span>
        </div>
      ))}
      {c.isReview && (
        <button
          className="btn btn-p btn-sm"
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            store.openTicket(c.k, "diff");
          }}
          style={{ width: "100%" }}
        >
          <Ic name="eye" cls="s12" />
          Review
        </button>
      )}
    </div>
  );
}
