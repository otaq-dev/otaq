// Files dropped on the window (d0099, T-0388; the door's `files_dropped` event): over a terminal that takes typing
// the paths are pasted into it, as in Terminal or iTerm; over one that doesn't nothing happens; anywhere else the drop
// goes on as before (Add project). Every terminal on screen registers its element here, so the one under the pointer
// is found without its host passing anything down. The sheet's Overview registers too and takes the files themselves
// (RPT-02, T-0465: the comment box opens with the first as its chip).

/** What a terminal (or Overview) offers a drop. */
export interface DropTarget {
  /** Not read-only, and its session is live. */
  takesTyping(): boolean;
  /** Pastes the text as ⌘V does, then gives the terminal the keyboard. */
  paste(text: string): void;
  /** Takes the dropped files' paths instead of a paste (Overview's Attachments). */
  takeFiles?: (paths: readonly string[]) => void;
}

export class DropTargets {
  private readonly targets = new Map<Element, DropTarget>();

  /** Registers a terminal's element; the returned function removes it (on unmount). */
  register(element: Element, target: DropTarget): () => void {
    this.targets.set(element, target);
    return () => {
      if (this.targets.get(element) === target) this.targets.delete(element);
    };
  }

  /** The terminal the element belongs to: the element itself or its nearest registered ancestor. */
  at(element: Element | null): DropTarget | undefined {
    for (let e = element; e; e = e.parentElement) {
      const target = this.targets.get(e);
      if (target) return target;
    }
    return undefined;
  }
}

export const dropTargets = new DropTargets();

export type DropRoute = "paste" | "files" | "ignore" | "elsewhere";

/**
 * Where a drop goes: into the terminal under it, to the place that takes files, nowhere (a terminal that doesn't take
 * typing), or as before.
 */
export function routeDrop(target: DropTarget | undefined): DropRoute {
  if (!target) return "elsewhere";
  if (target.takeFiles) return "files";
  return target.takesTyping() ? "paste" : "ignore";
}

export interface FilesDropped {
  paths: string[];
  /** The paths quoted as Terminal.app does, space-separated, with a trailing space (Rust's `drops::paste_text`). */
  paste: string;
  /** CSS pixels from the page's top left. */
  x: number;
  y: number;
}

/** Routes one drop; `elsewhere` gets the first path when it didn't land on a terminal. */
export function filesDropped(
  drop: FilesDropped,
  elsewhere: (path: string) => void,
  targets: DropTargets = dropTargets,
  elementAt: (x: number, y: number) => Element | null = (x, y) => document.elementFromPoint(x, y),
): DropRoute {
  const target = targets.at(elementAt(drop.x, drop.y));
  const route = routeDrop(target);
  if (route === "paste") target?.paste(drop.paste);
  else if (route === "files") target?.takeFiles?.(drop.paths);
  else if (route === "elsewhere" && drop.paths[0] !== undefined) elsewhere(drop.paths[0]);
  return route;
}
