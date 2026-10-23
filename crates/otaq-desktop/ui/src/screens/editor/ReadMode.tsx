// The description read as it reads (UI-05, TKT-05, TKT-10, TKT-11; d0019, d0078; *05 → Ticket tab · rich Markdown read
// mode*), shared by Overview (T-0465): What and why rendered by the read-only description editor; a field an agent
// changed since the user last looked carries a violet "Updated by agent" marker with Compare on its label row, which
// ends with Copy as Markdown and Expand (d0078 D7). Compare (*08 → 7 · Updated by agent · compare*) renders the
// description's before / after, with a Rendered / Markdown toggle for its source; other fields stay plain text.
import { type ReactNode, useEffect, useRef, useState } from "react";

import { Segmented, Tooltip } from "../../components";
import type { EditorCompareEntry } from "../../generated/EditorCompareEntry";
import type { EditorField } from "../../generated/EditorField";
import type { EditorReadView } from "../../generated/EditorReadView";
import type { EditorTexts } from "../../generated/EditorTexts";
import { MarkdownEditor } from "../../markdown";
import { Icon } from "../../theme/Icon";
import { COPIED_FOR, copyMarkdown } from "./logic";

const ignore = () => undefined;

/**
 * Copy as Markdown (*05 → c · Read mode*, *c · Copied state*): copies the stored description; the clipboard icon turns
 * into a check and the tooltip into a green check followed by "Copied".
 */
export function CopyMarkdown({ stored, texts }: { stored: string; texts: EditorTexts }) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  useEffect(
    () => () => {
      window.clearTimeout(timer.current);
    },
    [],
  );
  return (
    <Tooltip text={copied ? texts.copied : texts.copy_markdown} icon={copied ? "check" : undefined}>
      <button
        type="button"
        className="editor-tool"
        data-copied={copied || undefined}
        aria-label={texts.copy_markdown}
        onClick={() => {
          copyMarkdown(stored).then(() => {
            setCopied(true);
            window.clearTimeout(timer.current);
            timer.current = window.setTimeout(() => {
              setCopied(false);
            }, COPIED_FOR);
          }, ignore);
        }}
      >
        <Icon name={copied ? "check" : "clipboard-copy"} size={14} />
      </button>
    </Tooltip>
  );
}

/**
 * The icons at the right end of the "What and why" label row: Copy as Markdown, then Expand, which opens the
 * full-window view (T-0260) through `onExpand`; without it, it shows disabled.
 */
export function DescriptionTools({
  stored,
  texts,
  onExpand,
}: {
  stored: string;
  texts: EditorTexts;
  onExpand: (() => void) | undefined;
}) {
  return (
    <span className="editor-tools">
      <CopyMarkdown stored={stored} texts={texts} />
      <Tooltip text={texts.expand}>
        <button
          type="button"
          className="editor-tool"
          aria-label={texts.expand}
          disabled={onExpand === undefined}
          onClick={onExpand}
        >
          <Icon name="maximize" size={14} />
        </button>
      </Tooltip>
    </span>
  );
}

/** What and why, rendered; it is never edited or re-saved here. */
export function Description({
  read,
  texts,
  label,
}: {
  read: EditorReadView;
  texts: EditorTexts;
  label: string;
}) {
  if (read.body_empty) return <p className="editor-md-empty">{texts.nothing_written}</p>;
  return (
    <MarkdownEditor value={read.body_markdown} editable={false} label={label} className="editor-read-body" />
  );
}

/**
 * A field's label row: its name in capitals, then, when an agent changed it since the user last looked, "Updated by
 * agent" with Compare; `hint` and `tools` at the right end (Overview's "Click to edit", Copy as Markdown and Expand).
 */
export function LabelRow({
  name,
  field,
  read,
  texts,
  onCompare,
  hint,
  tools,
}: {
  name: string;
  field: EditorField | null;
  read: EditorReadView;
  texts: EditorTexts;
  onCompare: (field: EditorField) => void;
  /** Muted words after the marker and Compare. */
  hint?: ReactNode;
  /** Icons after the marker and Compare, at the row's right end. */
  tools?: ReactNode;
}) {
  const marked = field !== null && read.marked.includes(field);
  return (
    <div className="editor-section">
      <span className="editor-section-name">{name.toUpperCase()}</span>
      <span className="editor-spacer" />
      {marked && (
        <>
          <span className="editor-marker">
            <Icon name="sparkles" size={11} />
            {texts.marker}
          </span>
          <button
            type="button"
            className="editor-link"
            onClick={() => {
              onCompare(field);
            }}
          >
            {texts.compare}
          </button>
        </>
      )}
      {hint}
      {tools}
    </div>
  );
}

type Side = "before" | "after";

/** One side of a comparison: the description rendered by the read-only editor or as its source; other fields plain. */
function CompareSide({
  side,
  text,
  markdown,
  source,
  texts,
}: {
  side: Side;
  text: string;
  markdown: boolean;
  source: boolean;
  texts: EditorTexts;
}) {
  const name = side === "before" ? texts.before : texts.after;
  let body: ReactNode = <p>{text}</p>;
  if (markdown && source) body = <p className="editor-compare-source">{text}</p>;
  else if (markdown)
    body = <MarkdownEditor value={text} editable={false} label={name} className="editor-read-body" />;
  return (
    <div className="editor-compare-col" data-side={side}>
      <span className="editor-section-name">{name}</span>
      {body}
    </div>
  );
}

/**
 * One before / after pair. The description's pair is rendered by default; its heading row ends with the Rendered /
 * Markdown toggle, which shows the source instead (d0078).
 */
function CompareEntryView({ entry, texts }: { entry: EditorCompareEntry; texts: EditorTexts }) {
  const [source, setSource] = useState(false);
  const markdown = entry.field === "body";
  return (
    <div className="editor-compare-entry">
      <div className="editor-compare-head">
        <p className="editor-compare-heading">{entry.heading}</p>
        {markdown && (
          <Segmented
            label={texts.compare_view}
            options={[texts.rendered, texts.markdown]}
            selected={source ? 1 : 0}
            onSelect={(i) => {
              setSource(i === 1);
            }}
          />
        )}
      </div>
      <div className="editor-compare-cols">
        <CompareSide side="before" text={entry.before} markdown={markdown} source={source} texts={texts} />
        <CompareSide side="after" text={entry.after} markdown={markdown} source={source} texts={texts} />
      </div>
    </div>
  );
}

/** Compare · before / after (TKT-05): the agent's edits, newest first. */
export function Compare({ entries, texts }: { entries: readonly EditorCompareEntry[]; texts: EditorTexts }) {
  return (
    <div className="editor-compare">
      {entries.map((entry, i) => (
        <CompareEntryView key={String(i)} entry={entry} texts={texts} />
      ))}
    </div>
  );
}
