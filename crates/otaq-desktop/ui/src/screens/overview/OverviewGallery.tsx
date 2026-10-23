// The gallery's Overview parts (T-0465; *25 → Opening a ticket*, *Editing the description*, *Attachments*): SHOP-22's
// Attachments column as Rust words it (`gallery.overview`), and the tab row's bar once the description changed.
import { useCallback } from "react";

import type { OverviewView } from "../../generated/OverviewView";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Attachments } from "./Overview";
import { SaveBarView } from "./SaveBar";

const ignore = () => undefined;

export function OverviewGallery() {
  const load = useCallback(() => request("gallery.overview") as Promise<OverviewView>, []);
  const { view, error } = useServiceView(load, []);
  if (error) return <p role="alert">{error.message}</p>;
  if (!view) return null;
  return (
    <div className="gallery-stack overview-gallery">
      <figure className="gallery-stack-tight">
        <figcaption className="gallery-caption">The tab row once the description changed</figcaption>
        <div className="sheet-tabs overview-gallery-tabs">
          <SaveBarView bar={view.bar} saving={false} onSave={ignore} onDiscard={ignore} />
        </div>
      </figure>
      <figure className="gallery-stack-tight">
        <figcaption className="gallery-caption">{view.key} · Attachments, 300 px</figcaption>
        <Attachments ticket={view.key} view={view} error={null} onAdd={ignore} />
      </figure>
    </div>
  );
}
