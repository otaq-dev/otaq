// A row's Install / Sign in… (the Tools checklist and the agent tools, onboarding and Settings): a short terminal
// that opens under the row running the command, so its prompts (a password, a device code) are answered right
// there. One terminal at a time; Hide leaves the command running, and the row's button shows it again. When it ends,
// the tools are checked again: done closes the terminal, not done keeps the output with Retry.
import { useCallback, useEffect, useRef, useState } from "react";

import { Button, IconButton } from "../../components";
import type { TerminalPhase } from "../../generated/TerminalPhase";
import type { ToolTerminalTexts } from "../../generated/ToolTerminalTexts";
import { asRequestError } from "../../service";
import { Terminal } from "../../terminal/Terminal";
import { type RowTerminalOpen, type RowTerminalOpened, settle } from "./rowTerminalStages";
import "./row-terminal.css";

/** How long "Done" shows before the terminal closes. */
const DONE_SHOWN_MS = 1200;

export interface RowTerminals {
  open: RowTerminalOpen | null;
  /** Why the last button press didn't open a terminal, and under which row. */
  failure: { row: string; message: string } | null;
  /** Opens `key`'s terminal under `row` with `opener` (the request behind the button). */
  start: (row: string, key: string, opener: () => Promise<RowTerminalOpened>) => void;
  retry: () => void;
  hide: () => void;
  onPhase: (phase: TerminalPhase) => void;
}

/**
 * The rows' terminal. `stillOffered(key)` says whether the rows (as they are now) still offer that action;
 * `recheck` checks the tools again and settles once the new rows are in.
 */
export function useRowTerminals(
  stillOffered: (key: string) => boolean,
  recheck: () => Promise<void>,
): RowTerminals {
  const [open, setOpen] = useState<RowTerminalOpen | null>(null);
  const [failure, setFailure] = useState<RowTerminals["failure"]>(null);
  const [checked, setChecked] = useState(false);
  const openers = useRef(new Map<string, () => Promise<RowTerminalOpened>>());

  const start = useCallback((row: string, key: string, opener: () => Promise<RowTerminalOpened>) => {
    openers.current.set(key, opener);
    setFailure(null);
    setChecked(false);
    opener().then(
      (opened) => {
        setOpen({ ...opened, row, key, stage: "running" });
      },
      (e: unknown) => {
        setFailure({ row, message: asRequestError(e).message });
      },
    );
  }, []);

  const hide = useCallback(() => {
    setOpen(null);
  }, []);

  // The terminal tells every change (focus, scrolling); only its first "ended" checks again.
  const running = useRef<string | null>(null);
  useEffect(() => {
    running.current = open?.stage === "running" ? open.session : null;
  }, [open]);
  const onPhase = useCallback(
    (phase: TerminalPhase) => {
      if (phase !== "ended" || running.current === null) return;
      running.current = null;
      setOpen((o) => (o?.stage === "running" ? { ...o, stage: "checking" } : o));
      void recheck().finally(() => {
        setChecked(true);
      });
    },
    [recheck],
  );

  // Once the check after the command is in, the rows say whether it worked.
  const shown = open?.stage === "checking" && checked ? settle(open, stillOffered(open.key)) : open;

  useEffect(() => {
    if (shown?.stage !== "done") return;
    const timer = setTimeout(() => {
      setOpen(null);
    }, DONE_SHOWN_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [shown?.stage]);

  const retry = useCallback(() => {
    if (!open) return;
    const opener = openers.current.get(open.key);
    if (opener) start(open.row, open.key, opener);
  }, [open, start]);

  return { open: shown, failure, start, retry, hide, onPhase };
}

/** The terminal and the failure line under a row, when they are this row's. */
export function RowTerminal({
  row,
  texts,
  terminals,
}: {
  row: string;
  texts: ToolTerminalTexts;
  terminals: RowTerminals;
}) {
  const { open, failure } = terminals;
  if (failure?.row === row) return <p className="row-terminal-failure">{failure.message}</p>;
  if (open?.row !== row) return null;
  const title = {
    running: open.title,
    checking: texts.checking,
    not_done: texts.not_done,
    done: texts.done,
  }[open.stage];
  const running = open.stage === "running";
  return (
    <div className="row-terminal" data-stage={open.stage}>
      <div className="row-terminal-head">
        <span className="row-terminal-dot" data-live={running || undefined} />
        <span className="row-terminal-title">{title}</span>
        {open.stage === "not_done" && (
          <Button
            compact
            variant="secondary"
            icon="rotate-cw"
            label={texts.retry}
            onClick={terminals.retry}
          />
        )}
        <IconButton
          icon={running ? "chevron-up" : "x"}
          label={running ? texts.hide : texts.close}
          onClick={terminals.hide}
        />
      </div>
      <div className="row-terminal-body">
        <Terminal
          key={open.session}
          session={open.session}
          label={open.title}
          onState={(state) => {
            terminals.onPhase(state.phase);
          }}
        />
      </div>
    </div>
  );
}
