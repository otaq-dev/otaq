// The Diff tab (REV-01, REV-02, REV-03; *07 · Diff, review, approve & send back*): the agent's summary (foldable),
// the base indicator with the pending comments, the viewed count and Unified / Side-by-side, the file list with its
// letters, counts and viewed ticks (grouped with a filter from 20 files), and the diff, each file fetched as it comes
// into view with its lines highlighted by Rust (`diff_page.rs`). Line comments open from the gutter; threads sit under
// their line. Above the summary, how the ticket reached Review ("Validated · round 2"); under it, the final validation
// report, or the last findings of a ticket that didn't pass (VAL-12). Between them, the recorded decisions the review
// keeps or changes, grouped by child on an epic (DEC-04, `review.decisions`). What it says comes from `diff.view`,
// `diff.file`, `diff.comments`, `diff.review` and `review.decisions`.
import {
  type Dispatch,
  Fragment,
  type ReactNode,
  type RefObject,
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";

import { openOverlay } from "../../app/overlays";
import type { TicketProps } from "../../app/registry";
import { Button, DecisionsList, Kbd, Segmented } from "../../components";
import type { CodeSpan } from "../../generated/CodeSpan";
import type { DecisionItemView } from "../../generated/DecisionItemView";
import type { DecisionsSection } from "../../generated/DecisionsSection";
import type { DiffAnchor } from "../../generated/DiffAnchor";
import type { DiffComments } from "../../generated/DiffComments";
import type { DiffFileRow } from "../../generated/DiffFileRow";
import type { DiffFileView } from "../../generated/DiffFileView";
import type { DiffHunkView } from "../../generated/DiffHunkView";
import type { DiffLineView } from "../../generated/DiffLineView";
import type { DiffLoad } from "../../generated/DiffLoad";
import type { DiffReviewView } from "../../generated/DiffReviewView";
import type { DiffTabView } from "../../generated/DiffTabView";
import type { DiffThread } from "../../generated/DiffThread";
import type { NewToast } from "../../generated/NewToast";
import { asRequestError, useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { Report } from "../sections/Report";
import {
  type Composer,
  type ComposerAction,
  canSave,
  chunks,
  composerKey,
  composerReducer,
  FILE_LIST,
  fileListWidth,
  filteredGroups,
  groupFolded,
  LINE_HEIGHT,
  pathParts,
  rowAnchors,
  saveRequest,
  showsLines,
  showsSideBySide,
  threadsByAnchor,
  toggled,
} from "./logic";

/** The service events that change the file list (a new round, a merge) and the comments (any ticket change). */
export const DIFF_EVENTS = ["card_status_changed"] as const;

/** The events that change the Review entry or the validation report. */
export const REVIEW_EVENTS = ["card_status_changed", "ticket_changed"] as const;
export const COMMENT_EVENTS = ["ticket_changed"] as const;

/**
 * Change… on a recorded decision in the review (DEC-06): the "Change this decision" dialog (Send back) on a ticket,
 * "Add a fix child" in an epic's review; Rust picks which (`review.change_view`).
 */
export function changeDecision(item: DecisionItemView) {
  openOverlay("change_decision", { id: item.id });
}

const ignore = () => undefined;

function toastError(title: string, error: unknown) {
  const toast: NewToast = {
    level: "error",
    title,
    text: asRequestError(error).message,
    link: null,
    ticket: null,
  };
  request("notices.toast", toast).catch(ignore);
}

/** The element's width, followed as it resizes (Side-by-side and the file list depend on the panel's). */
function useWidth<T extends HTMLElement>(): [RefObject<T | null>, number] {
  const ref = useRef<T>(null);
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    setWidth(el.clientWidth);
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => {
      setWidth(el.clientWidth);
    });
    observer.observe(el);
    return () => {
      observer.disconnect();
    };
  }, []);
  return [ref, width];
}

type FileLoad =
  { state: "loading" } | { state: "loaded"; view: DiffFileView } | { state: "failed"; message: string };

