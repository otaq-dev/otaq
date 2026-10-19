import type { CSSProperties } from "react";

/** One of the demo's `d-*` icons. Decorative: screen readers skip it (WEB-06). */
export function Ic({ name, cls = "", style }: { name: string; cls?: string; style?: CSSProperties }) {
  return (
    <svg className={("ic " + cls).trim()} style={style} aria-hidden="true" focusable="false">
      <use href={"#d-" + name}></use>
    </svg>
  );
}

/** A 10 px icon, as drawn for priorities and epic chips. */
export const S10: CSSProperties = { width: 10, height: 10 };
