// Onboarding's moment of interaction, without the DOM: which step shows, what the Add project form holds, and what
// ending onboarding asks of the app. Every word and rule is Rust's (`onboarding_page.rs`, `onboarding.rs`,
// `add_project.rs`); this file only moves between them. The Flow step's moments (ONB-04…ONB-06) are at the end.
import { selectProject } from "../../app/selection";
import { bindingFor, type KeyLike } from "../../app/keys";
import type { AddProjectParams } from "../../generated/AddProjectParams";
import type { AddProjectView } from "../../generated/AddProjectView";
import type { FlowAtWorkAction } from "../../generated/FlowAtWorkAction";
import type { FlowAtWorkParams } from "../../generated/FlowAtWorkParams";
import type { FlowAtWorkView } from "../../generated/FlowAtWorkView";
import type { FlowPreviewProposed } from "../../generated/FlowPreviewProposed";
import type { Keymap } from "../../generated/Keymap";
import type { OnboardingFlowStep } from "../../generated/OnboardingFlowStep";
import type { OnboardingStepId } from "../../generated/OnboardingStepId";
import type { OnboardingTools } from "../../generated/OnboardingTools";
import type { PaneEndParams } from "../../generated/PaneEndParams";
import type { PaneFlowStarted } from "../../generated/PaneFlowStarted";
import type { PaneProjectParams } from "../../generated/PaneProjectParams";
import type { PaneResumeParams } from "../../generated/PaneResumeParams";
import { PANE_ROUTES, type PaneRoutes, showAgent, showPane } from "../pane/logic";

/** The door's request call, or a fake one in the tests. */
export type Ask = (method: string, params?: unknown) => Promise<unknown>;

/** The steps in order; Done is the empty board itself (*6 · Done · empty board*, ONB-10). */
export const STEP_ORDER: readonly OnboardingStepId[] = ["welcome", "theme", "tools", "project", "flow"];

/** Where Continue on the tools step goes: the project step, or the end for a user who has projects already. */
export function afterTools(tools: OnboardingTools): "project" | "done" {
  return tools.has_projects ? "done" : "project";
}

/** A fresh form, with a folder already picked or dropped if given. */
export function newForm(firstRun: boolean, folder: string | null = null): AddProjectParams {
  return { folder, key: "", key_edited: false, base_branch: "", base_edited: false, first_run: firstRun };
}

/** The key field as typed; Rust normalizes it (uppercase, trimmed) and the next answer shows that. */
export function typeKey(form: AddProjectParams, key: string): AddProjectParams {
  return { ...form, key, key_edited: true };
}

export function typeBase(form: AddProjectParams, base: string): AddProjectParams {
  return { ...form, base_branch: base, base_edited: true };
}

export function pickFolder(form: AddProjectParams, folder: string): AddProjectParams {
  return { ...form, folder };
}

/** The key field's text: the user's once they typed, else Rust's suggestion. */
export function keyText(form: AddProjectParams, view: AddProjectView | undefined): string {
  return form.key_edited ? form.key : (view?.key?.value ?? "");
}

/**
 * After Rust answered `asked`: the form with the key as Rust spells it (uppercase, trimmed), if the user hasn't typed
 * on meanwhile; otherwise `form` unchanged.
 */
export function adoptKey(
  form: AddProjectParams,
  asked: AddProjectParams,
  view: AddProjectView,
): AddProjectParams {
  const spelled = view.key?.value;
  if (form !== asked || !form.key_edited || spelled === undefined || spelled === form.key) return form;
  return { ...form, key: spelled };
}

export function baseText(form: AddProjectParams, view: AddProjectView | undefined): string {
  return form.base_edited ? form.base_branch : (view?.base?.value ?? "");
}

/**
 * Ending onboarding: the permission-mode banner when the warning was shown and not closed (T-0231's
 * `notices.permission_warning`). Returns whether it was raised.
 */
export async function finishOnboarding(warningShown: boolean, ask: Ask): Promise<boolean> {
  if (!warningShown) return false;
  await ask("notices.permission_warning");
  return true;
}

/**
 * Where the Flow step is (ONB-04…ONB-06): the go-ahead; the presets (Use a preset instead, or nothing to look at);
 * the onboarding agent at work (`agent`, `FlowAtWork`'s card: working, stopped, ended); the agent that couldn't start
 * (its tool isn't signed in: Rust's `card`); the proposal in the step (`agent` when one proposed it, `first` the
 * proposal shown first, so a later one reads "Updated after your answer").
 */
export type FlowStepState =
  | { at: "go_ahead" }
  | { at: "presets"; agent: string | null }
  | { at: "working"; agent: string }
  | { at: "cannot_start"; card: FlowAtWorkView }
  | { at: "proposal"; agent: string | null; first: number };

