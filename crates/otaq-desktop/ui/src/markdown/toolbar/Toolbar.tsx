// The description editor's toolbar (T-0250, d0078 D8), drawn from *crewboard.pen → 08 · Ticket editor → MD Toolbar ·
// component*: the Heading menu, the buttons of `buttons.ts` with their active states, "…" for the buttons that don't
// fit, and two slots at the right end, Expand (T-0260) and the Markdown toggle (T-0251), which never move. It goes in
// the `toolbar` slot of `MarkdownSurface` / `MarkdownEditor` and reaches the editor with `useCurrentEditor`.
//
// The buttons aren't in the tab order (Annotations): they're for the mouse, and each keeps the editor's focus when
// pressed. The doc only changes when one is clicked.
import type { Editor } from "@tiptap/core";
import { useCurrentEditor, useEditorState } from "@tiptap/react";
import { Fragment, type ReactNode, type Ref, useCallback, useLayoutEffect, useRef, useState } from "react";

import { keymap } from "../../app/keyboard";
import { useStore } from "../../app/store";
import { AnchoredMenu, type MenuClose, Tooltip } from "../../components";
import type { IconName } from "../../generated/IconName";
import type { MenuItem } from "../../generated/MenuItem";
import { Icon } from "../../theme/Icon";
import { TOOLBAR_BUTTONS, type ToolbarButton } from "./buttons";
import { overflowed, type OverflowItem } from "./overflow";
import { shortcutLabel } from "./shortcuts";
import "./toolbar.css";

/** The gap between toolbar items, in px (toolbar.css). */
const GAP = 1;
/** "…" and its gap. */
const MORE_WIDTH = 24 + GAP;

export interface MarkdownToolbarProps {
  /** The Expand button (T-0260). Until it's passed, an inert placeholder holds its place. */
  expand?: ReactNode;
  /** The Markdown source toggle (T-0251). Until it's passed, an inert placeholder holds its place. */
  sourceToggle?: ReactNode;
  /** The formatting buttons are drawn faint and do nothing (the Markdown source shows, T-0251); the slots still work. */
  disabled?: boolean;
}

/** Whether shortcuts are printed the macOS way: the keymap's platform, or the browser's guess before it arrived. */
function useMac(): boolean {
  const map = useStore(keymap);
  return map ? map.platform === "mac" : navigator.userAgent.includes("Mac");
}

const menuItem = (id: string, label: string, extra: Partial<MenuItem> = {}): MenuItem => ({
  id,
  label,
  icon: null,
  shortcut: null,
  danger: false,
  disabled: false,
  hint: null,
  separated: false,
  checked: false,
  header: false,
  ...extra,
});

/** The block at the caret: 0 for text, else the heading level. */
function headingLevel(editor: Editor): number {
  for (const level of [1, 2, 3, 4, 5, 6]) if (editor.isActive("heading", { level })) return level;
  return 0;
}

/** What the toolbar shows, read again after each transaction (the editor instance doesn't rerender by itself). */
function useToolbarState(editor: Editor) {
  return useEditorState({
    editor,
    selector: ({ editor: e }) => ({
      heading: headingLevel(e),
      active: TOOLBAR_BUTTONS.filter((b) => b.isActive?.(e) === true)
        .map((b) => b.id)
        .join(" "),
    }),
  });
}

interface ToolButtonProps {
  label: string;
  icon: IconName;
  keys?: string | undefined;
  active?: boolean;
  pressed?: boolean;
  inert?: boolean;
  onPress: () => void;
  buttonRef?: Ref<HTMLButtonElement>;
}

/** A 24 × 26 toolbar button with its tooltip; out of the tab order, and pressing it leaves the caret where it is. */
function ToolButton({
  label,
  icon,
  keys,
  active = false,
  pressed = false,
  inert = false,
  onPress,
  buttonRef,
}: ToolButtonProps) {
  return (
    <Tooltip text={label} keys={keys}>
      <button
        ref={buttonRef}
        type="button"
        tabIndex={-1}
        className="md-tool"
        aria-label={label}
        aria-pressed={inert ? undefined : active}
        aria-disabled={inert || undefined}
        aria-expanded={pressed || undefined}
        data-active={active || undefined}
        data-pressed={pressed || undefined}
        onMouseDown={(e) => {
          e.preventDefault();
        }}
        onClick={inert ? undefined : onPress}
      >
        <Icon name={icon} size={15} />
      </button>
    </Tooltip>
  );
}

const HEADING_LABELS = ["Text", "H1", "H2", "H3", "H4", "H5", "H6"];

function HeadingMenu({ editor, level, disabled }: { editor: Editor; level: number; disabled: boolean }) {
  const anchor = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const items = [
    menuItem("0", "Text", { checked: level === 0 }),
    ...[1, 2, 3].map((n) => menuItem(String(n), `Heading ${String(n)}`, { checked: level === n })),
  ];
  const choose = (id: string) => {
    const n = Number(id);
    if (n === 0) editor.chain().focus().setParagraph().run();
    else
      editor
        .chain()
        .focus()
        .setHeading({ level: n as 1 | 2 | 3 })
        .run();
  };
  const close = (how: MenuClose) => {
    setOpen(false);
    if (how === "key") editor.commands.focus();
  };
  return (
    <>
      <Tooltip text="Heading">
        <button
          ref={anchor}
          type="button"
          tabIndex={-1}
          className="md-tool md-heading"
          aria-label="Heading"
          aria-haspopup="menu"
          aria-expanded={open}
          aria-disabled={disabled || undefined}
          data-pressed={open || undefined}
          onMouseDown={(e) => {
            e.preventDefault();
          }}
          onClick={
            disabled
              ? undefined
              : () => {
                  setOpen((o) => !o);
                }
          }
        >
          <span>{HEADING_LABELS[level] ?? "Text"}</span>
          <Icon name="chevron-down" size={12} />
        </button>
      </Tooltip>
      {open && !disabled && (
        <AnchoredMenu
          anchor={anchor}
          label="Heading"
          items={items}
          width={160}
          onChoose={choose}
          onClose={close}
          list
        />
      )}
    </>
  );
}

