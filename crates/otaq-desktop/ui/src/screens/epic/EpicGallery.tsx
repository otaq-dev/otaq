// The gallery's epic sheet (T-0469; `s25-11`): the Wishlist epic's header pills with its lead asking, Overview's
// TICKETS grid and the Tickets tab, as Rust words them (`gallery.epic_sheet`). Nothing is sent.
import { useCallback } from "react";

import type { EpicSheetGallery as Fixtures } from "../../generated/EpicSheetGallery";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { EpicGrid, EpicHeadPills, TicketsBody } from "./EpicSheet";

export function EpicGallery() {
  const load = useCallback(() => request("gallery.epic_sheet") as Promise<Fixtures>, []);
  const { view: g, error } = useServiceView(load, []);
  if (error) return <p role="alert">{error.message}</p>;
  if (!g) return null;
  return (
    <div className="gallery-stack">
      <figure className="gallery-stack-tight">
        <figcaption className="gallery-caption">The header: progress and the lead&apos;s state</figcaption>
        <div className="sheet-title-row">
          <EpicHeadPills head={g.head} />
        </div>
      </figure>
      <figure className="gallery-stack-tight" style={{ width: 640 }}>
        <figcaption className="gallery-caption">Overview · {g.overview.grid.heading}</figcaption>
        <EpicGrid grid={g.overview.grid} />
      </figure>
      <figure className="gallery-stack-tight" style={{ width: 1000 }}>
        <figcaption className="gallery-caption">The Tickets tab</figcaption>
        <TicketsBody view={g.tickets} />
      </figure>
    </div>
  );
}
