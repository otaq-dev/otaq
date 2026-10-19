// The live demo's frame: the 1280 × 800 board scaled to the page width, as the canvas's script does
// (Main.dc.html: gutter 16 px below 760 px wide, else 32; at most 1280 px; view-only below 900 px).

export const DEMO_WIDTH = 1280;
export const DEMO_HEIGHT = 800;

export interface DemoFrame {
  /** The frame's width and height in px, rounded. */
  width: number;
  height: number;
  /** The board's scale. */
  scale: number;
  /** Too narrow to click around: the board ignores the pointer and the note shows. */
  viewOnly: boolean;
}

export function demoFrame(viewportWidth: number): DemoFrame {
  const gutter = viewportWidth < 760 ? 16 : 32;
  const available = Math.max(280, Math.min(DEMO_WIDTH, viewportWidth - gutter * 2));
  const scale = available / DEMO_WIDTH;
  return {
    width: Math.round(available),
    height: Math.round(DEMO_HEIGHT * scale),
    scale,
    viewOnly: viewportWidth < 900,
  };
}
