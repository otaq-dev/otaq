// The design gallery (T-0217): every foundation and component of *crewboard.pen → 01* on one page, to compare with
// the frames by hand, light and dark, and in any of the 15 themes (the palette switch recolors the page for a look,
// without saving; the theme picker in it saves, as it does in Settings). What comes from Rust (legend, priority, type
// scale, icons, keyboard map, menus) is `gallery.view`; the rest are the frames' sample texts.
import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";

import type { OverlayProps } from "../../app/registry";
import { openOverlay } from "../../app/overlays";
import { EpicCard, stateOfLook, TicketCard } from "../../cards";
import {
  AnchoredMenu,
  Banner,
  Button,
  type ButtonVariant,
  Checkbox,
  CountBadge,
  DecisionItem,
  DecisionsList,
  Field,
  IconButton,
  Kbd,
  MetaChip,
  OtaqMark,
  pickTheme,
  Popover,
  PriorityChip,
  PriorityLabel,
  Radio,
  Segmented,
  Select,
  type ShownState,
  StackList,
  StackRow,
  StatusPill,
  Tabs,
  TextArea,
  TextInput,
  ThemePicker,
  Toast,
  Toggle,
  Tooltip,
  useThemePicker,
} from "../../components";
import type { CardGallery } from "../../generated/CardGallery";
import type { DecisionGallery } from "../../generated/DecisionGallery";
import type { GalleryView } from "../../generated/GalleryView";
import type { IconName } from "../../generated/IconName";
import type { KeyMapGroup } from "../../generated/KeyMapGroup";
import type { LegendTier } from "../../generated/LegendTier";
import type { PillView } from "../../generated/PillView";
import {
  MarkdownEditor,
  MarkdownSurface,
  MarkdownToolbar,
  SourceArea,
  SourceToggle,
  useFocusOnLeave,
  useMarkdownEditor,
  useSourceMode,
} from "../../markdown";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { THEME_KEYS, type ThemeKey } from "../../theme/generated/tokens";
import { TokenSwatches } from "../../theme/Swatches";
import "./gallery.css";

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="gallery-section" aria-label={title}>
      <h3>{title}</h3>
      {children}
    </section>
  );
}

function Caption({ children }: { children: ReactNode }) {
  return <span className="gallery-caption">{children}</span>;
}

/** The palette switch: recolors the whole page for a look (not saved); closing the gallery puts the theme back. */
function PaletteSwitch({ shown, onShow }: { shown: string; onShow: (key: ThemeKey) => void }) {
  return (
    <div className="gallery-palettes" role="radiogroup" aria-label="Palette">
      {THEME_KEYS.map((key) => (
        <button
          key={key}
          type="button"
          role="radio"
          aria-checked={key === shown}
          className="gallery-palette"
          onClick={() => {
            onShow(key);
          }}
        >
          {key}
        </button>
      ))}
    </div>
  );
}

function Legend({ tiers, view }: { tiers: readonly LegendTier[]; view: GalleryView }) {
  return (
    <div className="gallery-stack">
      {tiers.map((tier) => (
        <div key={tier.title} className="gallery-stack-tight">
          <h4>{tier.title}</h4>
          <div className="gallery-wrap">
            {tier.pills.map((pill) => (
              <StatusPill key={pill.label} pill={pill} />
            ))}
          </div>
          <div className="gallery-wrap">
            <span className="gallery-tag">M12</span>
            {tier.m12.map((pill) => (
              <StatusPill key={pill.label} pill={pill} />
            ))}
          </div>
        </div>
      ))}
      <div className="gallery-stack-tight">
        <div className="gallery-wrap">
          <h4>{view.priority.title}</h4>
          <span className="gallery-note">{view.priority.note}</span>
        </div>
        <div className="gallery-wrap gallery-wrap-wide">
          {view.priority.chips.map((chip) => (
            <PriorityLabel key={chip.short} priority={chip} />
          ))}
          <span className="gallery-secondary">{view.priority.order}</span>
        </div>
      </div>
    </div>
  );
}

function TypeScale({ view }: { view: GalleryView }) {
  return (
    <div className="gallery-stack">
      {view.type_scale.map((row) => (
        <div key={row.token} className="gallery-type-row">
          <span className="gallery-type-name">
            <span>{row.name}</span>
            <Caption>{row.spec}</Caption>
          </span>
          <span
            style={{ font: `var(--text-${row.token})`, letterSpacing: `var(--text-${row.token}-tracking)` }}
          >
            {row.sample}
          </span>
        </div>
      ))}
    </div>
  );
}

function Spacing({ view }: { view: GalleryView }) {
  return (
    <div className="gallery-stack">
      <div className="gallery-measures">
        {view.spacing.map((step) => (
          <span key={step.name} className="gallery-measure">
            <span className="gallery-space" style={{ width: step.px, height: step.px }} />
            <Caption>{step.name}</Caption>
            <Caption>{step.px} px</Caption>
          </span>
        ))}
      </div>
      <div className="gallery-measures">
        {view.radii.map((r) => (
          <span key={r.name} className="gallery-measure">
            <span className="gallery-radius" style={{ borderRadius: r.px }} />
            <Caption>
              {r.name} {r.px}
            </Caption>
          </span>
        ))}
      </div>
      <Caption>{view.density}</Caption>
    </div>
  );
}

