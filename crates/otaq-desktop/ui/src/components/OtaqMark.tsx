// The Otaq mark, "Command tent" (d0076; *1. Otaq · Command tent*, *15 · Settings → About*): a tent with two board
// columns under the roof and a flag on the ridge. The lines take the text colour (ink on light, near-white on dark);
// the pole runs up to the flag's top edge and the flag starts at the pole's side; the flag is the fixed brand turquoise, not `--st-attn`, so the mark looks the same in every theme.
import { MARK_VIEWBOX, markStrokes } from "./logic";

export function OtaqMark({ size = 64, label }: { size?: number; label?: string }) {
  const { outline, columns, flagShift } = markStrokes(size);
  return (
    <svg
      className="otaq-mark"
      width={size}
      height={size}
      viewBox={`0 0 ${String(MARK_VIEWBOX)} ${String(MARK_VIEWBOX)}`}
      fill="none"
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      <path
        d="M18 106 V70 Q42 60 60 30 Q78 60 102 70 V106 Z"
        stroke="currentColor"
        strokeWidth={outline}
        strokeLinejoin="round"
        strokeLinecap="round"
      />
      <path d="M46 62 V106 M74 62 V106" stroke="currentColor" strokeWidth={columns} strokeLinecap="round" />
      <path d="M60 30 V4" stroke="currentColor" strokeWidth={columns} />
      <path
        className="otaq-mark-flag"
        d="M60 4 H80 Q83 4 81 7 L77 12 L81 17 Q83 20 80 20 H60 Z"
        transform={`translate(${String(flagShift)} 0)`}
      />
    </svg>
  );
}
