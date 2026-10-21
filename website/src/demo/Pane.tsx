import { useState } from "react";

import type { Line } from "./data";
import { Ic } from "./Ic";
import type { DemoStore } from "./state";
import type { BoardView } from "./view";

/** Terminal lines, as the `.tl` rows of the canvas. */
export function Lines({ lines }: { lines: readonly Line[] }) {
  return lines.map((l, i) => (
    // Lines are only ever appended, so their place is their identity.
    <div key={i} className={"tl " + l.c}>
      {l.t}
    </div>
  ));
}

/** The Project agent pane left of the board, or the thin strip when it's hidden. */
export function Pane({ v, store, hidden }: { v: BoardView; store: DemoStore; hidden: boolean }) {
  if (hidden)
    return (
      <div className="strip">
        <button
          className="ibtn"
          type="button"
          onClick={() => {
            store.togglePane();
          }}
          aria-label="Show the Project agent pane"
          style={{ width: 22, height: 22 }}
          data-focus-key="pane-toggle"
        >
          <Ic name="bot" />
        </button>
        {v.paneAttn && <span className="dot" style={{ background: "var(--attn)" }}></span>}
      </div>
    );
  const a = v.agent;
  return (
    <aside className="pane" aria-label="Project agents" tabIndex={-1} data-home="pane">
      <div className="pane-h">
        <span className="pane-t">Project agents</span>
        <button
          className="btn btn-g btn-sm"
          type="button"
          onClick={() => {
            store.newAgent();
          }}
        >
          <Ic name="plus" cls="s12" />
          New agent
        </button>
      </div>
      <div className="atabs" role="tablist">
        {v.agentTabs.map((t) => (
          <button
            key={t.id}
            className={"atab" + (t.on ? " on" : "")}
            type="button"
            role="tab"
            aria-selected={t.on}
            onClick={() => {
              store.selectAgent(t.id);
            }}
          >
            {t.attn && <span className="dot" style={{ background: "var(--attn)" }}></span>}
            {t.working && <Ic name="loader" cls="s12 spin" style={{ color: "var(--work)" }} />}
            {t.name}
          </button>
        ))}
      </div>
      <div className="ag-h">
        <Ic name="bot" cls="s16" style={{ color: "var(--tx2)" }} />
        <span className="ag-n">{a.name}</span>
        <span className={"pill " + a.pillCls}>{a.pillLabel}</span>
      </div>
      <div className="ag-sub">
        <span className="av av-CC">CC</span>
        <span>Mira · Claude Code · opus</span>
        <span>·</span>
        <span className="mono">{a.tokens}</span>
      </div>
      {a.created.length > 0 && (
        <div className="created">
          <span>Created</span>
          {a.created.map((k) => (
            <button
              key={k}
              className="klink"
              type="button"
              onClick={() => {
                store.openTicket(k);
              }}
            >
              {k}
            </button>
          ))}
          <span style={{ color: "var(--tx3)" }}>· in Backlog</span>
        </div>
      )}
      <div className="pterm">
        {a.isNew && (
          <div className="empty">
            <span>
              Ask about <b>{v.proj.name}</b>. The agent reads the code, investigates and proposes Backlog
              tickets. It never edits code.
            </span>
            {v.sugg.map((text) => (
              <button
                key={text}
                className="sugg"
                type="button"
                onClick={() => {
                  store.sendToAgent(text);
                }}
              >
                {text}
              </button>
            ))}
          </div>
        )}
        <Lines lines={a.lines} />
        {a.askQ !== null && (
          <div className="opts">
            <span className="opts-t">
              <Ic name="message-q" cls="s12" />
              {a.askQ}
            </span>
            {a.opts.map((o, i) => (
              <button
                key={o.act}
                className="opt"
                type="button"
                onClick={() => {
                  store.answer(o.act);
                }}
              >
                <span className="n">{i + 1}</span>
                {o.label}
                <span className="rec">{o.rec}</span>
              </button>
            ))}
          </div>
        )}
      </div>
      <Compose name={a.name} store={store} />
      <div className="pane-f">
        <span>Reads code · writes Backlog tickets only</span>
        <span className="mono">⌘L</span>
      </div>
    </aside>
  );
}

function Compose({ name, store }: { name: string; store: DemoStore }) {
  const [draft, setDraft] = useState("");
  const send = () => {
    store.sendToAgent(draft);
    if (draft.trim()) setDraft("");
  };
  return (
    <div className="compose">
      <input
        type="text"
        placeholder={"Message " + name + "…"}
        aria-label="Message the Project agent"
        value={draft}
        onChange={(e) => {
          setDraft(e.target.value);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            send();
          }
        }}
      />
      <button className="ibtn" type="button" onClick={send} aria-label="Send">
        <Ic name="send" />
      </button>
    </div>
  );
}