function Icons({ view }: { view: GalleryView }) {
  return (
    <div className="gallery-icons">
      {view.icons.map((entry) => (
        <span key={entry.name} className="gallery-icon">
          <Icon name={entry.icon} />
          <Caption>{entry.name}</Caption>
        </span>
      ))}
    </div>
  );
}

const CELLS: readonly { name: string; shown?: ShownState; disabled?: boolean; loading?: boolean }[] = [
  { name: "DEFAULT" },
  { name: "HOVER", shown: "hover" },
  { name: "PRESSED", shown: "pressed" },
  { name: "FOCUSED", shown: "focused" },
  { name: "DISABLED", disabled: true },
  { name: "LOADING", loading: true },
];

const VARIANTS: readonly {
  variant: ButtonVariant;
  name: string;
  label: string;
  busy: string;
  icon: IconName;
}[] = [
  { variant: "primary", name: "Primary", label: "Approve", busy: "Merging…", icon: "check" },
  { variant: "secondary", name: "Secondary", label: "Send back", busy: "Working…", icon: "undo-2" },
  { variant: "destructive", name: "Destructive", label: "Mark closed", busy: "Working…", icon: "x" },
  { variant: "ghost", name: "Ghost", label: "View parent", busy: "Working…", icon: "arrow-up-right" },
];

function ButtonMatrix() {
  const [loading, setLoading] = useState(false);
  const [runs, setRuns] = useState(0);
  return (
    <div className="gallery-stack">
      <div className="gallery-matrix">
        <span />
        {CELLS.map((cell) => (
          <Caption key={cell.name}>{cell.name}</Caption>
        ))}
        {VARIANTS.map((v) => (
          <MatrixRow key={v.variant} name={v.name}>
            {CELLS.map((cell) => (
              <Button
                key={cell.name}
                variant={v.variant}
                icon={v.icon}
                label={cell.loading ? v.busy : v.label}
                shown={cell.shown}
                disabled={cell.disabled}
                loading={cell.loading}
              />
            ))}
          </MatrixRow>
        ))}
        <MatrixRow name="Icon button">
          {CELLS.map((cell) => (
            <IconButton
              key={cell.name}
              icon="square-terminal"
              label="Open terminal"
              shown={cell.shown}
              disabled={cell.disabled}
              loading={cell.loading}
            />
          ))}
        </MatrixRow>
      </div>
      <div className="gallery-wrap">
        <Button
          label={loading ? "Merging…" : "Approve"}
          icon="check"
          loading={loading}
          onClick={() => {
            setLoading(true);
            setRuns((n) => n + 1);
            window.setTimeout(() => {
              setLoading(false);
            }, 2000);
          }}
        />
        <Caption>Live: a press loads for 2 s; presses meanwhile are ignored · runs: {runs}</Caption>
      </div>
    </div>
  );
}

function MatrixRow({ name, children }: { name: string; children: ReactNode }) {
  return (
    <>
      <span className="gallery-secondary">{name}</span>
      {children}
    </>
  );
}

function Primitives({ view }: { view: GalleryView }) {
  const sample = (label: string): PillView | undefined =>
    view.legend.flatMap((t) => [...t.pills, ...t.m12]).find((p) => p.label === label);
  const working = sample("Agent Working");
  return (
    <div className="gallery-stack">
      <div className="gallery-wrap gallery-wrap-wide">
        {working && <StatusPill pill={working} />}
        <MetaChip icon="link-2" label="SHOP-18 SHOP-19" />
        <Kbd keys="⌘K" />
        <Button label="Approve" icon="check" />
        <IconButton icon="square-terminal" label="Open terminal" />
        {view.priority.chips[1] && <PriorityChip priority={view.priority.chips[1]} />}
      </div>
      <Caption>COUNT</Caption>
      <div className="gallery-wrap">
        <CountBadge count={3} tone="attention" />
        <CountBadge count={12} tone="neutral" />
        <CountBadge count={2} tone="accent" />
      </div>
      <Caption>DEPENDENCY · META</Caption>
      <div className="gallery-wrap">
        <MetaChip icon="link-2" label="SHOP-18 SHOP-19" />
        <MetaChip icon="git-branch" label="otaq/SHOP-18" />
        <MetaChip icon="coins" label="182k" />
        <MetaChip icon="play" label=":4020" tone="running" />
        <MetaChip icon="git-pull-request" label="PR #481" />
      </div>
      <Caption>LAST SIGNAL</Caption>
      <div className="gallery-wrap">
        <MetaChip icon="activity" label="40 s ago" />
        <MetaChip icon="activity" label="4 min ago" tone="warning" />
      </div>
      <Caption>KBD</Caption>
      <div className="gallery-wrap">
        {["⌘K", "⌘↵", "⇧⌘I", "Ctrl+Shift+J", "esc"].map((k) => (
          <Kbd key={k} keys={k} />
        ))}
      </div>
    </div>
  );
}

