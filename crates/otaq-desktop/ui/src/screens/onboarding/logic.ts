// Onboarding's moment of interaction, without the DOM: which step shows, what the Add project form holds, and what
// ending onboarding asks of the app. Every word and rule is Rust's (`onboarding_page.rs`, `onboarding.rs`,
// `add_project.rs`); this file only moves between them. The Flow step's moments (ASK-04…ASK-06) are at the end.
import { selectProject } from "../../app/selection";
import { bindingFor, type KeyLike } from "../../app/keys";
import type { AddProjectParams } from "../../generated/AddProjectParams";
import type { AddProjectView } from "../../generated/AddProjectView";
import type { FlowAtWorkAction } from "../../generated/FlowAtWorkAction";
import type { FlowAtWorkView } from "../../generated/FlowAtWorkView";
import type { FlowCompareView } from "../../generated/FlowCompareView";
import type { FlowPreviewProposed } from "../../generated/FlowPreviewProposed";
import type { Keymap } from "../../generated/Keymap";
import type { OnboardingAskParams } from "../../generated/OnboardingAskParams";
import type { OnboardingAsked } from "../../generated/OnboardingAsked";
import type { OnboardingFlowStep } from "../../generated/OnboardingFlowStep";
import type { OnboardingProjectParams } from "../../generated/OnboardingProjectParams";
import type { OnboardingStepId } from "../../generated/OnboardingStepId";
import type { OnboardingTools } from "../../generated/OnboardingTools";
import type { PaneEndParams } from "../../generated/PaneEndParams";
import type { PaneResumeParams } from "../../generated/PaneResumeParams";
import { PANE_ROUTES, type PaneRoutes, showAgent, showPane } from "../pane/logic";

/** The door's request call, or a fake one in the tests. */
export type Ask = (method: string, params?: unknown) => Promise<unknown>;

/** The steps in order; Done is the empty board itself (*6 · Done · empty board*, ONB-10). */
export const STEP_ORDER: readonly OnboardingStepId[] = ["welcome", "theme", "tools", "project", "flow"];

/**
 * Where Continue on the tools step goes: back to the Flow step when first run's project is still in Flow setup (Back
 * to Tools), else the project step, or the end for a user who has projects already.
 */