/** What the comment parts need, passed down unchanged. */
interface CommentContext {
  ticket: string;
  comments: DiffComments | undefined;
  threads: Map<string, DiffThread>;
  composer: Composer | null;
  dispatch: Dispatch<ComposerAction>;
  save: (text: string) => void;
}

function Code({ spans }: { spans: readonly CodeSpan[] }) {
  return (
    <span className="diff-code">
      {spans.map((s, i) =>
        s.token === "plain" ? (
          s.text
        ) : (
          <span key={i} className="tok" data-token={s.token}>
            {s.text}
          </span>
        ),
      )}
    </span>
  );
}

/** The line numbers; with comments allowed, a button that opens a comment box on the line. */
function Gutter({
  numbers,
  anchor,
  ctx,
}: {
  numbers: ReactNode;
  anchor: DiffAnchor | null;
  ctx: CommentContext;
}) {
  if (anchor === null || !ctx.comments?.can_comment) return <span className="diff-gutter">{numbers}</span>;
  const thread = ctx.threads.get(anchor.key);
  return (
    <button
      type="button"
      className="diff-gutter"
      data-commentable
      aria-label={ctx.comments.texts.add_on_line}
      title={ctx.comments.texts.add_on_line}
      onClick={() => {
        ctx.dispatch({ kind: "open", anchor, replyTo: thread?.comments[0]?.id ?? null });
      }}
    >
      {numbers}
      <span className="diff-plus" aria-hidden>
        +
      </span>
    </button>
  );
}

function Num({ n }: { n: number | null }) {
  return <span className="diff-num">{n ?? ""}</span>;
}

function ComposerBox({ ctx, composer }: { ctx: CommentContext; composer: Composer }) {
  const texts = ctx.comments?.texts;
  const [text, setText] = useState(composer.initial);
  const input = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    input.current?.focus();
  }, []);
  if (!texts) return null;
  return (
    <div className="diff-composer">
      <textarea
        ref={input}
        className="text-input text-area diff-composer-input"
        rows={3}
        value={text}
        placeholder={texts.placeholder}
        aria-label={texts.placeholder}
        onChange={(e) => {
          setText(e.target.value);
        }}
        onKeyDown={(e) => {
          const what = composerKey(e);
          if (what === null) return;
          e.preventDefault();
          e.stopPropagation();
          if (what === "save") ctx.save(text);
          else ctx.dispatch({ kind: "cancel" });
        }}
      />
      {composer.error !== null && <p className="diff-composer-error">{composer.error}</p>}
      <div className="diff-composer-bar">
        <span className="diff-composer-hint">{texts.hint}</span>
        <Button
          compact
          variant="ghost"
          label={texts.cancel}
          onClick={() => {
            ctx.dispatch({ kind: "cancel" });
          }}
        />
        <Button
          compact
          label={composer.editing === null ? texts.add : texts.save}
          loading={composer.saving}
          disabled={!canSave(composer, text)}
          onClick={() => {
            ctx.save(text);
          }}
        />
        <Kbd keys={ctx.comments?.save_keys ?? ""} />
      </div>
    </div>
  );
}