function MoreMenu({
  editor,
  buttons,
  mac,
  disabled,
}: {
  editor: Editor;
  buttons: readonly ToolbarButton[];
  mac: boolean;
  disabled: boolean;
}) {
  const anchor = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const items = buttons.map((b) =>
    menuItem(b.id, b.label, { icon: b.icon, shortcut: b.keys ? shortcutLabel(b.keys, mac) : null }),
  );
  return (
    <>
      <ToolButton
        label="More"
        icon="ellipsis"
        pressed={open}
        inert={disabled}
        buttonRef={anchor}
        onPress={() => {
          setOpen((o) => !o);
        }}
      />
      {open && !disabled && (
        <AnchoredMenu
          anchor={anchor}
          align="end"
          label="More formatting"
          items={items}
          width={200}
          onChoose={(id) => {
            buttons.find((b) => b.id === id)?.run(editor);
          }}
          onClose={(how) => {
            setOpen(false);
            if (how === "key") editor.commands.focus();
          }}
        />
      )}
    </>
  );
}

/** The placeholders T-0260 and T-0251 replace: drawn as designed, but they do nothing yet. */
function ExpandPlaceholder() {
  return <ToolButton label="Expand" icon="maximize" inert onPress={() => undefined} />;
}

function SourceTogglePlaceholder() {
  return (
    <span className="toggle md-source-toggle" aria-hidden>
      <span className="toggle-track">
        <span className="toggle-knob" />
      </span>
      <span>Markdown</span>
    </span>
  );
}

/** The toolbar of the editor around it. */
export function MarkdownToolbar({ expand, sourceToggle, disabled = false }: MarkdownToolbarProps) {
  const { editor } = useCurrentEditor();
  if (editor === null) return null;
  return <ToolbarFor editor={editor} expand={expand} sourceToggle={sourceToggle} disabled={disabled} />;
}

function ToolbarFor({
  editor,
  expand,
  sourceToggle,
  disabled = false,
}: { editor: Editor } & MarkdownToolbarProps) {
  const state = useToolbarState(editor);
  const active = new Set(disabled ? [] : state.active.split(" "));
  const mac = useMac();
  const left = useRef<HTMLDivElement>(null);
  const widths = useRef(new Map<string, number>());
  const [hidden, setHidden] = useState<readonly string[]>([]);

  // Measures what is drawn (a moved button keeps its last width) and works out what goes into "…".
  const fit = useCallback(() => {
    const box = left.current;
    if (!box) return;
    for (const el of box.querySelectorAll<HTMLElement>("[data-item]")) {
      const id = el.dataset.item;
      if (id !== undefined) widths.current.set(id, el.offsetWidth + GAP);
    }
    const items: OverflowItem[] = [...box.querySelectorAll<HTMLElement>("[data-item]")]
      .map((el) => el.dataset.item ?? "")
      .concat(hiddenIds(widths.current, box))
      .map((id) => ({
        id,
        width: widths.current.get(id) ?? 0,
        overflow: TOOLBAR_BUTTONS.find((b) => b.id === id)?.overflow,
      }));
    const next = overflowed(items, box.clientWidth, MORE_WIDTH);
    setHidden((current) => (current.join(" ") === next.join(" ") ? current : next));
  }, []);

  useLayoutEffect(() => {
    const box = left.current;
    if (!box) return;
    const observer = new ResizeObserver(fit);
    observer.observe(box);
    return () => {
      observer.disconnect();
    };
  }, [fit]);
  // Measured again after each move, so a button that comes back out of "…" brings its real width.
  useLayoutEffect(fit, [fit, hidden]);

  const shown = TOOLBAR_BUTTONS.filter((b) => !hidden.includes(b.id));
  const moved = hidden.flatMap((id) => TOOLBAR_BUTTONS.filter((b) => b.id === id));
  return (
    <div className="md-toolbar" role="toolbar" aria-label="Formatting" data-disabled={disabled || undefined}>
      <div ref={left} className="md-toolbar-tools">
        <span className="md-toolbar-fit">
          <span className="md-toolbar-item" data-item="heading">
            <HeadingMenu editor={editor} level={state.heading} disabled={disabled} />
          </span>
          {shown.map((b, i) => (
            <Fragment key={b.id}>
              {(i === 0 || shown[i - 1]?.group !== b.group) && (
                <span className="md-toolbar-sep" data-item={`sep-${b.group}`} aria-hidden />
              )}
              <span className="md-toolbar-item" data-item={b.id}>
                <ToolButton
                  label={b.label}
                  icon={b.icon}
                  keys={b.keys ? shortcutLabel(b.keys, mac) : undefined}
                  active={active.has(b.id)}
                  inert={disabled}
                  onPress={() => {
                    b.run(editor);
                  }}
                />
              </span>
            </Fragment>
          ))}
        </span>
        {moved.length > 0 && <MoreMenu editor={editor} buttons={moved} mac={mac} disabled={disabled} />}
      </div>
      <div className="md-toolbar-end">
        {expand ?? <ExpandPlaceholder />}
        {sourceToggle ?? <SourceTogglePlaceholder />}
      </div>
    </div>
  );
}

/** The measured items that aren't drawn right now (they're in "…"). */
function hiddenIds(widths: ReadonlyMap<string, number>, box: HTMLElement): string[] {
  return [...widths.keys()].filter((id) => box.querySelector(`[data-item="${id}"]`) === null);
}
