// The Ticket tab's read mode (UI-05, TKT-05; d0019; *08 → Ticket tab · read mode, with agent edits, Compare · before /
// after*): who created it, What and why as simple Markdown, Priority (changed in place), Depends on with live
// statuses, the base and the External ID. A field an agent changed since the user last looked carries a violet
// "Updated by agent" marker with Compare.
import { useRef, useState } from "react";

import { openTicket } from "../../app/selection";
import { AnchoredMenu, PriorityLabel, StatusPill } from "../../components";
import type { EditorCompareEntry } from "../../generated/EditorCompareEntry";
import type { EditorField } from "../../generated/EditorField";
import type { EditorReadView } from "../../generated/EditorReadView";
import type { EditorTexts } from "../../generated/EditorTexts";
import type { MarkdownBlock } from "../../generated/MarkdownBlock";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";

const ignore = () => undefined;

export function Markdown({ blocks, empty }: { blocks: readonly MarkdownBlock[]; empty: string }) {
  if (blocks.length === 0) return <p className="editor-md-empty">{empty}</p>;
  return (
    <div className="editor-md">
      {blocks.map((block, i) => {
        const key = String(i);
        switch (block.kind) {
          case "paragraph":
            return (
              <p key={key} className="editor-md-p">
                {block.text}
              </p>
            );
          case "heading":
            return (
              <p key={key} className="editor-md-h">
                {block.text}
              </p>
            );
          case "code":
            return (
              <pre key={key} className="editor-md-code">
                {block.text}
              </pre>
            );
          case "list":
            return (
              <ul key={key} className="editor-md-list">
                {block.items.map((item, j) => (
                  <li key={String(j)}>{item}</li>
                ))}
              </ul>
            );
        }
      })}
    </div>
  );
}

function Section({
  name,
  field,
  read,
  texts,
  onCompare,
}: {
  name: string;
  field: EditorField | null;
  read: EditorReadView;
  texts: EditorTexts;
  onCompare: (field: EditorField) => void;
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
}: {
  ticket: string;
  read: EditorReadView;
  texts: EditorTexts;
  labels: { deps: string; base: string; externalId: string; priority: string; body: string; title: string };
  onCompare: (field: EditorField) => void;
}) {
  const section = (name: string, field: EditorField | null) => (
    <Section name={name} field={field} read={read} texts={texts} onCompare={onCompare} />
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
      {section(labels.body, "body")}
      <Markdown blocks={read.body} empty={texts.nothing_written} />
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
