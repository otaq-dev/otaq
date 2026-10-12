// The PR dialog's interaction state (UIT-04): the fields as the user types them, when to ask the flow again, and when
// the branch name's check is due. Every text and rule is Rust's (`pr_page.rs`, `pr_dialog.rs`, `src/api/pr.rs`).
import type { PrAnswer } from "../../generated/PrAnswer";
import type { PrDialogView } from "../../generated/PrDialogView";

/** While the skill prepares or pushes, the flow is asked again this often (the GPUI app's second). */
export const POLL_MS = 1000;

/** The fields as the user edits them; filled once from the proposal. */
export interface Fields {
  branch: string;
  commits: string[];
  title: string;
  description: string;
}

export function fieldsOf(view: PrDialogView): Fields {
  return {
    branch: view.branch,
    commits: view.commits.map((c) => c.message),
    title: view.title,
    description: view.description,
  };
}

/** The proposal just arrived: the fields take its values (later edits are the user's). */
export function proposalArrived(before: PrDialogView | null, now: PrDialogView): boolean {
  return now.stage.kind === "proposed" && before?.stage.kind !== "proposed";
}

/** Whether the skill is working, so the flow is asked again every `POLL_MS`. */
export function polls(view: PrDialogView): boolean {
  return view.stage.kind === "preparing" || view.stage.kind === "pushing";
}

/** Whether the branch name's check for edit `view.check` is still to be asked (it waits `check_after_ms`). */
export function checkDue(view: PrDialogView, asked: number): boolean {
  return view.stage.kind === "proposed" && !view.branch_locked && view.check > asked;
}

/** An answer older than one already shown (an edit's reply overtaken by a newer one) is dropped. */
export function isStale(answer: PrAnswer, newestCheck: number): boolean {
  return answer.outcome === "dialog" && answer.view.check < newestCheck;
}

/** What the confirm button does on each stage. */
export type Press = "start" | "confirm" | "retry";