function TabsAndToggles() {
  const [tab, setTab] = useState(0);
  const [narrow, setNarrow] = useState(1);
  const [segment, setSegment] = useState(0);
  const [on, setOn] = useState(true);
  const [off, setOff] = useState(false);
  const tabs = [
    { icon: "square-terminal", label: "Terminal", indicator: { kind: "live" }, keys: "⌘⌥1" },
    { icon: "file-diff", label: "Diff", indicator: { kind: "note", text: "+412 −38" }, keys: "⌘⌥2" },
    { icon: "play", label: "Run", indicator: { kind: "note", text: ":4020" }, keys: "⌘⌥3" },
    { icon: "terminal", label: "Shell", keys: "⌘⌥4" },
    { icon: "file-text", label: "Ticket", keys: "⌘⌥5" },
    { icon: "history", label: "History", indicator: { kind: "live_attention" }, keys: "⌘⌥6" },
  ] as const;
  return (
    <div className="gallery-stack">
      <Caption>TABS</Caption>
      <Tabs label="Panel tabs" items={tabs} active={tab} onSelect={setTab} />
      <Caption>TABS · ICONS ONLY (narrow panel)</Caption>
      <Tabs label="Panel tabs, narrow" items={tabs} active={narrow} onSelect={setNarrow} iconsOnly />
      <Caption>SEGMENTED</Caption>
      <Segmented
        label="Diff layout"
        options={["Unified", "Side-by-side"]}
        selected={segment}
        onSelect={setSegment}
      />
      <Caption>TOGGLES</Caption>
      <div className="gallery-wrap gallery-wrap-wide">
        <Toggle on={on} label={on ? "On" : "Off"} onChange={setOn} />
        <Toggle on={off} label="Off · default" onChange={setOff} />
        <Toggle on={false} label="Disabled" disabled onChange={setOff} />
      </div>
    </div>
  );
}

function Fields({ view }: { view: GalleryView }) {
  const [empty, setEmpty] = useState("");
  const [title, setTitle] = useState("Checkout API");
  const [env, setEnv] = useState(".env.local");
  const [body, setBody] = useState(
    "POST /api/checkout creates a Stripe session. Uses the client from SHOP-17.",
  );
  const [priority, setPriority] = useState("priority-2");
  const [checked, setChecked] = useState(true);
  const [radio, setRadio] = useState("squash");
  const chosen = view.priority_menu.find((i) => i.id === priority);
  const items = view.priority_menu.map((i) => ({ ...i, checked: i.id === priority }));
  return (
    <div className="gallery-fields">
      <Field label="Title">
        {(id) => (
          <TextInput id={id} value={empty} placeholder="Short, imperative title" onChange={setEmpty} />
        )}
      </Field>
      <Field label="Title" hint="Focused · 1.5 px focus ring">
        {(id) => <TextInput id={id} value={title} onChange={setTitle} />}
      </Field>
      <Field label="Env file" error="File not found in repo root: .env.local">
        {(id) => <TextInput id={id} value={env} mono invalid onChange={setEnv} />}
      </Field>
      <Field label="Priority">
        {(id) => (
          <Select
            id={id}
            value={chosen?.label ?? priority}
            icon={chosen?.icon}
            items={items}
            onChange={setPriority}
          />
        )}
      </Field>
      <Field label="PR provider">
        {(id) => (
          <Select
            id={id}
            value="GitHub"
            icon="git-pull-request"
            items={[]}
            disabled
            onChange={() => undefined}
          />
        )}
      </Field>
      <Field label="What and why">{(id) => <TextArea id={id} value={body} onChange={setBody} />}</Field>
      <Field hint="Disabled">
        {() => <TextInput label="Locked" value="Locked after start" disabled onChange={() => undefined} />}
      </Field>
      <Checkbox checked={checked} label="Delete the branch after merging" onChange={setChecked} />
      <div className="gallery-wrap gallery-wrap-wide">
        <Radio
          name="gallery-merge"
          checked={radio === "squash"}
          label="Squash"
          onChange={() => {
            setRadio("squash");
          }}
        />
        <Radio
          name="gallery-merge"
          checked={radio === "merge"}
          label="Merge commit"
          onChange={() => {
            setRadio("merge");
          }}
        />
        <Radio name="gallery-merge" checked={false} label="Rebase" disabled onChange={() => undefined} />
      </div>
    </div>
  );
}

