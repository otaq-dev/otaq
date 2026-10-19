import { type Ref, useState } from "react";

import { Ic, S10 } from "./Ic";
import { Lines } from "./Pane";
import type { DemoState, DemoStore } from "./state";
import { otherTab, PANEL_TABS, type PanelView } from "./view";

interface PanelProps {
  t: PanelView;
  s: DemoState;
  branch: string;
  projName: string;
  store: DemoStore;
  panelRef: Ref<HTMLElement>;
}

/** The ticket panel over the right of the board: header, actions for the ticket's state, and its tabs. */
export function Panel({ t, s, branch, projName, store, panelRef }: PanelProps) {
  const tip = () => {
    store.say("Try this one in the app.");
  };
  const other = otherTab(s);
  return (
    <aside className="panel" aria-label={"Ticket " + t.k} ref={panelRef} tabIndex={-1} data-home="panel">
      <div className="pn-h">
        <div className="pn-ti">
          <span className="key">{t.k}</span>
          {t.title}
        </div>
        <span className={"prio p" + String(t.prio)} style={{ marginTop: 4 }}>
          <Ic name={t.prioIcon} style={S10} />P{t.prio} {t.prioWord}
        </span>
        <button className="ibtn" type="button" onClick={tip} aria-label="Expand">
          <Ic name="maximize" />
        </button>
        <button
          className="ibtn"
          type="button"
          onClick={() => {
            store.closePanel();
          }}
          aria-label="Close"
        >
          <Ic name="x" cls="s16" />
        </button>
      </div>
      <div className="pn-row">
        <span className={"pill " + t.pill.cls}>
          <Ic name={t.pill.icon} cls={t.pill.spin ? "spin" : ""} />
          {t.pill.label}
        </span>
        <span className="chip">
          <Ic name="git-branch" cls="s12" />
          otaq/{t.k}
        </span>
        <span style={{ fontSize: 11, color: "var(--tx3)" }}>from {branch}</span>
        {t.epicPos && (
          <span className="epchip">
            <Ic name="layers" style={S10} />
            {t.epicPos}
          </span>
        )}
        <span style={{ flex: 1 }}></span>
        <span className="mono" style={{ fontSize: 11, color: "var(--tx3)" }}>
          {t.tokens}
        </span>
      </div>
      <div className="pn-ag">
        <div>
          <span className={"av lg av-" + t.tool}>{t.tool}</span>Developer <b>{t.agentLabel}</b>
        </div>
        <div>
          <span className="av lg av-CP">CP</span>Validator <b>Vera · Copilot · gemini-2.5-pro</b>
        </div>
      </div>
      <div className="pn-act">
        {t.canApprove && (
          <>
            <button
              className="btn btn-p"
              type="button"
              onClick={() => {
                store.approve(t.k);
              }}
            >
              <Ic name="git-merge" />
              Approve · Local merge
            </button>
            <button
              className="btn btn-s"
              type="button"
              onClick={() => {
                store.sendBack(t.k);
              }}
            >
              <Ic name="undo" />
              Send back
            </button>
          </>
        )}
        {t.canStart && (
          <>
            <button
              className="btn btn-p"
              type="button"
              onClick={() => {
                store.move(t.k, "dev");
              }}
            >
              <Ic name="play" />
              Move to Dev
            </button>
            <span className="note">or drag the card to Dev</span>
          </>
        )}
        {t.isAsking && (
          <span className="note" style={{ color: "var(--attn)" }}>
            <Ic name="message-q" />
            The agent is waiting for your answer, below.
          </span>
        )}
        {t.isRunning && (
          <>
            <button className="btn btn-s" type="button" onClick={tip}>
              <Ic name="stop" />
              Stop
            </button>
            <span className="note">
              <Ic name="folder-git" />
              <span className="mono">
                ~/.otaq/wt/{projName}/{t.k}
              </span>
            </span>
          </>
        )}
        {t.isDone && (
          <span className="note">
            <Ic name="git-merge" style={{ color: "var(--ok)" }} />
            Squash-merged into {branch}
          </span>
        )}
      </div>
      <div className="ptabs" role="tablist">
        {PANEL_TABS.map(([id, label, icon]) => (
          <button
            key={id}
            className={"ptab" + (s.ptab === id ? " on" : "")}
            type="button"
            role="tab"
            aria-selected={s.ptab === id}
            onClick={() => {
              store.setPanelTab(id);
            }}
          >
            <Ic name={icon} />
            {label}
          </button>
        ))}
      </div>
      <div className="pbody">
        {s.ptab === "terminal" && (
          <div className="tterm" style={{ fontSize: s.fontSize }}>
            <div className="term-bar">
              <span className="sess on">Agent</span>
              <span className="sess">Validator · round {t.round}</span>
              <span style={{ flex: 1 }}></span>
              <span className="mono">otaq attach {t.k}</span>
            </div>
            {t.noSession && (
              <div className="tl d">No session yet. The agent starts when this ticket moves to Dev.</div>
            )}
            <Lines lines={t.lines} />
            {t.ask && (
              <div className="opts">
                <span className="opts-t">
                  <Ic name={t.ask.icon} cls="s12" />
                  {t.ask.title}
                </span>
                {t.ask.opts.map((o, i) => (
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
            <span className="grow"></span>
            {/* A new prompt per ticket: opening another ticket starts with an empty line. */}
            <TerminalPrompt key={t.k} store={store} />
          </div>
        )}
        {s.ptab === "diff" &&
          (t.files.length > 0 ? (
            <>
              <div className="dstat">
                <span>Against the commit where {t.k} started</span>
                <span style={{ flex: 1 }}></span>
                <span className="add">+{t.adds}</span>
                <span className="del">−{t.dels}</span>
                <span>{t.files.length} files</span>
              </div>
              {t.files.map((f) => (
                <div key={f.name} className="dfile">
                  <div className="dfile-h">
                    <Ic name="file-diff" cls="s12" />
                    {f.name}
                    <span style={{ flex: 1 }}></span>
                    <span className="add">+{f.a}</span>
                    <span className="del">−{f.d}</span>
                  </div>
                  {f.lines.map(([c, n, text], i) => (
                    // A diff never changes while it is shown.
                    <div key={i} className={("dl " + c).trim()}>
                      <span className="ln">{n}</span>
                      <span className="tx">{text}</span>
                    </div>
                  ))}
                </div>
              ))}
            </>
          ) : (
            <div className="phd">
              <Ic name="file-diff" cls="s16" />
              No changes yet. The diff shows up here once the agent commits.
            </div>
          ))}
        {s.ptab === "ticket" && (
          <>
            {t.decision && (
              <div className="dec">
                <div className="dec-h">
                  <Ic name="lightbulb" style={{ color: "var(--inv)" }} />
                  Decision recorded by the agent
                </div>
                <div className="prose" style={{ margin: 0 }}>
                  {t.decision}
                </div>
              </div>
            )}
            <div className="slab">What and why</div>
            <p className="prose">{t.desc}</p>
            <div className="slab">Acceptance</div>
            <ul className="ul">
              {t.acc.map((a) => (
                <li key={a}>{a}</li>
              ))}
            </ul>
            <div className="slab">Depends on</div>
            <p className="prose" style={{ color: "var(--tx2)" }}>
              {t.depsText}
            </p>
          </>
        )}
        {s.ptab === "history" &&
          t.hist.map((h, i) => (
            // History entries are only appended.
            <div key={i} className="hist">
              <span className="t">{h.at}</span>
              <span>{h.t}</span>
            </div>
          ))}
        {(s.ptab === "run" || s.ptab === "shell") && (
          <div className="phd">
            <Ic name={other.icon} cls="s16" />
            <b style={{ color: "var(--tx)" }}>{other.title}</b>
            <span>{other.text}</span>
          </div>
        )}
      </div>
    </aside>
  );
}

function TerminalPrompt({ store }: { store: DemoStore }) {
  const [draft, setDraft] = useState("");
  return (
    <div className="tprompt">
      <span aria-hidden="true">❯</span>
      <input
        type="text"
        placeholder="Type to the agent…"
        aria-label="Type to the agent"
        value={draft}
        onChange={(e) => {
          setDraft(e.target.value);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" && draft.trim()) {
            e.preventDefault();
            store.typeToTicket(draft);
            setDraft("");
          }
        }}
      />
    </div>
  );
}
