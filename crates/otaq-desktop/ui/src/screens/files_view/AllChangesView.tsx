// All changes in the project Files view (FIL-08; T-0479; `s28-06`): Changed with no file picked shows every diff in
// one scroll, grouped by ticket, the ones that need you first: "8 tickets · 11 files against main · grouped by ticket,
// the ones that need you first", Unified · Split (the Files tab's, kept for every ticket), each group with its tool,
// key, title, line, files, +/− and Open (its sheet on Files), its files folded to a line until opened; the first group
// opens on its first file. The files' sections are the Files tab's (`ChangeSection`), without line comments (d0017).
// Words and order are Rust's (`files_view.changes`).
import { useCallback, useRef, useState } from "react";

import { Segmented, ToolAvatar } from "../../components";
import type { FilesChangeRow } from "../../generated/FilesChangeRow";
import type { FilesViewChanges } from "../../generated/FilesViewChanges";
import type { FilesViewGroup } from "../../generated/FilesViewGroup";
import { Icon } from "../../theme/Icon";
import { paletteInk } from "../../cards/look";
import { ChangeSection } from "../files/FilesTab";
import { showsLines, showsSideBySide, toggled } from "../files/logic";

export interface AllChangesViewProps {
  changes: FilesViewChanges;
  /** The scrolling pane, for loading each file's lines as it comes near. */
  pane: HTMLElement | null;
  width: number;
  onMode: (sideBySide: boolean) => void;
  /** Open file: the file in the viewer. */
  onOpenFile: (path: string) => void;
  /** Open: the ticket's sheet on its Files tab. */
  onOpen: (key: string) => void;
}

function GroupHead({
  group,
  open,
  openLabel,
  onToggle,
  onOpen,
}: {
  group: FilesViewGroup;
  open: boolean;
  openLabel: string;
  onToggle: () => void;
  onOpen: () => void;
}) {
  return (
    <div className="files-view-group-head">
      <button type="button" className="files-view-group-toggle" aria-expanded={open} onClick={onToggle}>
        <Icon name={open ? "chevron-down" : "chevron-right"} size={12} />
        <span className="files-view-edge" style={{ background: `var(--${group.colour})` }} aria-hidden />
        {group.avatar && <ToolAvatar avatar={group.avatar} />}
        <span className="files-view-owner-key">{group.key}</span>
        <span className="files-view-group-title">{group.title}</span>
        {group.status !== null && (
          <span
            className="files-view-group-status"
            style={group.palette === null ? undefined : { color: paletteInk(group.palette) }}
          >
            {`· ${group.status}`}
          </span>
        )}
      </button>
      <span className="files-spacer" />
      <span className="files-view-group-summary">{group.summary}</span>
      <button type="button" className="files-link" onClick={onOpen}>
        {openLabel}
        <Icon name="arrow-up-right" size={11} />
      </button>
    </div>
  );
}

/** Every ticket's changes, grouped (also what the gallery and the markup tests render). */
export function AllChangesView({ changes, pane, width, onMode, onOpenFile, onOpen }: AllChangesViewProps) {
  // What the user opened or closed, against what each group and file shows at first.
  const [flippedGroups, setFlippedGroups] = useState<ReadonlySet<string>>(new Set());
  const [flippedFiles, setFlippedFiles] = useState<ReadonlySet<string>>(new Set());
  const [loadedLarge, setLoadedLarge] = useState<ReadonlySet<string>>(new Set());
  const sections = useRef(new Map<string, HTMLElement>());
  const register = useCallback((id: string, el: HTMLElement | null) => {
    if (el) sections.current.set(id, el);
    else sections.current.delete(id);
  }, []);
  const onOpenLines = useCallback((id: string) => {
    setLoadedLarge((o) => new Set(o).add(id));
  }, []);
  const onOpenRow = useCallback(
    (file: FilesChangeRow) => {
      onOpenFile(file.path);
    },
    [onOpenFile],
  );
  const sideBySide = showsSideBySide(changes.side_by_side, width, changes.side_by_side_from);
  return (
    <div className="files-changes">
      <div className="files-main-head">
        <div className="files-main-title">
          <span className="files-title">{changes.title}</span>
          <span className="files-subtitle">{changes.summary}</span>
        </div>
        <Segmented
          label={changes.modes.join(" / ")}
          options={changes.modes}
          selected={changes.side_by_side ? 1 : 0}
          onSelect={(i) => {
            onMode(i === 1);
          }}
        />
      </div>
      {changes.empty && (
        <div className="files-view-empty">
          <p className="files-view-empty-title">{changes.empty.title}</p>
          <p className="files-quiet">{changes.empty.text}</p>
        </div>
      )}
      {changes.groups.map((group) => {
        const open = group.open !== flippedGroups.has(group.key);
        return (
          <section key={group.key} className="files-view-group" aria-label={`${group.key} ${group.title}`}>
            <GroupHead
              group={group}
              open={open}
              openLabel={changes.open}
              onToggle={() => {
                setFlippedGroups((s) => toggled(s, group.key));
              }}
              onOpen={() => {
                onOpen(group.key);
              }}
            />
            {open && (
              <div className="files-view-group-files">
                {group.files.map((file) => {
                  const id = `${group.key}\u0000${file.id}`;
                  const folded = (group.unfolded !== file.id) !== flippedFiles.has(id);
                  return (
                    <ChangeSection
                      key={file.id}
                      ticket={group.key}
                      file={file}
                      open={showsLines(file, loadedLarge)}
                      sideBySide={sideBySide}
                      ctx={null}
                      root={pane}
                      openFileLabel={changes.open_file}
                      loadDiffLabel={changes.load_diff}
                      onOpenLines={onOpenLines}
                      onOpenFile={onOpenRow}
                      register={register}
                      folded={folded}
                      onFold={() => {
                        setFlippedFiles((s) => toggled(s, id));
                      }}
                    />
                  );
                })}
              </div>
            )}
          </section>
        );
      })}
    </div>
  );
}