function Overlays({ view }: { view: GalleryView }) {
  const menuButton = useRef<HTMLButtonElement>(null);
  const popoverButton = useRef<HTMLButtonElement>(null);
  const [menu, setMenu] = useState<"button" | { x: number; y: number } | null>(null);
  const [popover, setPopover] = useState(false);
  const [chosen, setChosen] = useState("nothing yet");
  const closePopover = useCallback(() => {
    setPopover(false);
  }, []);
  return (
    <div className="gallery-overlays">
      <div className="gallery-stack">
        <Caption>CONTEXT MENU · ↑/↓, ↵, Esc</Caption>
        <div className="gallery-wrap">
          <Button
            ref={menuButton}
            variant="secondary"
            label="Open menu"
            icon="ellipsis"
            onClick={() => {
              setMenu("button");
            }}
          />
          <button
            type="button"
            className="gallery-target"
            onContextMenu={(e) => {
              e.preventDefault();
              setMenu({ x: e.clientX, y: e.clientY });
            }}
          >
            Right-click here
          </button>
        </div>
        <Caption>Chosen: {chosen}</Caption>
        {menu && (
          <AnchoredMenu
            label="Card actions"
            anchor={menu === "button" ? menuButton : { left: menu.x, top: menu.y, width: 0, height: 0 }}
            items={view.context_menu}
            onChoose={setChosen}
            onClose={(how) => {
              setMenu(null);
              if (how === "key" && menu === "button") menuButton.current?.focus();
            }}
          />
        )}
        <Caption>TOOLTIP · POPOVER</Caption>
        <div className="gallery-wrap">
          <Tooltip text="Open terminal" keys="⌘⌥1">
            <IconButton icon="square-terminal" label="Open terminal" />
          </Tooltip>
          <Tooltip
            text="2 running"
            detail={["SHOP-18 · npm run dev :4020", "SHOP-22 · npm test"]}
            note="Click to manage"
          >
            <Button variant="ghost" label="Hover for details" icon="play" />
          </Tooltip>
          <Button
            ref={popoverButton}
            variant="secondary"
            label="Popover"
            icon="gauge"
            onClick={() => {
              setPopover((p) => !p);
            }}
          />
          {popover && (
            <Popover label="Agent limits" anchor={popoverButton} onClose={closePopover}>
              <div className="gallery-popover">
                <strong>Agent limits</strong>
                <span className="gallery-secondary">Esc or a click outside closes it.</span>
              </div>
            </Popover>
          )}
        </div>
      </div>
      <div className="gallery-stack">
        <Caption>CONFIRM DIALOG · Esc cancels; ↵ = Cancel on destructive dialogs</Caption>
        <div className="gallery-wrap">
          <Button
            variant="secondary"
            label="Primary dialog"
            onClick={() => {
              openOverlay("confirm", {
                title: "Send SHOP-18 Checkout API back?",
                body: "SHOP-18 goes straight back to its agent in Dev (Agent Working) and continues in the same session.",
                confirmLabel: "Send back",
                destructive: false,
                onConfirm: () => {
                  setChosen("dialog: Send back");
                },
              });
            }}
          />
          <Button
            variant="destructive"
            label="Destructive dialog"
            onClick={() => {
              openOverlay("confirm", {
                title: "Delete SHOP-21 Order confirmation email?",
                body: "It never started, so nothing else is lost. Its number is never reused.",
                confirmLabel: "Delete",
                destructive: true,
                onConfirm: () => {
                  setChosen("dialog: Delete");
                },
              });
            }}
          />
        </div>
        <Caption>TOASTS</Caption>
        <div className="gallery-toasts">
          <Toast
            kind="success"
            icon="git-pull-request"
            iconTone="review"
            title="PR #481 opened on GitHub"
            text="SHOP-26 Add sitemap → main · Open in browser"
            onClose={() => undefined}
          />
          <Toast
            kind="success"
            icon="git-merge"
            title="Local merge done · SHOP-18"
            text="Squashed into main as 9f3e2c1"
            onClose={() => undefined}
          />
          <Toast
            kind="success"
            icon="sparkles"
            iconTone="investigate"
            title="Ticket created by agent"
            text="SHOP-29 Add cart analytics → Backlog"
            onClose={() => undefined}
          />
          <Toast
            kind="error"
            title="gh is not logged in"
            text="Run gh auth login, then retry Open PR."
            action={{ label: "Copy command", onClick: () => undefined }}
            onClose={() => undefined}
          />
        </div>
      </div>
    </div>
  );
}

/** The epic queue toast (T-0272, EPC-15): flows 2 (several children), 9 (one child) and 12 (Auto-merge off). */
function EpicQueueToasts() {
  const [pressed, setPressed] = useState<string>();
  const press = (label: string) => ({
    label,
    onClick: () => {
      setPressed(label);
    },
  });
  return (
    <div className="gallery-toasts">
      <Toast
        kind="success"
        title="Queued 2 children of SHOP-45"
        actions={[press("Undo")]}
        onClose={() => undefined}
      />
      <Toast
        kind="success"
        title="Queued SHOP-45.1 of Search"
        actions={[press("Undo")]}
        onClose={() => undefined}
      />
      <Toast
        kind="success"
        title="Queued 3 tickets. Auto-merge is off for shop-web, so each one waits for your review."
        actions={[press("Turn on for epic children"), press("Undo")]}
        onClose={() => undefined}
      />
      <Caption>Last press: {pressed ?? "—"}</Caption>
    </div>
  );
}

