// The flow editor's moment of interaction (T-0471; FED-01, FED-03, FED-04, FED-07; d0115): where it opens, which
// step is selected, which agent the panel shows, the drag of a step, the animation's clock and the calls each click
// makes. What it says and every rule are Rust's (`flow_editor.view`, `otaq-view`'s `flow_editor.rs`); every edit of
// the user's goes to the draft through `flow_editor.edit` / `pick` / `undo` / `import` (`flow.draft_update`).
// Nothing here starts an agent without the user's words, a chip or an Ask (d0087).
import { openOverlay } from "../../app/overlays";
import { createStore, type Store } from "../../app/store";
import type { BoardColumnEditor } from "../../generated/BoardColumnEditor";
import type { FlowEditorChipAction } from "../../generated/FlowEditorChipAction";
import type { FlowEditorEdited } from "../../generated/FlowEditorEdited";
import type { FlowEditorFrom } from "../../generated/FlowEditorFrom";
import type { FlowEditorPick } from "../../generated/FlowEditorPick";
import type { FlowEditorStageEdit } from "../../generated/FlowEditorStageEdit";
import type { FlowEditorStarted } from "../../generated/FlowEditorStarted";
import type { FlowEditorView } from "../../generated/FlowEditorView";
import { request } from "../../tauri";

/** The door's request call, or a fake one in the tests. */
export type Ask = (method: string, params?: unknown) => Promise<unknown>;

/**
 * Where the editor opens over the board (FED-09): the project, where it opened, and optionally a stage to select
 * (Edit <stage>…) or one to add a new stage after (Add a stage after <stage>, the new one selected with its details
 * open; `null`: first, after Ready for Dev). Onboarding's Flow step and the Flow setup page draw it in place instead.
 */
export interface FlowEditorOpen {
  project: string;
  from: FlowEditorFrom;
  select_stage?: string;
  add_after?: string | null;
}

/** Opens the editor over the window. */
export function openFlowEditor(open: FlowEditorOpen): void {
  openOverlay("flow_editor", open);
}

/** The project menu's Change flow…, Settings → Flow's Change flow… and a column's Change the whole flow…. */
export function changeFlowOpen(project: string): FlowEditorOpen {
  return { project, from: "project" };
}

/** How a column's ⋯ item (BRD-04) opens the editor on its project: that stage selected, a new stage after it, or the
 * whole flow. */
export function columnEditorOpen(project: string, editor: BoardColumnEditor): FlowEditorOpen {
  switch (editor.open) {
    case "stage":
      return { project, from: "project", select_stage: editor.stage };
    case "add_after":
      return { project, from: "project", add_after: editor.after };
    case "flow":
      return changeFlowOpen(project);
  }
}

/** The edit that adds the stage a column's Add a stage after <stage> asked for, on opening. */
export function addOnOpening(after: string | null): FlowEditorStageEdit {
  return after === null ? { edit: "add" } : { edit: "add", after };
}

/** The agent each project's editor panel shows, once one was asked (the page's own state, like the pane's). */
export const editorAgents: Store<Readonly<Record<string, string>>> = createStore<
  Readonly<Record<string, string>>
>({});

export function editorAgent(
  project: string,
  agents: Store<Readonly<Record<string, string>>> = editorAgents,
): string | null {
  return agents.get()[project] ?? null;
}

export function showEditorAgent(
  project: string,
  agent: string,
  agents: Store<Readonly<Record<string, string>>> = editorAgents,
): void {
  agents.update((all) => ({ ...all, [project]: agent }));
}

/** The service events that change what the editor shows. */
export const FLOW_EDITOR_EVENTS = [
  "flow_changed",
  "flow_setup_changed",
  "agents_changed",
  "agent_tools_changed",
  "project_agent_changed",
  "projects_changed",
  "ticket_changed",
  "card_status_changed",
  "settings_changed",
] as const;

/** A step's edit; resolves to the step to select next (a stage just added), else `null`. */
export async function editStage(
  project: string,
  edit: FlowEditorStageEdit,
  ask: Ask = request,
): Promise<string | null> {
  const edited = (await ask("flow_editor.edit", { project, edit })) as FlowEditorEdited;
  return edited.selected;
}

/** Picking a Start from entry copies it into the draft. */
export function pickEntry(project: string, pick: FlowEditorPick, ask: Ask = request): Promise<unknown> {
  return ask("flow_editor.pick", { project, pick });
}