/** The comments on one line, and the comment box when it is open there. */
function ThreadBlock({ anchor, ctx }: { anchor: DiffAnchor; ctx: CommentContext }) {
  const thread = ctx.threads.get(anchor.key);
  const composing = ctx.composer?.anchor.key === anchor.key ? ctx.composer : null;
  if (!thread && !composing) return null;
  const texts = ctx.comments?.texts;
  return (
    <div className="diff-thread">
      {thread?.comments.map((c) =>
        composing?.editing === c.id ? (
          <ComposerBox key={c.id} ctx={ctx} composer={composing} />
        ) : (
          <div key={c.id} className="diff-comment">
            <div className="diff-comment-head">
              <span className="diff-avatar">{c.initial}</span>
              <span className="diff-author">{c.author}</span>
              <span className="diff-age">{c.age}</span>
              <span className="diff-badge" data-pending={c.pending || undefined}>
                {c.badge}
              </span>
              <span className="diff-spacer" />
              {c.editable && texts && (
                <>
                  <button
                    type="button"
                    className="diff-comment-icon"
                    aria-label={texts.edit}
                    title={texts.edit}
                    onClick={() => {
                      ctx.dispatch({ kind: "edit", anchor, id: c.id, body: c.body });
                    }}
                  >
                    <Icon name="pencil" size={13} />
                  </button>
                  <button
                    type="button"
                    className="diff-comment-icon"
                    aria-label={texts.delete}
                    title={texts.delete}
                    onClick={() => {
                      request("diff.comment_delete", { id: c.id }).catch((e: unknown) => {
                        toastError(texts.delete, e);
                      });
                    }}
                  >
                    <Icon name="trash-2" size={13} />
                  </button>
                </>
              )}
            </div>
            <p className="diff-comment-body">{c.body}</p>
          </div>
        ),
      )}
      {composing && composing.editing === null && <ComposerBox ctx={ctx} composer={composing} />}
      {!composing && thread && ctx.comments?.can_comment && texts && (
        <button
          type="button"
          className="diff-reply"
          onClick={() => {
            ctx.dispatch({ kind: "open", anchor, replyTo: thread.comments[0]?.id ?? null });
          }}
        >
          {texts.reply}
        </button>
      )}
    </div>
  );
}

function UnifiedLine({ line, ctx }: { line: DiffLineView; ctx: CommentContext }) {
  return (
    <>
      <div className="diff-line" data-kind={line.kind}>
        <Gutter
          anchor={line.anchor}
          ctx={ctx}
          numbers={
            <>
              <Num n={line.old} />
              <Num n={line.new} />
            </>
          }
        />
        <span className="diff-sign">{line.sign}</span>
        <Code spans={line.spans} />
      </div>
      {line.anchor && <ThreadBlock anchor={line.anchor} ctx={ctx} />}
    </>
  );
}

function Half({ line, old, ctx }: { line: DiffLineView | undefined; old: boolean; ctx: CommentContext }) {
  if (!line) return <div className="diff-half" data-blank />;
  return (
    <div className="diff-half" data-kind={line.kind}>
      <Gutter anchor={line.anchor} ctx={ctx} numbers={<Num n={old ? line.old : line.new} />} />
      <span className="diff-sign">{line.sign}</span>
      <Code spans={line.spans} />
    </div>
  );
}

function SideRow({
  hunk,
  row,
  ctx,
}: {
  hunk: DiffHunkView;
  row: [number | null, number | null];
  ctx: CommentContext;
}) {
  const left = row[0] === null ? undefined : hunk.lines[row[0]];
  const right = row[1] === null ? undefined : hunk.lines[row[1]];
  return (
    <>
      <div className="diff-side-row">
        <Half line={left} old ctx={ctx} />
        <Half line={right} old={false} ctx={ctx} />
      </div>
      {rowAnchors(left?.anchor, right?.anchor).map((a) => (
        <ThreadBlock key={a.key} anchor={a} ctx={ctx} />
      ))}
    </>
  );
}

/** A hunk's rows, in chunks the webview lays out only near the view (`content-visibility`). */
function Hunk({ hunk, sideBySide, ctx }: { hunk: DiffHunkView; sideBySide: boolean; ctx: CommentContext }) {
  const count = sideBySide ? hunk.rows.length : hunk.lines.length;
  return (
    <>
      <div className="diff-hunk-header">{hunk.header}</div>
      {chunks(count).map(([start, end]) => (
        <div
          key={start}
          className="diff-chunk"
          style={{ containIntrinsicSize: `auto ${String((end - start) * LINE_HEIGHT)}px` }}
        >
          {sideBySide
            ? hunk.rows
                .slice(start, end)
                .map((row, i) => <SideRow key={start + i} hunk={hunk} row={row} ctx={ctx} />)
            : hunk.lines
                .slice(start, end)
                .map((line, i) => <UnifiedLine key={start + i} line={line} ctx={ctx} />)}
        </div>
      ))}
    </>
  );
}

function Counts({ file }: { file: DiffFileRow }) {
  if (file.binary) return <span className="diff-counts">binary</span>;
  return (
    <span className="diff-counts">
      {file.added !== null && <span className="diff-added">{file.added}</span>}
      {file.removed !== null && <span className="diff-removed">{file.removed}</span>}
    </span>
  );
}

