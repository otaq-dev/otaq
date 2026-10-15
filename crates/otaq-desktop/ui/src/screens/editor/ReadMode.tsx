// The Ticket tab's read mode (UI-05, TKT-05, TKT-10, TKT-11; d0019, d0078; *08 → Ticket tab · read mode, with agent
// edits, Compare · before / after*, *05 → Ticket tab · rich Markdown read mode*): who created it, What and why rendered
// by the read-only description editor, Priority (changed in place), Depends on with live statuses, the base and the
// External ID. A field an agent changed since the user last looked carries a violet "Updated by agent" marker with
// Compare. The "What and why" label row ends with Copy as Markdown and Expand, in every ticket state (d0078 D7).
import { type ReactNode, useEffect, useRef, useState } from "react";

import { openTicket } from "../../app/selection";
import { AnchoredMenu, PriorityLabel, StatusPill, Tooltip } from "../../components";
import type { EditorCompareEntry } from "../../generated/EditorCompareEntry";
import type { EditorField } from "../../generated/EditorField";
import type { EditorReadView } from "../../generated/EditorReadView";
import type { EditorTexts } from "../../generated/EditorTexts";
import { MarkdownEditor } from "../../markdown";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { COPIED_FOR, copyMarkdown } from "./logic";

const ignore = () => undefined;

/** Copy as Markdown: copies the stored description; the icon turns into a check and the tooltip says "Copied ✓". */
function CopyMarkdown({ stored, texts }: { stored: string; texts: EditorTexts }) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  useEffect(
    () => () => {
      window.clearTimeout(timer.current);
    },
    [],
  );
  return (
    <Tooltip text={copied ? texts.copied : texts.copy_markdown}>
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
        <Icon name={copied ? "check" : "copy"} size={14} />
      </button>
    </Tooltip>
  );
}

/**
 * The icons at the right end of the "What and why" label row: Copy as Markdown, then Expand. Expand is the slot the
 * full-window view (T-0260) fills through `onExpand`; until then it shows, disabled.
 */
function DescriptionTools({
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
          <Icon name="maximize-2" size={14} />
        </button>
      </Tooltip>
    </span>
  );
}

/** What and why, rendered; it is never edited or re-saved here. */
function Description({ read, texts, label }: { read: EditorReadView; texts: EditorTexts; label: string }) {
  if (read.body_empty) return <p className="editor-md-empty">{texts.nothing_written}</p>;
  return (
    <MarkdownEditor value={read.body_markdown} editable={false} label={label} className="editor-read-body" />
  );
}

function Section({
  name,
  field,
  read,
  texts,
  onCompare,
  tools,
}: {
  name: string;
  field: EditorField | null;
  read: EditorReadView;
  texts: EditorTexts;
  onCompare: (field: EditorField) => void;
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
      {tools}
    </div>
  );
}

function PriorityPicker({ ticket, read }: { ticket: string; read: EditorReadView }) {
  const anchor = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  return (
    <span className="editor-read-priority">
      <button
        ref={anchor}
        type="button"
        className="panel-priority"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => {
          setOpen(!open);
        }}
      >
        <PriorityLabel priority={read.priority} />
        <Icon name="chevron-down" size={11} />
      </button>
      {read.follows !== null && <span className="editor-hint">{read.follows}</span>}
      {open && (
        <AnchoredMenu
          anchor={anchor}
          label={read.priority.label}
          items={read.priority_menu}
          width={180}
          onChoose={(item) => {
            setOpen(false);
            request("panel.priority", { key: ticket, item }).catch(ignore);
          }}
          onClose={() => {
            setOpen(false);
          }}
        />
      )}
    </span>
  );
}

export function ReadMode({
  ticket,
  read,
  texts,
  labels,
  onCompare,
  onExpand,
}: {
  ticket: string;
  read: EditorReadView;
  texts: EditorTexts;
  labels: { deps: string; base: string; externalId: string; priority: string; body: string; title: string };
  onCompare: (field: EditorField) => void;
  /** Expand on the "What and why" row: the full-window view (T-0260). Without it the button is disabled. */
  onExpand?: () => void;
}) {
  const section = (name: string, field: EditorField | null, tools?: ReactNode) => (
    <Section name={name} field={field} read={read} texts={texts} onCompare={onCompare} tools={tools} />
  );
  return (
    <div className="editor-read">
      <p className="editor-origin" data-agent={read.origin_agent || undefined}>
        <Icon name={read.origin_agent ? "sparkles" : "user"} size={12} />
        {read.origin}
      </p>
      {read.marked.includes("title") && (
        <>
          {section(labels.title, "title")}
          <p className="editor-read-title">{read.title}</p>
        </>
      )}
      {section(
        labels.body,
        "body",
        <DescriptionTools stored={read.body_markdown} texts={texts} onExpand={onExpand} />,
      )}
      <Description read={read} texts={texts} label={labels.body} />
      {section(labels.priority, "priority")}
      <PriorityPicker ticket={ticket} read={read} />
      {section(labels.deps, null)}
      {read.deps.length === 0 ? (
        <p className="editor-md-empty">{texts.deps_none}</p>
      ) : (
        <div className="editor-read-deps">
          {read.deps.map((dep) => (
            <button
              key={dep.key}
              type="button"
              className="editor-chip editor-chip-link"
              onClick={() => {
                openTicket(dep.key);
              }}
            >
              <span className="editor-chip-key">{dep.key}</span>
              <StatusPill pill={dep.pill} />
            </button>
          ))}
        </div>
      )}
      {section(labels.base, "base_branch")}
      <p className="editor-read-mono">{read.base}</p>
      {read.external_id !== null && (
        <>
          {section(labels.externalId, "external_id")}
          <p className="editor-read-mono">{read.external_id}</p>
        </>
      )}
    </div>
  );
}

export function Compare({ entries, texts }: { entries: readonly EditorCompareEntry[]; texts: EditorTexts }) {
  return (
    <div className="editor-compare">
      {entries.map((entry, i) => (
        <div key={String(i)} className="editor-compare-entry">
          <p className="editor-compare-heading">{entry.heading}</p>
          <div className="editor-compare-cols">
            <div className="editor-compare-col" data-side="before">
              <span className="editor-section-name">{texts.before}</span>
              <p>{entry.before}</p>
            </div>
            <div className="editor-compare-col" data-side="after">
              <span className="editor-section-name">{texts.after}</span>
              <p>{entry.after}</p>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
