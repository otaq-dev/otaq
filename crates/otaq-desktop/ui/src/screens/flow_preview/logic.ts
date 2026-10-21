// The flow preview's moments of interaction (T-0420, FLW-04): the picks kept until Apply, the three answers (Apply,
// Cancel, Ask for more changes), and when the preview opens. What it says is Rust's (`src/api/flow_preview.rs`,
// `otaq-view`'s `flow_preview.rs`).
import { openOverlay } from "../../app/overlays";
import { createStore, type Store } from "../../app/store";
import type { FlowCompareProposal } from "../../generated/FlowCompareProposal";
import type { FlowCompareView } from "../../generated/FlowCompareView";
import type { FlowPreviewChoices } from "../../generated/FlowPreviewChoices";
import type { FlowPreviewClashPick } from "../../generated/FlowPreviewClashPick";
import type { FlowPreviewProposed } from "../../generated/FlowPreviewProposed";
import type { FlowPreviewSource } from "../../generated/FlowPreviewSource";
import type { FlowPreviewView } from "../../generated/FlowPreviewView";
import type { PageEvent } from "../../generated/PageEvent";
import { type PageEvents, pageEvents } from "../../service";
import { request } from "../../tauri";

export type Ask = typeof request;

export const NO_CHOICES: FlowPreviewChoices = { clashes: {}, skill_folders: {}, models: {} };

/** An import clash picked: Keep mine, Take theirs or Add as. */
export function withClash(
  choices: FlowPreviewChoices,
  agent: string,
  pick: FlowPreviewClashPick,
): FlowPreviewChoices {
  return { ...choices, clashes: { ...choices.clashes, [agent]: pick } };
}

/** A missing skill's folder picked (Import its folder…). */
export function withFolder(choices: FlowPreviewChoices, skill: string, folder: string): FlowPreviewChoices {
  return { ...choices, skill_folders: { ...choices.skill_folders, [skill]: folder } };
}

/**
 * A local model picked for an agent: the default model (`default`), a tool (`claude`, its default model) or one of
 * its models (`claude:opus`).
 */
export function withModel(
  choices: FlowPreviewChoices,
  agent: string,
  tool: string,
  model: string | null = null,
): FlowPreviewChoices {
  const value = tool === "default" || model === null ? tool : `${tool}:${model}`;
  return { ...choices, models: { ...choices.models, [agent]: value } };
}

/** The tool part of a picked model (`claude:opus` → `claude`). */
export function pickedTool(choices: FlowPreviewChoices, agent: string): string | null {
  const value = choices.models[agent];
  return value === undefined ? null : (value.split(":")[0] ?? null);
}

/** Apply, with the picks as the view gives them back (every clash with its pick). */
export function applyPreview(view: FlowPreviewView, ask: Ask = request): Promise<unknown> {
  return ask("flow_preview.apply", { project: view.project, proposal: view.proposal, choices: view.choices });
}

/** Use this flow on one of the proposals compared (ASK-07): `flow.apply` with its number, which closes the others. */
export function applyProposal(proposal: FlowCompareProposal, ask: Ask = request): Promise<unknown> {
  return applyPreview(proposal.view, ask);
}

/**
 * The proposal each flow agent showed first (`flow_preview.compare`'s `first`), with those of `view` not yet in it: a
 * later proposal from the same agent is its answer ("updated after your answer"). The same map when nothing is new.
 */
export function rememberFirsts(first: Record<string, number>, view: FlowCompareView): Record<string, number> {
  const added = view.proposals.filter((p) => p.agent !== null && !(p.agent in first));
  if (added.length === 0) return first;
  const next = { ...first };
  for (const p of added) if (p.agent !== null) next[p.agent] = p.view.proposal;
  return next;
}

/** Cancel (and Close on "No changes"): the proposal goes; a Project agent gets Cancel as its answer. */
export function cancelPreview(view: FlowPreviewView, ask: Ask = request): Promise<unknown> {
  return ask("flow_preview.cancel", { project: view.project });
}

/** Ask for more changes with the user's line; nothing is sent while it is empty. */
export function askForChanges(
  view: FlowPreviewView,
  text: string,
  ask: Ask = request,
): Promise<unknown> | null {
  const line = text.trim();
  if (line === "") return null;
  return ask("flow_preview.ask", { project: view.project, proposal: view.proposal, text: line });
}

/** The project whose proposal opened, when `event` is a `flow_changed` with a new proposal. */
export function proposedIn(event: PageEvent): string | null {
  if (event.kind !== "service" || event.name !== "flow_changed") return null;
  const change = event.event.change as { what?: string } | undefined;
  const project = event.event.project;
  return change?.what === "proposed" && typeof project === "string" ? project : null;
}

/**
 * The project whose proposals onboarding's Flow step shows itself (ONB-06): the usual preview doesn't open over it.
 * `null` while no Flow step shows.
 */
export const previewHeld: Store<string | null> = createStore<string | null>(null);

/**
 * Opens the preview whenever a proposal arrives, for any project (it names its project), and on the newest open one
 * when the stream opens or resyncs: a proposal made while the app was closed or away isn't missed (ONB-07). The
 * project onboarding's Flow step holds is left to it. Returns the stop.
 */
export function followProposals(
  events: PageEvents = pageEvents,
  open: (project: string) => void = (project) => {
    openOverlay("flow_preview", { project });
  },
  ask: Ask = request,
  held: Store<string | null> = previewHeld,
): () => void {
  let stopped = false;
  const openUnheld = (project: string) => {
    if (project !== held.get()) open(project);
  };
  const openNewest = () => {
    ask("flow_preview.open").then(
      (projects) => {
        const newest = (projects as string[])[0];
        if (!stopped && newest !== undefined) openUnheld(newest);
      },
      () => undefined,
    );
  };
  const listening = events.listen((event) => {
    if (event.kind === "resync") {
      openNewest();
      return;
    }
    const project = proposedIn(event);
    if (project !== null) openUnheld(project);
  });
  listening.ready.then(openNewest, () => undefined);
  return () => {
    stopped = true;
    listening.stop();
  };
}

/** The preview of `project`'s open proposal (Settings → Flow, T-0422): the keyboard moves into it. */
export function openFlowPreview(project: string): void {
  openOverlay("flow_preview", { project, focus: true });
}

/**
 * Settings → Flow's actions (T-0422): a proposal from a preset, another project, a file, the previous flow or the
 * shipped agents file, then its preview. Rejects with why the service refused it.
 */
export async function proposeFlow(
  project: string,
  source: FlowPreviewSource,
  ask: Ask = request,
): Promise<void> {
  const proposed = (await ask("flow_preview.propose", { project, source })) as FlowPreviewProposed;
  openFlowPreview(proposed.project);
}