function ViewedTick({ viewed, label, onToggle }: { viewed: boolean; label?: string; onToggle: () => void }) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={viewed}
      aria-label={label ?? "Viewed"}
      className="diff-viewed"
      data-viewed={viewed || undefined}
      onClick={(e) => {
        e.stopPropagation();
        onToggle();
      }}
    >
      <Icon name={viewed ? "square-check" : "square"} size={13} />
      {label !== undefined && <span>{label}</span>}
    </button>
  );
}

interface SectionProps {
  view: DiffTabView;
  file: DiffFileRow;
  load: FileLoad | undefined;
  open: boolean;
  sideBySide: boolean;
  ctx: CommentContext;
  root: HTMLElement | null;
  onVisible: (path: string) => void;
  onOpen: (path: string) => void;
  onViewed: (path: string) => void;
  register: (path: string, el: HTMLElement | null) => void;
}

/** One file of the diff: its header and its lines, asked for the first time it comes near the view. */
const FileSection = memo(function FileSection({
  view,
  file,
  load,
  open,
  sideBySide,
  ctx,
  root,
  onVisible,
  onOpen,
  onViewed,
  register,
}: SectionProps) {
  const ref = useRef<HTMLElement>(null);
  const wants = open && load === undefined;
  useEffect(() => {
    const el = ref.current;
    if (!wants || !el) return;
    if (typeof IntersectionObserver === "undefined") {
      onVisible(file.path);
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) onVisible(file.path);
      },
      { root, rootMargin: "800px 0px" },
    );
    observer.observe(el);
    return () => {
      observer.disconnect();
    };
  }, [wants, root, file.path, onVisible]);
  let body: ReactNode;
  if (file.binary_label !== null) {
    body = (
      <p className="diff-note">
        <Icon name="image" size={16} />
        {file.binary_label}
      </p>
    );
  } else if (!open) {
    body = (
      <div className="diff-note">
        <span>{file.collapsed}</span>
        <Button
          compact
          variant="secondary"
          label={view.load_diff}
          onClick={() => {
            onOpen(file.path);
          }}
        />
      </div>
    );
  } else if (load?.state === "loaded") {
    body = (
      <>
        {file.rename_label !== null && (
          <p className="diff-note" data-rename>
            <Icon name="file-symlink" size={16} />
            {file.rename_label}
          </p>
        )}
        {load.view.hunks.map((hunk, i) => (
          <Hunk key={i} hunk={hunk} sideBySide={sideBySide} ctx={ctx} />
        ))}
        {load.view.truncated !== null && <p className="diff-note">{load.view.truncated}</p>}
      </>
    );
  } else if (load?.state === "failed") {
    body = <p className="diff-note">{load.message}</p>;
  } else {
    // Local data arrives at once: room for the lines rather than a spinner.
    body = <div style={{ height: file.rows * LINE_HEIGHT }} />;
  }
  return (
    <section
      ref={(el) => {
        ref.current = el;
        register(file.path, el);
      }}
      className="diff-file"
      aria-label={file.label}
    >
      <header className="diff-file-header">
        <Icon name="chevron-down" size={12} />
        <span className="diff-file-name">
          <Path text={file.label} />
        </span>
        {file.note !== null && (
          <span className="diff-file-note" data-status={file.status}>
            {file.note}
          </span>
        )}
        <span className="diff-spacer" />
        <Counts file={file} />
        <ViewedTick
          viewed={file.viewed}
          label={view.viewed_label}
          onToggle={() => {
            onViewed(file.path);
          }}
        />
      </header>
      {body}
    </section>
  );
});

/** A path that may wrap after each "/" (as the frame's long names do), not inside a name. */
function Path({ text }: { text: string }) {
  return (
    <>
      {pathParts(text).map((part, i) => (
        <Fragment key={i}>
          {i > 0 && <wbr />}
          {part}
        </Fragment>
      ))}
    </>
  );
}

