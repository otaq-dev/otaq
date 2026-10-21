// A row's Install / Sign in… terminal, without the DOM: where it stands. The service picks and runs the command;
// when it ends, the tools are checked again, and the rows decide: the same button still offered means not done
// yet, otherwise done. Every word is Rust's.

/** Running its command; ended and checking again; checked and still to do; checked and done. */
export type RowTerminalStage = "running" | "checking" | "not_done" | "done";

/** What opening a row's terminal answers: the session to attach to, and its title ("Installing gh…"). */
export interface RowTerminalOpened {
  session: string;
  title: string;
}

export interface RowTerminalOpen extends RowTerminalOpened {
  /** The row it shows under ("gh", "codex"). */
  row: string;
  /** The row's action ("gh:install", "codex:sign_in"): what has to stop being offered for "done". */
  key: string;
  stage: RowTerminalStage;
}

/** The stage once the check after the command is in. */
export function settle(open: RowTerminalOpen, stillOffered: boolean): RowTerminalOpen {
  return { ...open, stage: stillOffered ? "not_done" : "done" };
}
