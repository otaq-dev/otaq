// The palette's states for the design gallery (s02-20 … s02-23): Rust's views of the artboards' samples, drawn still.
import type { PaletteGalleryFrame } from "../../generated/PaletteGalleryFrame";
import { PaletteBox } from "./Palette";
import { firstPickable, splitQuery } from "./logic";

function StillField({ query, placeholder }: { query: string; placeholder: string }) {
  const { actionsOnly, value } = splitQuery(query);
  return (
    <>
      {actionsOnly && <span className="palette-prefix">&gt;</span>}
      <span className="palette-query" data-empty={(!actionsOnly && value === "") || undefined}>
        {actionsOnly || value !== "" ? value : placeholder}
      </span>
    </>
  );
}

/** Each frame with its caption, as the artboards draw them side by side. */
export function PaletteGallery({ frames }: { frames: readonly PaletteGalleryFrame[] }) {
  return (
    <div className="palette-gallery">
      {frames.map((frame) => (
        <div key={frame.caption} className="palette-gallery-frame">
          <span>{frame.caption}</span>
          <div className="palette-still">
            <PaletteBox
              view={frame.view}
              field={<StillField query={frame.query} placeholder={frame.view.placeholder} />}
              cursor={firstPickable(frame.view.rows)}
            />
          </div>
        </div>
      ))}
    </div>
  );
}