function FileRowButton({
  file,
  grouped,
  selected,
  onSelect,
  onViewed,
}: {
  file: DiffFileRow;
  grouped: boolean;
  selected: boolean;
  onSelect: (path: string) => void;
  onViewed: (path: string) => void;
}) {
  return (
    <div
      className="diff-list-row"
      data-selected={selected || undefined}
      role="option"
      aria-selected={selected}
      tabIndex={0}
      onClick={() => {
        onSelect(file.path);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect(file.path);
        }
      }}
    >
      <span className="diff-letter" data-status={file.binary ? "binary" : file.status}>
        {file.binary ? "B" : file.letter}
      </span>
      <span className="diff-list-label">
        <Path text={(grouped ? file.in_group : null) ?? file.label} />
      </span>
      <Counts file={file} />
      <ViewedTick
        viewed={file.viewed}
        onToggle={() => {
          onViewed(file.path);
        }}
      />
    </div>
  );
}

function FileList({
  view,
  width,
  selected,
  onSelect,
  onViewed,
}: {
  view: DiffTabView;
  width: number;
  selected: string | null;
  onSelect: (path: string) => void;
  onViewed: (path: string) => void;
}) {
  const [filter, setFilter] = useState("");
  const [toggledGroups, setToggledGroups] = useState<ReadonlySet<string>>(new Set());
  const row = (file: DiffFileRow) => (
    <FileRowButton
      key={file.path}
      file={file}
      grouped={view.groups !== null}
      selected={selected === file.path}
      onSelect={onSelect}
      onViewed={onViewed}
    />
  );
  return (
    <div
      className="diff-list"
      data-narrow={fileListWidth(width) < FILE_LIST || undefined}
      style={{ width: fileListWidth(width) }}
      role="listbox"
      aria-label={view.base}
    >
      {view.groups === null ? (
        view.files.map(row)
      ) : (
        <>
          <label className="diff-filter">
            <Icon name="search" size={13} />
            <input
              value={filter}
              placeholder={view.filter_placeholder}
              aria-label={view.filter_placeholder}
              onChange={(e) => {
                setFilter(e.target.value);
              }}
            />
            <span className="diff-filter-count">{view.viewed}</span>
          </label>
          {filteredGroups(view.groups, view.files, filter).map((group) => {
            const folded = groupFolded(group, toggledGroups) && filter.trim() === "";
            return (
              <div key={group.name} className="diff-group">
                <button
                  type="button"
                  className="diff-group-head"
                  aria-expanded={!folded}
                  onClick={() => {
                    setToggledGroups((t) => toggled(t, group.name));
                  }}
                >
                  <Icon name={folded ? "chevron-right" : "chevron-down"} size={12} />
                  {group.child === null ? (
                    <>
                      <Icon name="folder" size={12} />
                      <span className="diff-group-name">{folded ? group.folded_label : group.name}</span>
                      <span className="diff-group-count">{group.files.length}</span>
                    </>
                  ) : (
                    <>
                      <span className="diff-child-key">{group.child.key}</span>
                      <span className="diff-group-name">{group.child.title}</span>
                      <span className="diff-group-count">{group.child.counts}</span>
                    </>
                  )}
                </button>
                {!folded &&
                  group.files.map((i) => {
                    const file = view.files[i];
                    return file ? row(file) : null;
                  })}
              </div>
            );
          })}
        </>
      )}
    </div>
  );
}

export interface DiffTabBodyProps {
  ticket: string;
  view: DiffTabView;
  comments: DiffComments | undefined;
  /** The tab's width (the panel's). */
  width: number;
  /** Reloads the view after a tick or a mode change. */
  reload: () => void;
  /** How it reached Review and the final validation report (VAL-12); none without a Review entry. */
  review?: DiffReviewView | null;
  /** The recorded decisions under the agent summary (DEC-04); none when there are none. */
  decisions?: DecisionsSection | null;
  /** Change… on one of them (T-0288's dialog). */
  onChangeDecision?: (item: DecisionItemView) => void;
}

