import { Fragment, type Ref } from "react";

import { AGENTS, type AgentName, project, PROJECTS } from "./data";
import { Ic } from "./Ic";
import type { DemoState, DemoStore, ProjectField } from "./state";
import {
  type InboxGroup,
  KEYS,
  NAMED_AGENTS,
  ROLES,
  SETTINGS_GLOBAL,
  settingsProject,
  settingsTitle,
  THEMES,
  toolRows,
} from "./view";

/** The Needs you inbox under the header: its groups, or "Nothing needs you". */
export function Inbox({
  inbox,
  store,
  popRef,
}: {
  inbox: InboxGroup[];
  store: DemoStore;
  popRef: Ref<HTMLDivElement>;
}) {
  return (
    <div className="pop" role="dialog" aria-label="Inbox" ref={popRef} tabIndex={-1} data-home="inbox">
      <div className="pop-h">
        <Ic name="inbox" cls="s16" />
        <b>Inbox</b>
        <span className="cnt">all projects</span>
        <button
          className="ibtn"
          type="button"
          onClick={() => {
            store.toggleInbox();
          }}
          aria-label="Close inbox"
        >
          <Ic name="x" />
        </button>
      </div>
      <div className="pop-b">
        {inbox.map((g) => (
          <Fragment key={g.title}>
            <div className="grp">
              <span className="dot" style={{ background: g.color }}></span>
              {g.title} <span className="cnt">{g.items.length}</span>
            </div>
            {g.items.map((it) => (
              <button
                key={it.pk + it.k + it.title}
                className="item"
                type="button"
                onClick={() => {
                  if ("agent" in it.go) store.showAgent(it.go.agent);
                  else store.openTicket(it.go.ticket, it.go.tab);
                }}
              >
                <span className="bar" style={{ background: g.color }}></span>
                <span className="main-i">
                  <span className="t1">
                    <span className="pk">{it.pk}</span>
                    <span className="key">{it.k}</span>
                    <b>{it.title}</b>
                  </span>
                  <span className="t2">{it.reason}</span>
                </span>
                <span className="age">{it.age}</span>
              </button>
            ))}
          </Fragment>
        ))}
        {inbox.length === 0 && (
          <div className="allclear">
            <Ic name="circle-check" style={{ width: 28, height: 28, color: "var(--ok)" }} />
            <b style={{ color: "var(--tx)", fontSize: 14 }}>Nothing needs you</b>
            <span>Your agents are working. Otaq tells you when one needs you.</span>
          </div>
        )}
      </div>
    </div>
  );
}