function Banners() {
  const noop = () => undefined;
  return (
    <div className="gallery-stack">
      <Banner
        tone="attention"
        icon="gauge"
        title="Usage limit reached"
        text="Resets at 3:00 pm (in 1 h 12 min) · 8 agents stopped · no new tickets will start"
        actions={[
          { label: "Resume all", icon: "play", disabled: true, onClick: noop },
          { label: "Retry", icon: "rotate-ccw", onClick: noop },
        ]}
        onDismiss={noop}
      />
      <Banner
        tone="warning"
        icon="shield-alert"
        title="Permission prompts are on"
        text="With 5–10 agents, permission prompts will stop agents often. Read about allowlists and auto mode →"
        onDismiss={noop}
      />
      <Banner
        tone="info"
        icon="refresh-cw"
        title="Service update ready"
        text="Restart now (3 agents working, 2 waiting) · or later, when it suits you"
        actions={[{ label: "Restart now", icon: "rotate-cw", onClick: noop }]}
        onDismiss={noop}
      />
      <Banner
        tone="warning"
        icon="server-cog"
        title="Restart required"
        text="This app can’t talk to the running service (older version) · 3 agents working, 2 waiting"
        actions={[{ label: "Restart now", icon: "rotate-cw", onClick: noop }]}
      />
      <Banner tone="error" icon="zap-off" title="Background service stopped" text="Agents are not running" />
      <Banner
        tone="neutral"
        icon="loader-circle"
        title="Reconnecting…"
        text="Trying to reach the background service"
      />
    </div>
  );
}

function Stack({ view }: { view: GalleryView }) {
  const find = (label: string) =>
    view.legend.flatMap((t) => [...t.pills, ...t.m12]).find((p) => p.label === label);
  const merged = find("Merged");
  const backlog = find("Backlog");
  const queued = find("Queued");
  const [current, setCurrent] = useState("SHOP-20");
  if (!merged || !backlog || !queued) return null;
  const rows = [
    { key: "SHOP-17", title: "Add Stripe client", pill: merged, depth: 0 },
    { key: "SHOP-18", title: "Checkout API", pill: backlog, depth: 1 },
    { key: "SHOP-20", title: "Checkout page", pill: queued, depth: 2, waits: "Waits for SHOP-18 SHOP-19" },
    { key: "SHOP-21", title: "Order confirmation email", pill: backlog, depth: 3 },
  ];
  return (
    <div className="gallery-two">
      <StackList title="Stack">
        {rows.map((row) => (
          <StackRow
            key={row.key}
            depth={row.depth}
            ticketKey={row.key}
            title={row.title}
            waits={row.waits}
            pill={row.pill}
            current={row.key === current}
            onSelect={() => {
              setCurrent(row.key);
            }}
          />
        ))}
      </StackList>
      <StackList title="Stack" empty="SHOP-12 has no dependencies and nothing depends on it." />
    </div>
  );
}

function KeyColumn({ keys }: { keys: string | null }) {
  return (
    <span className="gallery-keys">
      {keys === null ? <span className="gallery-secondary">—</span> : <Kbd keys={keys} />}
    </span>
  );
}

function KeyboardMap({
  groups,
  columns,
  notes,
}: {
  groups: readonly KeyMapGroup[];
  columns: string;
  notes: readonly string[];
}) {
  return (
    <div className="gallery-stack">
      <div className="gallery-keymap">
        {groups.map((group) => (
          <div key={group.title} className="gallery-keygroup">
            <div className="gallery-keygroup-head">
              <h4>{group.title}</h4>
              <span className="gallery-note">{columns}</span>
            </div>
            {group.rows.map((row) => (
              <div key={row.label} className="gallery-keyrow">
                <span className="gallery-keylabel">{row.label}</span>
                <KeyColumn keys={row.mac} />
                <KeyColumn keys={row.win_linux} />
                <KeyColumn keys={row.in_terminal} />
              </div>
            ))}
          </div>
        ))}
      </div>
      {notes.map((note, i) => (
        <p key={note} className={i === 0 ? "gallery-keynote-first" : "gallery-note"}>
          {note}
        </p>
      ))}
    </div>
  );
}

function ThemeSection() {
  const picker = useThemePicker();
  if (picker.error) return <p role="alert">{picker.error.message}</p>;
  if (!picker.view) return null;
  return (
    <div className="gallery-picker">
      <ThemePicker
        sections={picker.view}
        onPick={(key) => {
          void pickTheme(key);
        }}
      />
    </div>
  );
}

/** *04 · Ticket card variants → Variant grid*: one cell per variant, named as in the frame, in a column-width card. */
function CardGrid({ cells, onPick }: { cells: CardGallery["variants"]; onPick: (what: string) => void }) {
  return (
    <div className="gallery-cards">
      {cells.map((cell) => (
        <div key={cell.name} className="gallery-card-cell">
          <Caption>{cell.name.toUpperCase()}</Caption>
          <TicketCard
            card={cell.card}
            state={stateOfLook(cell.look)}
            onSelect={() => {
              onPick(`${cell.card.key}: selected`);
            }}
            onAction={(kind) => {
              onPick(`${cell.card.key}: ${JSON.stringify(kind)}`);
            }}
            onOpenPanel={() => {
              onPick(`${cell.card.key}: open panel`);
            }}
          />
        </div>
      ))}
    </div>
  );
}