/** "Validated · round 2" with how it got there (*07 → Row · validation report & Auto-merge*). */
function ReviewEntry({ review }: { review: DiffReviewView }) {
  return (
    <div className="diff-entry">
      <span className="status-pill" data-palette={review.entry.palette}>
        <Icon name={review.entry.icon} size={12} />
        <span>{review.entry.label}</span>
      </span>
      {review.entry.text !== null && <span className="diff-muted">{review.entry.text}</span>}
    </div>
  );
}

/** The tab for a loaded view (also what the markup tests render). */
export function DiffTabBody({
  ticket,
  view,
  comments,
  width,
  reload,
  review = null,
  decisions = null,
  onChangeDecision,
}: DiffTabBodyProps) {
  const [loads, setLoads] = useState<ReadonlyMap<string, FileLoad>>(new Map());
  const [opened, setOpened] = useState<ReadonlySet<string>>(new Set());
  const [selected, setSelected] = useState<string | null>(null);
  const [composer, dispatch] = useReducer(composerReducer, null);
  const [pane, setPane] = useState<HTMLElement | null>(null);
  const sections = useRef(new Map<string, HTMLElement>());
  /** The files asked for in this file list (each once). */
  const asked = useRef(new Set<string>());
  const revision = useRef(view.revision);

  // A new file list: lines loaded for the old one go.
  useEffect(() => {
    if (revision.current === view.revision) return;
    revision.current = view.revision;
    asked.current = new Set();
    setLoads(new Map());
    setOpened(new Set());
  }, [view.revision]);
  // Comments can't be added any more (sent back, approved): the open box goes.
  useEffect(() => {
    if (comments && !comments.can_comment) dispatch({ kind: "cancel" });
  }, [comments]);

  const onVisible = useCallback(
    (path: string) => {
      if (asked.current.has(path)) return;
      const mine = asked.current;
      mine.add(path);
      setLoads((current) => new Map(current).set(path, { state: "loading" }));
      const done = (load: FileLoad) => {
        // An answer for an older file list is dropped.
        if (asked.current === mine) setLoads((current) => new Map(current).set(path, load));
      };
      (request("diff.file", { key: ticket, path }) as Promise<DiffFileView>).then(
        (file) => {
          done({ state: "loaded", view: file });
        },
        (error: unknown) => {
          done({ state: "failed", message: asRequestError(error).message });
        },
      );
    },
    [ticket],
  );
  const onOpen = useCallback((path: string) => {
    setOpened((o) => new Set(o).add(path));
  }, []);
  const onViewed = useCallback(
    (path: string) => {
      request("diff.viewed", { key: ticket, round: view.round, path }).then(reload, ignore);
    },
    [ticket, view.round, reload],
  );
  const register = useCallback((path: string, el: HTMLElement | null) => {
    if (el) sections.current.set(path, el);
    else sections.current.delete(path);
  }, []);
  const onSelect = useCallback((path: string) => {
    setSelected(path);
    sections.current.get(path)?.scrollIntoView({ block: "start" });
  }, []);

  const save = useCallback(
    (text: string) => {
      if (composer === null || !canSave(composer, text)) return;
      const { method, params } = saveRequest(composer, text, ticket);
      dispatch({ kind: "saving" });
      request(method, params).then(
        () => {
          dispatch({ kind: "saved" });
        },
        (error: unknown) => {
          dispatch({ kind: "failed", message: asRequestError(error).message });
        },
      );
    },
    [composer, ticket],
  );
  const threads = useMemo(() => threadsByAnchor(comments), [comments]);
  const ctx = useMemo<CommentContext>(
    () => ({ ticket, comments, threads, composer, dispatch, save }),
    [ticket, comments, threads, composer, save],
  );
  const sideBySide = showsSideBySide(view.side_by_side, width, view.side_by_side_from);

  return (
    <div className="diff">
      {review && <ReviewEntry review={review} />}
      {view.summary && (
        <div className="diff-summary">
          <button
            type="button"
            className="diff-summary-head"
            aria-expanded={view.summary.open}
            onClick={() => {
              request("diff.summary_fold", { key: ticket }).then(reload, ignore);
            }}
          >
            <Icon name={view.summary.open ? "chevron-down" : "chevron-right"} size={12} />
            <Icon name="sparkles" size={13} />
            <span className="diff-summary-title">{view.summary.title}</span>
            <span className="diff-spacer" />
            <span className="diff-muted">{view.summary.note}</span>
          </button>
          {view.summary.open && <p className="diff-summary-text">{view.summary.text}</p>}
        </div>
      )}
      {decisions && (
        <div className="diff-decisions">
          <DecisionsList section={decisions} onChange={onChangeDecision} />
        </div>
      )}
      {review?.report && <Report report={review.report} ticket={ticket} />}
      {review?.foot != null && <p className="diff-entry-foot">{review.foot}</p>}
      <div className="diff-toolbar">
        <span className="diff-base">
          <Icon name="git-compare" size={13} />
          <span className="diff-muted">{view.base_prefix}</span>
          <span className="diff-base-name">{view.base}</span>
          <span className="diff-muted">{view.rest}</span>
        </span>
        <span className="diff-spacer" />
        {comments?.pending_label && (
          <span className="diff-pending">
            <Icon name="message-square" size={12} />
            {comments.pending_label}
          </span>
        )}
        <span className="diff-muted">{view.viewed}</span>
        <Segmented
          label={view.modes.join(" / ")}
          options={view.modes}
          selected={view.side_by_side ? 1 : 0}
          onSelect={(i) => {
            request("diff.mode", { side_by_side: i === 1 }).then(reload, ignore);
          }}
        />
      </div>
      {view.empty ? (
        <div className="diff-empty-wrap">
          <div className="diff-empty">
            <div className="diff-empty-head">
              <span>{view.empty.title}</span>
              <span className="diff-muted">{view.empty.base}</span>
            </div>
            <p>{view.empty.text}</p>
          </div>
        </div>
      ) : (
        <div className="diff-main">
          <FileList view={view} width={width} selected={selected} onSelect={onSelect} onViewed={onViewed} />
          <div className="diff-pane" ref={setPane}>
            {view.files.map((file) => (
              <FileSection
                key={file.path}
                view={view}
                file={file}
                load={loads.get(file.path)}
                open={showsLines(file, opened)}
                sideBySide={sideBySide}
                ctx={ctx}
                root={pane}
                onVisible={onVisible}
                onOpen={onOpen}
                onViewed={onViewed}
                register={register}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

/** The Diff tab registered in the panel: loads the file list and the comments and follows their events. */
export function DiffTab({ ticket }: TicketProps) {
  const [tick, setTick] = useState(0);
  const reload = useCallback(() => {
    setTick((t) => t + 1);
  }, []);
  // `tick` makes a new load after a tick or a mode change (the service ignores it).
  const load = useCallback(
    () => request("diff.view", { key: ticket, tick }) as Promise<DiffLoad>,
    [ticket, tick],
  );
  const loadComments = useCallback(
    () => request("diff.comments", { key: ticket }) as Promise<DiffComments>,
    [ticket],
  );
  const loadReview = useCallback(
    () => request("diff.review", { key: ticket }) as Promise<DiffReviewView | null>,
    [ticket],
  );
  const { view: loaded, error } = useServiceView(load, DIFF_EVENTS);
  const { view: review } = useServiceView(loadReview, REVIEW_EVENTS);
  const { view: comments } = useServiceView(loadComments, COMMENT_EVENTS);
  const loadDecisions = useCallback(
    () => request("review.decisions", { key: ticket }) as Promise<DecisionsSection | null>,
    [ticket],
  );
  const { view: decisions } = useServiceView(loadDecisions, REVIEW_EVENTS);
  const [ref, width] = useWidth<HTMLDivElement>();
  const view = loaded?.view;
  return (
    <div className="diff-host" ref={ref}>
      {view && view.key === ticket ? (
        <DiffTabBody
          key={ticket}
          ticket={ticket}
          view={view}
          comments={comments?.key === ticket ? comments : undefined}
          width={width}
          reload={reload}
          review={review}
          decisions={decisions}
          onChangeDecision={changeDecision}
        />
      ) : (
        <p className="diff-problem">{loaded?.problem ?? error?.message ?? ""}</p>
      )}
    </div>
  );
}