/** Settings: its pages as drawn. */
export function Settings({
  s,
  store,
  dlgRef,
}: {
  s: DemoState;
  store: DemoStore;
  dlgRef: Ref<HTMLDivElement>;
}) {
  const say = (text: string) => () => {
    store.say(text);
  };
  const spk = settingsProject(s);
  const sp0 = project(spk);
  const pf = s.projFields[spk];
  const setPF = (field: ProjectField) => (e: { target: { value: string } }) => {
    store.setProjectField(spk, field, e.target.value);
  };
  const isProj = s.spage === "proj-NOTE" || s.spage === "proj-SHOP";
  return (
    // A click on the dimmed backdrop closes Settings, as drawn; the Close button is the keyboard path.
    // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- the backdrop is for the mouse only
    <div
      className="scrim"
      onClick={() => {
        store.closeSettings();
      }}
    >
      {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-noninteractive-element-interactions -- only stops a click inside from reaching the backdrop */}
      <div
        className="dlg"
        role="dialog"
        aria-label="Settings"
        onClick={(e) => {
          e.stopPropagation();
        }}
        ref={dlgRef}
        tabIndex={-1}
        data-home="settings"
      >
        <nav className="snav" aria-label="Settings pages">
          <h2>Settings</h2>
          {SETTINGS_GLOBAL.map(([id, label, icon]) => (
            <button
              key={id}
              className={"srow" + (s.spage === id ? " on" : "")}
              type="button"
              onClick={() => {
                store.setSettingsPage(id);
              }}
            >
              <Ic name={icon} />
              {label}
            </button>
          ))}
          <span className="gl">Projects</span>
          {PROJECTS.map((p) => (
            <button
              key={p.key}
              className={"srow" + (s.spage === "proj-" + p.key ? " on" : "")}
              type="button"
              onClick={() => {
                store.setSettingsPage(p.key === "NOTE" ? "proj-NOTE" : "proj-SHOP");
              }}
            >
              <span className="pk">{p.key}</span>
              {p.name}
            </button>
          ))}
        </nav>
        <div className="spage">
          <div className="spage-h">
            <h3>{settingsTitle(s)}</h3>
            <button
              className="ibtn"
              type="button"
              onClick={() => {
                store.closeSettings();
              }}
              aria-label="Close settings"
            >
              <Ic name="x" cls="s16" />
            </button>
          </div>
          <div className="spage-b">
            {s.spage === "tools" && (
              <>
                <p className="sdesc">
                  Otaq finds the agent tools on this computer and uses each tool’s own sign-in and permission
                  mode. It never handles your passwords or API keys.
                </p>
                {toolRows(s).map((tl) => (
                  <div key={tl.id} className="trow">
                    <span className={"av xl av-" + tl.id}>{tl.id}</span>
                    <div className="tt">
                      <b>{tl.name}</b>
                      <span>{tl.detail}</span>
                    </div>
                    <span className={"pill " + tl.pillCls}>
                      <Ic name={tl.pillIcon} />
                      {tl.status}
                    </span>
                    <button className="btn btn-s btn-sm" type="button" onClick={say(tl.tip)}>
                      {tl.action}
                    </button>
                  </div>
                ))}
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <button
                    className="btn btn-s"
                    type="button"
                    onClick={() => {
                      store.recheck();
                    }}
                  >
                    <Ic name={s.rechecking ? "loader" : "refresh"} cls={s.rechecking ? "spin" : ""} />
                    {s.rechecking ? "Checking…" : "Check again"}
                  </button>
                  <span className="sdesc">
                    What Otaq records in each tool’s config: that you trust the folders you add as projects,
                    so agents don’t stop on a trust screen.
                  </span>
                </div>
              </>
            )}
            {s.spage === "agents" && (
              <>
                <p className="sdesc">
                  Named agents are a tool, a model and a note on what each is for. Pick one per ticket, or let
                  the defaults below decide.
                </p>
                {NAMED_AGENTS.map((n) => {
                  const a = AGENTS[n];
                  return (
                    <div key={n} className="trow">
                      <span className={"av lg av-" + a.tool}>{a.tool}</span>
                      <div className="tt">
                        <b>{n}</b>
                        <span>
                          {a.toolName} · <span className="mono">{a.model}</span> · {a.use}
                        </span>
                      </div>
                      <button
                        className="ibtn"
                        type="button"
                        onClick={say(
                          "In the app you pick the tool and model, from that tool’s own model list.",
                        )}
                        aria-label={"Edit " + n}
                      >
                        <Ic name="pencil" />
                      </button>
                    </div>
                  );
                })}
                <button
                  className="btn btn-s"
                  type="button"
                  onClick={say("In the app: pick a tool, a model, and write what it’s for.")}
                  style={{ alignSelf: "flex-start" }}
                >
                  <Ic name="plus" />
                  Add agent
                </button>
                <div className="slab" style={{ marginTop: 8 }}>
                  Defaults
                </div>
                {ROLES.map(([id, label, hint]) => (
                  <div key={id} className="field">
                    <label htmlFor={"role-" + id}>{label}</label>
                    <div>
                      <select
                        className="inp"
                        id={"role-" + id}
                        value={s.roles[id]}
                        onChange={(e) => {
                          store.setRole(id, e.target.value as AgentName);
                        }}
                      >
                        {NAMED_AGENTS.map((n) => (
                          <option key={n} value={n}>
                            {n}
                          </option>
                        ))}
                      </select>
                      <div className="hint">{hint}</div>
                    </div>
                  </div>
                ))}
              </>
            )}
            {s.spage === "notifications" && (
              <div className="trow">
                <div className="tt">
                  <b>OS notifications</b>
                  <span>
                    When an agent asks you something, needs a permission, or stops. None while that ticket’s
                    terminal has focus.
                  </span>
                </div>
                <button
                  className={"sw" + (s.notif ? " on" : "")}
                  type="button"
                  role="switch"
                  aria-checked={s.notif}
                  aria-label="OS notifications"
                  onClick={() => {
                    store.toggleNotif();
                  }}
                ></button>
              </div>
            )}
            {s.spage === "appearance" && (
              <>
                <p className="sdesc">Pick one theme. The demo switches as you click.</p>
                <div className="themes">
                  {THEMES.map((th) => (
                    <button
                      key={th.id}
                      className={"theme" + (s.theme === th.id ? " on" : "")}
                      type="button"
                      onClick={() => {
                        store.pickTheme(th.id);
                      }}
                      aria-pressed={s.theme === th.id}
                    >
                      <span className="tprev" style={{ background: th.bg }}>
                        <i style={{ background: th.col }}>
                          <b style={{ background: th.card }}></b>
                          <b style={{ background: th.card }}></b>
                        </i>
                        <i style={{ background: th.col }}>
                          <b style={{ background: th.card, boxShadow: "inset 2px 0 0 #2dd4bf" }}></b>
                        </i>
                        <i style={{ background: th.col }}>
                          <b style={{ background: th.ac }}></b>
                          <b style={{ background: th.card }}></b>
                        </i>
                      </span>
                      <b>{th.name}</b>
                    </button>
                  ))}
                </div>
                <p className="sdesc">
                  13 more in the app: Catppuccin Latte, Frappé, Macchiato and Mocha, Tokyo Night, Dracula,
                  Nord, Gruvbox Dark, Rosé Pine, Rosé Pine Dawn, One Dark, Daylight and Solarized Light.
                </p>
                <div className="field">
                  <span className="fl">Terminal font size</span>
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <button
                      className="btn btn-s btn-sm"
                      type="button"
                      onClick={() => {
                        store.fontDown();
                      }}
                      aria-label="Smaller"
                    >
                      −
                    </button>
                    <span className="mono">{s.fontSize} px</span>
                    <button
                      className="btn btn-s btn-sm"
                      type="button"
                      onClick={() => {
                        store.fontUp();
                      }}
                      aria-label="Larger"
                    >
                      +
                    </button>
                  </div>
                </div>
              </>
            )}
            {s.spage === "keyboard" &&
              KEYS.map((ky) => (
                <div key={ky.t} className="kbrow">
                  <span>{ky.t}</span>
                  <span className="kbd">{ky.k}</span>
                </div>
              ))}
            {s.spage === "data" && (
              <div className="field">
                <span className="fl">Database</span>
                <div>
                  <input
                    className="inp ro"
                    type="text"
                    readOnly
                    value="~/.otaq/otaq.db"
                    aria-label="Database location"
                  />
                  <div className="hint">
                    Your normal backup covers it. Set OTAQ_HOME to use another folder.
                  </div>
                </div>
              </div>
            )}
            {s.spage === "about" && (
              <div
                style={{
                  display: "flex",
                  flexDirection: "column",
                  alignItems: "center",
                  gap: 10,
                  padding: "30px 0",
                  textAlign: "center",
                }}
              >
                <svg className="mk" width="64" height="64" viewBox="0 0 120 120" aria-hidden="true">
                  <path className="o" d="M18 106 V70 Q42 60 60 30 Q78 60 102 70 V106 Z"></path>
                  <path className="c" d="M46 62 V106 M74 62 V106"></path>
                  <path className="c" d="M60 30 V4"></path>
                  <path className="f" d="M60 4 H80 Q83 4 81 7 L77 12 L81 17 Q83 20 80 20 H60 Z"></path>
                </svg>
                <b style={{ fontSize: 20 }}>Otaq</b>
                <span style={{ color: "var(--tx2)" }}>Run your coding agents from one board</span>
                <span className="mono" style={{ color: "var(--tx3)", fontSize: 12 }}>
                  0.1.0
                </span>
                <span style={{ color: "var(--tx3)", fontSize: 12 }}>
                  © 2026 The Otaq Authors · MIT License
                </span>
              </div>
            )}
            {isProj && (
              <>
                <div className="field">
                  <span className="fl">Name</span>
                  <input className="inp ro" type="text" value={sp0.name} readOnly aria-label="Project name" />
                </div>
                <div className="field">
                  <span className="fl">Project key</span>
                  <div>
                    <input className="inp ro" type="text" value={sp0.key} readOnly aria-label="Project key" />
                    <div className="hint">Used in ticket names, like {sp0.key}-12. It can’t change.</div>
                  </div>
                </div>
                <div className="field">
                  <span className="fl">Repository</span>
                  <input className="inp ro" type="text" value={sp0.path} readOnly aria-label="Repository" />
                </div>
                <div className="field">
                  <label htmlFor="f-setup">Setup command</label>
                  <div>
                    <input
                      className="inp"
                      id="f-setup"
                      type="text"
                      value={pf.setup}
                      onChange={setPF("setup")}
                    />
                    <div className="hint">Runs in every new worktree.</div>
                  </div>
                </div>
                <div className="field">
                  <label htmlFor="f-run">Run command</label>
                  <div>
                    <input className="inp" id="f-run" type="text" value={pf.run} onChange={setPF("run")} />
                    <div className="hint">
                      Each ticket gets its own $PORT, so several dev servers run at once.
                    </div>
                  </div>
                </div>
                <div className="field">
                  <label htmlFor="f-instr">Agent instructions</label>
                  <div>
                    <textarea
                      className="inp"
                      id="f-instr"
                      value={pf.instr}
                      onChange={setPF("instr")}
                    ></textarea>
                    <div className="hint">Empty means the global instructions are used.</div>
                  </div>
                </div>
                <div className="field">
                  <span className="fl">Validation</span>
                  <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                    <button
                      className="sw on"
                      type="button"
                      role="switch"
                      aria-checked="true"
                      aria-label="Validation"
                      onClick={say("Try this one in the app.")}
                    ></button>
                    <span>Up to 3 rounds</span>
                  </div>
                </div>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