/** *04 → Compact density → Compare*: the same tickets, comfortable at 200 px and compact at 168 px. */
function CompactDensity({ cards }: { cards: CardGallery }) {
  return (
    <div className="gallery-wrap gallery-compare">
      <div className="gallery-card-column" data-density="comfortable">
        <Caption>COMFORTABLE</Caption>
        {cards.compact.map((card) => (
          <TicketCard key={card.key} card={card} />
        ))}
      </div>
      <div className="gallery-card-column" data-density="compact">
        <Caption>COMPACT · 168 px</Caption>
        {cards.compact.map((card) => (
          <TicketCard key={card.key} card={card} density="compact" />
        ))}
      </div>
    </div>
  );
}

function EpicGrid({ cards, onPick }: { cards: CardGallery; onPick: (what: string) => void }) {
  return (
    <div className="gallery-cards">
      {cards.epics.map((cell) => (
        <div key={cell.name} className="gallery-card-cell">
          <Caption>{cell.name.toUpperCase()}</Caption>
          <EpicCard
            card={cell.card}
            onSelect={() => {
              onPick(`${cell.card.key}: selected`);
            }}
            onChild={(key) => {
              onPick(`${key}: selected`);
            }}
            onAdd={() => {
              onPick(`${cell.card.key}: add ticket`);
            }}
            onAction={(kind) => {
              onPick(`${cell.card.key}: ${JSON.stringify(kind)}`);
            }}
          />
        </div>
      ))}
    </div>
  );
}

function Cards() {
  const { view, error } = useServiceView(loadCards, []);
  const [picked, setPicked] = useState<string | null>(null);
  if (error) return <p role="alert">{error.message}</p>;
  if (!view) return null;
  return (
    <>
      <Section title="04 · Ticket card variants · Variant grid (fixtures)">
        <Caption>Last click: {picked ?? "—"}</Caption>
        <CardGrid cells={view.variants} onPick={setPicked} />
      </Section>
      <Section title="04 · Ticket card variants · Compact density">
        <CompactDensity cards={view} />
      </Section>
      <Section title="04 · Ticket card variants · Row · epics (fixtures)">
        <EpicGrid cards={view} onPick={setPicked} />
      </Section>
      <Section title="04 · Ticket card variants · Row · epic children · the sleeve (T-0266, d0080)">
        <CardGrid cells={view.sleeves} onPick={setPicked} />
      </Section>
    </>
  );
}

/** SHOP-47's description, the update P example (tickets/m15-tickets.md, "The example"). */
const SHOP_47 = [
  "## Why",
  "Users mistype addresses at checkout; **12%** of failed deliveries trace back to it (see [the support report](https://example.com/report)).",
  "",
  "## What",
  "- Suggest addresses as the user types in *Street*",
  "- Keyboard: ↑ ↓ to move, Enter to pick, Esc to close",
  "",
  "## Done when",
  "- [x] Suggestions appear after 3 characters",
  "- [ ] Arrow keys move the highlight",
  "- [ ] Works with the existing `AddressForm` validation",
  "",
  "| Field | Source |",
  "|---|---|",
  "| Street | Places API |",
  "| Postcode | Places API, editable |",
  "",
  "```ts",
  'const results = await places.autocomplete(query, { country: "TR" });',
  "```",
].join("\n");

/** The description editor (T-0249) with the example, in edit and read mode; read mode follows the edits. */
function MarkdownSample() {
  const [body, setBody] = useState(SHOP_47);
  const [empty, setEmpty] = useState("");
  return (
    <div className="gallery-two">
      <div className="gallery-stack-tight">
        <Caption>Edit mode · b · Filled · edit mode (the toolbar comes with T-0250)</Caption>
        <MarkdownEditor value={body} onChange={setBody} label="What and why" />
        <Caption>Edit mode · empty, with the placeholder</Caption>
        <MarkdownEditor
          value={empty}
          onChange={setEmpty}
          label="What and why"
          placeholder="Context, constraints, links…"
        />
      </div>
      <div className="gallery-stack-tight">
        <Caption>Read mode · editable=false (task boxes don&apos;t react)</Caption>
        <MarkdownEditor value={body} editable={false} label="What and why" />
      </div>
    </div>
  );
}

/** Code blocks (T-0254): the language label and picker, the Diff tab's colours, a long line, and read mode's Copy. */
const CODE_SAMPLE = [
  "## Sketch",
  "",
  "```ts",
  'const results = await places.autocomplete(query, { country: "TR" }); // a long line scrolls sideways, never wraps',
  "if (!results.length) {",
  "  showNoMatches();",
  "}",
  "```",
  "",
  "````md",
  "```rust",
  "fn main() {}",
  "```",
  "````",
].join("\n");