/** Where the step opens: the presets when there is nothing to look at, else the go-ahead. */
export function flowStepOpens(step: OnboardingFlowStep): FlowStepState {
  return step.opens_on_presets ? { at: "presets", agent: null } : { at: "go_ahead" };
}

/** The onboarding agent behind the state, if one started. */
export function flowAgent(state: FlowStepState): string | null {
  return state.at === "go_ahead" || state.at === "cannot_start" ? null : state.agent;
}

/** A proposal shows (the agent's, or a preset picked): the proposal, keeping the first one seen. */
export function proposalShown(state: FlowStepState, proposal: number): FlowStepState {
  if (state.at === "proposal") return state;
  return { at: "proposal", agent: flowAgent(state), first: proposal };
}

/**
 * The proposal went without the step's answer (cancelled or applied elsewhere): back to the agent at work when one
 * proposed it, else the presets.
 */
export function proposalGone(state: FlowStepState): FlowStepState {
  if (state.at !== "proposal") return state;
  return state.agent !== null ? { at: "working", agent: state.agent } : { at: "presets", agent: null };
}

/** How Look and propose a flow went: the agent started, or its tool isn't signed in (the card says so). */
export type FlowStart = { kind: "started"; agent: string } | { kind: "cannot_start"; card: FlowAtWorkView };

/**
 * Look and propose a flow (ONB-05): starts the project's onboarding agent (`pane.start_flow`: its first Project agent
 * on the `flow` skill, or the one already running) unless its tool is Found · not signed in (`onboarding.at_work`
 * without an agent, d0086). Every start goes through here, Sign in…'s afterwards too; the click is the go-ahead
 * (d0087).
 */
export async function startFlowAgent(step: OnboardingFlowStep, ask: Ask): Promise<FlowStart> {
  const check: FlowAtWorkParams = { project: step.project };
  const card = (await ask("onboarding.at_work", check)) as FlowAtWorkView | null;
  if (card !== null) return { kind: "cannot_start", card };
  const params: PaneProjectParams = { project: step.project };
  const started = (await ask("pane.start_flow", params)) as PaneFlowStarted;
  return { kind: "started", agent: started.id };
}

/** Where a start leaves the step: the agent at work, or the card of a tool that isn't signed in. */
export function flowStarted(start: FlowStart): FlowStepState {
  return start.kind === "started"
    ? { at: "working", agent: start.agent }
    : { at: "cannot_start", card: start.card };
}

/**
 * Stop and use a preset: ends the onboarding agent (asked nothing mid-turn: the click is the answer); it goes to
 * Recently ended like any ended agent, and the step shows the presets with no agent left to open on the board.
 */
export async function stopFlowAgent(project: string, agent: string, ask: Ask): Promise<FlowStepState> {
  const params: PaneEndParams = { project, id: agent, confirm: true };
  await ask("pane.end", params);
  return { at: "presets", agent: null };
}

/**
 * Try again on a failure's card (Sign in… is the dialog's, not a request): its tool's Resume after the plan limit,
 * which types "continue" into the same session; Resume of the ended agent's conversation (d0088).
 */
export async function retryFlowAgent(
  action: Exclude<FlowAtWorkAction, { kind: "sign_in" }>,
  ask: Ask,
): Promise<void> {
  if (action.kind === "resume_tool") {
    await ask("limits.resume_tool", { tool: action.tool });
    return;
  }
  const params: PaneResumeParams = { id: action.agent };
  await ask("pane.resume", params);
}

/** How often the card asks again while it shows, so "Working · 2 min" moves on (the words stay Rust's). */
export const AT_WORK_TICK_MS = 60_000;

/**
 * ⌘L inside the step (the onboarding overlay holds the app's shortcuts back): whether the key is the user's Focus the
 * pane, which here focuses the onboarding agent's terminal.
 */
export function focusesAgent(map: Keymap | undefined, e: KeyLike, inTerminal: boolean): boolean {
  return map !== undefined && bindingFor(map, e, inTerminal)?.action.action === "focus_pane";
}

/** A preset picked in the step: its proposal (`Source::Preset`), whose preview then shows in the step. */
export async function proposePreset(step: OnboardingFlowStep, file: string, ask: Ask): Promise<number> {
  const proposed = (await ask("flow_preview.propose", {
    project: step.project,
    source: { from: "preset", file },
  })) as FlowPreviewProposed;
  return proposed.proposal;
}

/**
 * The Flow step is over (Use this flow, Use the default, Skip): the project's board shows, with the onboarding agent
 * open in its pane (ONB-06).
 */
export function afterFlowStep(project: string, agent: string | null, routes: PaneRoutes = PANE_ROUTES): void {
  selectProject(project, routes.selection);
  if (agent !== null) {
    showAgent(project, agent, routes.shown);
    showPane(routes);
  }
}
