// The stage strip above a long flow's board that doesn't fit (FLW-12; *Row · A flow with many stages*): every column
// with its count, the stages folded into it underneath ("Dev 1" over "Validate 1"), chevrons between them. The
// columns in view are outlined; a click scrolls the board to one. It stays put while the board scrolls sideways.
// What it says is `BoardStageStrip`'s; which columns are in view is measured here (`columnsInView`).
import { Fragment } from "react";

import type { BoardColumnId } from "../../generated/BoardColumnId";
import type { BoardStageStrip } from "../../generated/BoardStageStrip";
import { Icon } from "../../theme/Icon";

/** A column's left and right edge, in the same coordinates as the view. */
export interface ColumnSpan {
  id: BoardColumnId;
  left: number;
  right: number;
}

/** The columns at least half in view (a sliver at the edge isn't "in view"). */
export function columnsInView(
  columns: readonly ColumnSpan[],
  view: { left: number; right: number },
): Set<string> {
  return new Set(
    columns
      .filter((c) => {
        const shown = Math.min(c.right, view.right) - Math.max(c.left, view.left);
        return shown > 0 && shown >= (c.right - c.left) / 2;
      })
      .map((c) => c.id),
  );
}

export interface StageStripProps {
  strip: BoardStageStrip;
  inView: ReadonlySet<string>;
  onPick: (column: BoardColumnId) => void;
}

export function StageStrip({ strip, inView, onPick }: StageStripProps) {
  return (
    <nav className="stage-strip" aria-label={strip.label}>
      {strip.columns.map((c, i) => (
        <Fragment key={c.column}>
          {i > 0 && (
            <span className="stage-strip-chevron" aria-hidden>
              <Icon name="chevron-right" size={11} />
            </span>
          )}
          <button
            type="button"
            className="stage-strip-item"
            data-column={c.column}
            data-in-view={inView.has(c.column) || undefined}
            aria-current={inView.has(c.column) || undefined}
            onClick={() => {
              onPick(c.column);
            }}
          >
            <span className="stage-strip-line">
              <span className="stage-strip-title">{c.title}</span>
              <span className="stage-strip-count">{c.count}</span>
            </span>
            {c.folded.map((f) => (
              <span key={f.name} className="stage-strip-line stage-strip-folded">
                <span className="stage-strip-folded-name">{f.name}</span>
                <span className="stage-strip-count">{f.count}</span>
              </span>
            ))}
          </button>
        </Fragment>
      ))}
    </nav>
  );
}