function CodeSample() {
  const [body, setBody] = useState(CODE_SAMPLE);
  return (
    <div className="gallery-two">
      <div className="gallery-stack-tight">
        <Caption>
          Edit mode · g · the label opens the picker; ``` on an empty last line or ↓ at the end leaves
        </Caption>
        <MarkdownEditor value={body} onChange={setBody} label="What and why" />
      </div>
      <div className="gallery-stack-tight">
        <Caption>Read mode · c · Copy on hover</Caption>
        <MarkdownEditor value={body} editable={false} label="What and why" />
      </div>
    </div>
  );
}

function ToolbarSample() {
  const [body, setBody] = useState(SHOP_47);
  const [narrow, setNarrow] = useState(SHOP_47);
  return (
    <div className="gallery-stack-tight">
      <Caption>Full width · b · Filled · edit mode (the caret in a task item lights Task list)</Caption>
      <MarkdownEditor value={body} onChange={setBody} label="What and why" toolbar={<MarkdownToolbar />} />
      <Caption>
        At 480 px · h · Narrow Ticket tab · toolbar overflow (Strikethrough, Quote, Table, Horizontal rule in
        “…”)
      </Caption>
      <div style={{ width: 480 }}>
        <MarkdownEditor
          value={narrow}
          onChange={setNarrow}
          label="What and why"
          toolbar={<MarkdownToolbar />}
        />
      </div>
    </div>
  );
}

const TABLE_SAMPLE = `## Sources
Where each address field comes from:

| Field | Source |
|---|---|
| Street | Places API |
| Postcode | Places API, editable |
`;

function TableSample() {
  const [body, setBody] = useState(TABLE_SAMPLE);
  return (
    <div className="gallery-stack-tight">
      <Caption>f · Caret in a table: click a cell; the bar floats above the table (edit mode only)</Caption>
      <MarkdownEditor value={body} onChange={setBody} label="What and why" toolbar={<MarkdownToolbar />} />
    </div>
  );
}

/** Links (T-0252): the ⌘K popover and the hover bubble in edit mode; the tooltip and a click to open in read mode. */
const LINK_SAMPLE = [
  "## Why",
  "Users mistype addresses at checkout; **12%** of failed deliveries trace back to it (see [the support report](https://example.com/report)).",
  "",
  "Write to [the team](mailto:checkout@example.com) about it.",
].join("\n");

function LinkSample() {
  const [body, setBody] = useState(LINK_SAMPLE);
  return (
    <div className="gallery-two">
      <div className="gallery-stack-tight">
        <Caption>
          Edit mode · e · select text and press ⌘K, or hover a link (Open opens the system browser)
        </Caption>
        <MarkdownEditor value={body} onChange={setBody} label="What and why" toolbar={<MarkdownToolbar />} />
      </div>
      <div className="gallery-stack-tight">
        <Caption>Read mode · c · hover shows the URL; a click opens it</Caption>
        <MarkdownEditor value={body} editable={false} label="What and why" />
      </div>
    </div>
  );
}

/** The Markdown source toggle (T-0251): opens with the source on; switching here isn't saved. */
function SourceSample() {
  const [body, setBody] = useState(SHOP_47);
  const mode = useSourceMode(body, true);
  const editor = useMarkdownEditor({ value: mode.editorValue, onChange: setBody, label: "What and why" });
  useFocusOnLeave(editor, mode);
  return (
    <div className="gallery-stack-tight">
      <Caption>
        d · Markdown source on: the raw source in Geist Mono; the formatting buttons are disabled
      </Caption>
      <MarkdownSurface
        editor={editor}
        toolbar={
          <MarkdownToolbar
            disabled={mode.on}
            sourceToggle={<SourceToggle on={mode.on} label="Markdown" onChange={mode.toggle} />}
          />
        }
        {...(mode.on
          ? {
              source: (
                <SourceArea value={body} label="What and why" takeFocus={mode.switched} onChange={setBody} />
              ),
            }
          : {})}
      />
    </div>
  );
}

const loadGallery = () => request("gallery.view") as Promise<GalleryView>;
const loadCards = () => request("gallery.cards") as Promise<CardGallery>;
const loadDecisions = () => request("gallery.decisions") as Promise<DecisionGallery>;

/** *07 → Row · Decision Item (0084)* and the SHOP-23 / SHOP-40 review lists (T-0287), from `gallery.decisions`. */
function Decisions() {
  const { view, error } = useServiceView(loadDecisions, []);
  const [picked, setPicked] = useState<string | null>(null);
  if (error) return <p role="alert">{error.message}</p>;
  if (!view) return null;
  const onChange = (item: { id: string }) => {
    setPicked(`Change… ${item.id}`);
  };
  return (
    <>
      <Section title="07 · Decision Item (0084) · every variant (T-0287)">
        <Caption>Last click: {picked ?? "—"}</Caption>
        <div className="gallery-decisions">
          {view.variants.map((v) => (
            <div key={v.name} className="gallery-decision">
              <Caption>{v.name.toUpperCase()}</Caption>
              <DecisionItem item={v.item} onChange={onChange} />
            </div>
          ))}
        </div>
      </Section>
      <Section title="07 · SHOP-23 review · Decisions · SHOP-40 epic review · Decisions (T-0287)">
        <div className="gallery-decisions">
          <DecisionsList section={view.ticket} onChange={onChange} />
          <DecisionsList section={view.epic} onChange={onChange} />
        </div>
      </Section>
    </>
  );
}

/** The gallery over the whole window; Esc or the close button leaves it. */
export function Gallery({ close }: OverlayProps<"gallery">) {
  const { view, error } = useServiceView(loadGallery, []);
  const root = useRef<HTMLDivElement>(null);
  const [shown, setShown] = useState(() => document.documentElement.dataset.theme ?? "");
  // The palette switch recolors the page without saving; leaving the gallery puts the stored theme back.
  useEffect(() => {
    const before = document.documentElement.dataset.theme;
    return () => {
      if (before !== undefined) document.documentElement.dataset.theme = before;
    };
  }, []);
  useEffect(() => {
    const box = root.current;
    if (!box) return;
    box.focus();
    // Esc leaves the gallery, unless it belongs to a menu or popover inside it (a dialog's Esc never gets here).
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || (e.target as Element | null)?.closest(".menu, .popover")) return;
      e.stopPropagation();
      close();
    };
    box.addEventListener("keydown", onKey);
    return () => {
      box.removeEventListener("keydown", onKey);
    };
  }, [close]);
  const show = (key: ThemeKey) => {
    document.documentElement.dataset.theme = key;
    setShown(key);
  };
  return (
    <div ref={root} className="gallery" role="dialog" aria-modal="true" aria-label="Gallery" tabIndex={-1}>
      <header className="gallery-header">
        <h2>Gallery · 01 · Foundations &amp; Components</h2>
        <PaletteSwitch shown={shown} onShow={show} />
        <IconButton icon="x" label="Close the gallery" onClick={close} />
      </header>
      <div className="gallery-body">
        {error && <p role="alert">{error.message}</p>}
        <Section title="Color Tokens · Otaq Light and Dark (01 · Foundations → Color Tokens)">
          <div className="gallery-two">
            <TokenSwatches theme="otaq-light" />
            <TokenSwatches theme="otaq-dark" />
          </div>
        </Section>
        <Section title="08 · Ticket editor · MD Content · component (T-0249, d0078)">
          <MarkdownSample />
        </Section>
        <Section title="08 · Ticket editor · g · Code block · language picker (T-0254, d0078)">
          <CodeSample />
        </Section>
        <Section title="08 · Ticket editor · MD Toolbar · component (T-0250, d0078)">
          <ToolbarSample />
        </Section>
        <Section title="08 · Ticket editor · f · Caret in a table · the table bar (T-0253, d0078)">
          <TableSample />
        </Section>
        <Section title="08 · Ticket editor · e · Link popover and link bubble (T-0252, d0078)">
          <LinkSample />
        </Section>
        <Section title="08 · Ticket editor · d · Markdown source on (T-0251, d0078)">
          <SourceSample />
        </Section>
        <Section title="Mark · Command tent (d0076; 15 · Settings → About)">
          <div className="gallery-wrap gallery-wrap-wide">
            {[64, 40, 32, 20, 16].map((size) => (
              <OtaqMark key={size} size={size} label={`Otaq, ${String(size)} px`} />
            ))}
          </div>
        </Section>
        <Decisions />
        <Section title="Theme picker · 14 → 2 · Theme (a click applies and saves)">
          <ThemeSection />
        </Section>
        {view && (
          <>
            <Section title="Status Legend">
              <Legend tiers={view.legend} view={view} />
            </Section>
            <Section title="Type scale">
              <TypeScale view={view} />
            </Section>
            <Section title="Spacing · radius · density">
              <Spacing view={view} />
            </Section>
            <Section title="Icons · Lucide (ISC), 14 px / 1.5 stroke">
              <Icons view={view} />
            </Section>
            <Section title="Primitives">
              <Primitives view={view} />
            </Section>
            <Section title="Buttons · Button matrix">
              <ButtonMatrix />
            </Section>
            <Section title="Badges & Inputs · Tabs & segmented">
              <TabsAndToggles />
            </Section>
            <Section title="Badges & Inputs · Fields">
              <Fields view={view} />
            </Section>
            <Section title="Overlays & feedback">
              <Overlays view={view} />
            </Section>
            <Section title="Banners">
              <Banners />
            </Section>
            <Section title="Stack list row">
              <Stack view={view} />
            </Section>
            <Section title="Keyboard map">
              <KeyboardMap
                groups={view.keyboard_map}
                columns={view.keyboard_columns}
                notes={view.keyboard_notes}
              />
            </Section>
          </>
        )}
        <Section title="03 · Board & columns · epic flows 2, 9, 12 · the queue toast (T-0272, d0080)">
          <EpicQueueToasts />
        </Section>
        <Cards />
      </div>
    </div>
  );
}