/** Undo on a tinted step, or Undo all (`stage` null). */
export function undo(project: string, stage: string | null, ask: Ask = request): Promise<unknown> {
  return ask("flow_editor.undo", stage === null ? { project } : { project, stage });
}

/** Import a flow…'s menu choice: `file` (the open dialog, cancelling does nothing) or `project:<KEY>`. */
export async function importChoice(
  project: string,
  id: string,
  pickFile: () => Promise<string | null>,
  ask: Ask = request,
): Promise<void> {
  if (id === "file") {
    const path = await pickFile();
    if (path === null) return;
    await ask("flow_editor.import", { project, path });
    return;
  }
  if (id.startsWith("project:")) {
    await pickEntry(project, { pick: "project", project: id.slice("project:".length) }, ask);
  }
}

/** Apply (Use this flow) and Cancel (Skip, use the default): both close the editor once the service answered. */
export function applyDraft(view: FlowEditorView, ask: Ask = request): Promise<unknown> {
  return ask("flow_editor.apply", { project: view.project, from: view.from, proposal: view.proposal });
}

export function cancelDraft(view: FlowEditorView, ask: Ask = request): Promise<unknown> {
  return ask("flow_editor.cancel", { project: view.project, from: view.from, proposal: view.proposal });
}

/** The words on their way, by project: a second Return while one is sending starts nothing. */
const sending = new Set<string>();

/**
 * The panel's first message (typed, or a chip's words): starts the Project agent on the `flow` skill, which the panel
 * then shows. Nothing is sent for empty words. Resolves to the agent's id, or `null` when nothing was sent.
 */
export async function sendFirst(
  project: string,
  text: string,
  ask: Ask = request,
  agents: Store<Readonly<Record<string, string>>> = editorAgents,
): Promise<string | null> {
  if (text.trim() === "" || sending.has(project)) return null;
  sending.add(project);
  try {
    const started = (await ask("flow_editor.send", { project, text })) as FlowEditorStarted;
    showEditorAgent(project, started.agent, agents);
    return started.agent;
  } finally {
    sending.delete(project);
  }
}

/** Ask <Tool> (Made for, the chooser, "Ask Claude Code"); the panel shows the agent that looks. */
export async function askTool(
  project: string,
  from: FlowEditorFrom,
  tool: string,
  ask: Ask = request,
  agents: Store<Readonly<Record<string, string>>> = editorAgents,
): Promise<string> {
  const started = (await ask("flow_editor.ask", { project, from, tool })) as FlowEditorStarted;
  showEditorAgent(project, started.agent, agents);
  return started.agent;
}

/** "Both, and compare": asks each tool in turn (each proposal becomes its own Made for entry). */
export async function askEach(
  project: string,
  from: FlowEditorFrom,
  tools: readonly string[],
  ask: Ask = request,
  agents: Store<Readonly<Record<string, string>>> = editorAgents,
): Promise<void> {
  for (const tool of tools) await askTool(project, from, tool, ask, agents);
}

/** What a chip does: send its words, open the chooser, or ask the one tool. `chooser` opens the tool picker. */
export function chipPressed(
  project: string,
  from: FlowEditorFrom,
  action: FlowEditorChipAction,
  chooser: () => void,
  ask: Ask = request,
  agents: Store<Readonly<Record<string, string>>> = editorAgents,
): Promise<unknown> {
  switch (action.kind) {
    case "send":
      return sendFirst(project, action.message, ask, agents);
    case "choose_tool":
      chooser();
      return Promise.resolve(null);
    case "ask":
      return askTool(project, from, action.tool, ask, agents);
  }
}

/**
 * Where a dragged step lands (`move`'s `to`: its index among the stages once it is taken out): how many of the other
 * rows' vertical middles are above the pointer. `middles` are the stage rows' middles in order, `from` the dragged one.
 */
export function dropIndex(middles: readonly number[], y: number, from: number): number {
  return middles.filter((middle, i) => i !== from && middle < y).length;
}

/** Alt+↑ / Alt+↓ on a focused step: its new index, or `null` at the edge (or when it can't move). */
export function keyMove(index: number, count: number, key: string): number | null {
  if (key === "ArrowUp") return index > 0 ? index - 1 : null;
  if (key === "ArrowDown") return index < count - 1 ? index + 1 : null;
  return null;
}

/** How long the walking ticket stays on each tile. */
export const TILE_MS = 1600;

/** The next tile the ticket walks to, wrapping. */
export function nextTile(at: number, count: number): number {
  return count === 0 ? 0 : (at + 1) % count;
}
