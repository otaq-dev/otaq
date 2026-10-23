// The gallery's project Files view (T-0478; `s28-03`, `s28-04`, `s28-09`): shop-web on All with src/lib open and
// pricing.ts picked, Needs you, the loading and empty states, the picker and SHOP-29's worktree's note. The fixtures
// are Rust's (`gallery.files_view`).
import { useCallback } from "react";

import type { FilesViewGallery as Fixtures } from "../../generated/FilesViewGallery";
import type { FilesViewPage } from "../../generated/FilesViewPage";
import type { FilesTreeRow } from "../../generated/FilesTreeRow";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { FilesViewBody, PickerBody } from "./FilesView";

const ignore = () => undefined;

function Frame({
  caption,
  page,
  rows,
  file,
}: {
  caption: string;
  page: FilesViewPage;
  rows: readonly FilesTreeRow[];
  file?: string;
}) {
  const open = new Set(rows.filter((r) => r.kind === "folder").map((r) => r.path));
  return (
    <figure className="gallery-stack-tight">
      <figcaption className="gallery-caption">{caption}</figcaption>
      <div style={{ height: 460, display: "flex" }}>
        <FilesViewBody
          project={page.project}
          page={page}
          rows={rows}
          open={open}
          file={file ?? null}
          found={null}
          query=""
          onQuery={ignore}
          selectedMatch={-1}
          onOpenMatch={ignore}
          onRow={ignore}
          onFilter={ignore}
          source={
            <div className="files-source files-view-source">
              <Icon name="git-branch" size={14} />
              <span className="files-source-text">
                <span className="files-source-title">{page.source.title}</span>
                <span className="files-source-sub">{page.source.subtitle}</span>
              </span>
              <Icon name="chevron-down" size={12} />
            </div>
          }
        />
      </div>
    </figure>
  );
}

export function FilesViewGallery() {
  const load = useCallback(() => request("gallery.files_view") as Promise<Fixtures>, []);
  const { view: g, error } = useServiceView(load, []);
  if (error) return <p role="alert">{error.message}</p>;
  if (!g) return null;
  return (
    <div className="gallery-stack">
      <Frame
        caption="shop-web · All · src/lib open, pricing.ts picked"
        page={g.all}
        rows={g.all_rows}
        file="src/lib/pricing.ts"
      />
      <Frame
        caption="Needs you · the files of the tickets that need you"
        page={g.needs_you}
        rows={g.needs_you.rows}
      />
      <Frame caption="Loading · the base at once, the counts follow" page={g.loading} rows={g.loading_rows} />
      <Frame caption="Nothing changing" page={g.empty} rows={[]} />
      <figure className="gallery-stack-tight">
        <figcaption className="gallery-caption">The picker · Find a ticket or epic</figcaption>
        <div className="popover" style={{ position: "relative", width: "fit-content" }}>
          <PickerBody picker={g.picker} query="" onQuery={ignore} onPick={ignore} />
        </div>
      </figure>
      <figure className="gallery-stack-tight">
        <figcaption className="gallery-caption">
          SHOP-29’s worktree · the back link, the source and the note
        </figcaption>
        <div className="files-side" style={{ width: 300 }}>
          <button type="button" className="files-view-back">
            <Icon name="arrow-left" size={12} />
            <span>{g.picked.back}</span>
          </button>
          <div className="files-source">
            <Icon name="folder-git-2" size={14} />
            <div className="files-source-text">
              <span className="files-source-title">{g.picked.tab.source.title}</span>
              <span className="files-source-sub">{g.picked.tab.source.subtitle}</span>
            </div>
          </div>
        </div>
        <div className="files-view-picked-note">
          <span className="files-quiet">{g.picked.note}</span>
          <button type="button" className="files-view-open">
            <Icon name="external-link" size={12} />
            <span>{g.picked.open}</span>
          </button>
        </div>
      </figure>
    </div>
  );
}