export function afterTools(
  tools: OnboardingTools,
  flowProject: string | null = null,
): "project" | "flow" | "done" {
  if (flowProject !== null) return "flow";
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

/** The base branch as its dropdown shows it: a remote branch by its short name (`origin/next` → `next`). */
export function baseLabel(form: AddProjectParams, view: AddProjectView | undefined): string {
  const id = baseText(form, view);
  return view?.base?.items.find((i) => !i.header && i.id === id)?.label ?? id;
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
 * Where the Flow step is (ASK-04…ASK-06): the go-ahead (Ask an agent); the presets (Use a preset instead, Stop all,
 * or no tool connected); the agents at work (the tabs and the card, `FlowAtWork.tsx`); a proposal in the step (`first`
 * the proposal shown first, so a later one reads "Updated after your answer").
 */
export type FlowStepState =
  { at: "go_ahead" } | { at: "presets" } | { at: "at_work" } | { at: "proposal"; first: number };

/**
 * Where the step is with its Flow setup as Rust says it now: the go-ahead stands for "where it opens", so it shows the
 * agents at work once a tool was asked and the presets with no tool connected. Elsewhere the user's place stays.
 */
export function flowStepChanged(state: FlowStepState, step: OnboardingFlowStep): FlowStepState {
  if (state.at !== "go_ahead") return state;
  switch (step.opens) {
    case "go_ahead":
      return state;
    case "at_work":
      return { at: "at_work" };
    case "no_tool":
      return { at: "presets" };
  }
}

/** Where the step opens, as Rust says: nothing asked, a tool asked, or no tool connected. */
export function flowStepOpens(step: OnboardingFlowStep): FlowStepState {
  return flowStepChanged({ at: "go_ahead" }, step);
}

/** A proposal shows (an agent's, or a preset picked): the proposal, keeping the first one seen. */
export function proposalShown(state: FlowStepState, proposal: number): FlowStepState {
  if (state.at === "proposal") return state;
  return { at: "proposal", first: proposal };
}

/** The proposal went without the step's answer (cancelled elsewhere): back where the step opens. */
export function proposalGone(state: FlowStepState): FlowStepState {
  return state.at === "proposal" ? { at: "go_ahead" } : state;
}

/** Ask <Tool> (ASK-02): starts the tool's flow agent for the project, or answers the running one. */
export async function askTool(project: string, tool: string, ask: Ask): Promise<OnboardingAsked> {
  const params: OnboardingAskParams = { project, tool };
  return (await ask("onboarding.ask", params)) as OnboardingAsked;
}

/**
 * A row's button (Sign in… is the dialog's, not a request): Ask <Tool>; Stop (the agent ends into Recently ended,
 * asked nothing mid-turn: the click is the answer); Try again at the limit, its tool's Resume ("continue" into the
 * same session); Try again after it ended, Resume of its conversation (d0088).
 */
export async function actOnRow(
  project: string,
  action: Exclude<FlowAtWorkAction, { kind: "sign_in" }>,
  ask: Ask,
): Promise<void> {
  switch (action.kind) {
    case "ask":
      await askTool(project, action.tool, ask);
      return;
    case "stop": {
      const params: PaneEndParams = { project, id: action.agent, confirm: true };
      await ask("pane.end", params);
      return;
    }
    case "resume_tool":
      await ask("limits.resume_tool", { tool: action.tool });
      return;
    case "resume": {
      const params: PaneResumeParams = { id: action.agent };
      await ask("pane.resume", params);
      return;
    }
  }
}

/** Stop all and use a preset: ends every flow agent at work or stopped (Rust's list), then the presets. */
export async function stopAll(project: string, view: FlowAtWorkView, ask: Ask): Promise<FlowStepState> {
  for (const agent of view.running) await actOnRow(project, { kind: "stop", agent }, ask);
  return { at: "presets" };
}

/** Skip, use the default and Use the default (ASK-03): the default flow; every proposal closes, every agent ends. */
export async function chooseDefault(project: string, ask: Ask): Promise<void> {
  const params: OnboardingProjectParams = { project };
  await ask("onboarding.use_default", params);
}

/**
 * The proposal each flow agent showed first, with the ones in `view` added: `firsts` itself when nothing is new (so a
 * watch keyed on it doesn't restart).
 */
export function firstProposals(
  firsts: Record<string, number>,
  view: FlowCompareView,
): Record<string, number> {
  const fresh = view.proposals.filter((p) => p.agent !== null && !(p.agent in firsts));
  if (fresh.length === 0) return firsts;
  const next = { ...firsts };
  for (const p of fresh) if (p.agent !== null) next[p.agent] = p.view.proposal;
  return next;
}

/** The tab whose terminal shows: the one picked while it's there, else the first. */
export function watchedTab(view: FlowAtWorkView | undefined, picked: string | null): string | null {
  const tabs = view?.tabs ?? [];
  return tabs.find((t) => t.agent === picked)?.agent ?? tabs[0]?.agent ?? null;
}

/** The flow agent behind a proposal, which stays open in the pane once its flow is used; none for a preset. */
export function proposingAgent(view: FlowAtWorkView, proposal: number): string | null {
  return view.rows.find((r) => r.proposal === proposal)?.agent ?? null;
}

/** How often the agents at work ask again while they show, so "Working · 2 min" moves on (the words stay Rust's). */
export const AT_WORK_TICK_MS = 60_000;

/**
 * ⌘L inside the step (the onboarding overlay holds the app's shortcuts back): whether the key is the user's Focus the
 * pane, which here focuses the shown flow agent's terminal.
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
 * The Flow step is over (Use this flow, Use the default, Skip): the project's board shows, with the chosen proposal's
 * flow agent open in its pane.
 */
export function afterFlowStep(project: string, agent: string | null, routes: PaneRoutes = PANE_ROUTES): void {
  selectProject(project, routes.selection);
  if (agent !== null) {
    showAgent(project, agent, routes.shown);
    showPane(routes);
  }
}
