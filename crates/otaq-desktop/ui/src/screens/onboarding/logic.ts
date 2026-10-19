// Onboarding's moment of interaction, without the DOM: which step shows, what the Add project form holds, and what
// ending onboarding asks of the app. Every word and rule is Rust's (`onboarding_page.rs`, `onboarding.rs`,
// `add_project.rs`); this file only moves between them.
import type { AddProjectParams } from "../../generated/AddProjectParams";
import type { AddProjectView } from "../../generated/AddProjectView";
import type { OnboardingStepId } from "../../generated/OnboardingStepId";
import type { OnboardingTools } from "../../generated/OnboardingTools";

/** The door's request call, or a fake one in the tests. */
export type Ask = (method: string, params?: unknown) => Promise<unknown>;

/** The steps in order; Done is the empty board itself (*4 · Done · empty board*). */
export const STEP_ORDER: readonly OnboardingStepId[] = ["welcome", "theme", "tools", "project"];

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
