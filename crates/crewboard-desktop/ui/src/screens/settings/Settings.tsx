// Settings (⌘,; *15 · Settings*): one modal window, 1040 × 720, a section nav on the left (the global sections, then
// every project) and one page on the right. The global page scrolls through all its sections and the nav follows
// the one in view; a project has its own page. The nav, the page and every row come from `settings.view`; an edit
// answers with the fresh view. Esc or × closes it.
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import { openOverlay } from "../../app/overlays";
import { type OverlayProps, useRegistries } from "../../app/registry";
import { IconButton } from "../../components";
import type { RemoveProjectDialog } from "../../generated/RemoveProjectDialog";
import type { SettingsButtonAction } from "../../generated/SettingsButtonAction";
import type { SettingsNavRow } from "../../generated/SettingsNavRow";
import type { SettingsValue } from "../../generated/SettingsValue";
import type { SettingsView } from "../../generated/SettingsView";
import type { ThemeCreditsDialog } from "../../generated/ThemeCreditsDialog";
import type { RequestError } from "../../generated/RequestError";
import { asRequestError, watchView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { SettingsContext, type SettingsContextValue, sectionInView } from "./context";
import { CreditsDialog, RemoveDialog } from "./Dialogs";

type Open =
  { kind: "remove"; dialog: RemoveProjectDialog } | { kind: "credits"; dialog: ThemeCreditsDialog } | null;

function NavItem({ row, on, onClick }: { row: SettingsNavRow; on: boolean; onClick: () => void }) {
  return (
    <button type="button" className="settings-nav-item" aria-current={on || undefined} onClick={onClick}>
      <Icon name={row.icon} />
      <span className="settings-nav-label">{row.label}</span>
      {row.alert && (
        <span className="settings-nav-alert">
          <Icon name="circle-alert" size={13} />
        </span>
      )}
    </button>
  );
}

export function Settings({ params, close }: OverlayProps<"settings">) {
  const app = useRegistries();
  const [project, setProject] = useState<string | null>(params?.project ?? null);
  const [view, setView] = useState<SettingsView | null>(null);
  const [error, setError] = useState<RequestError | null>(null);
  const [dialog, setDialog] = useState<Open>(null);
  const [inView, setInView] = useState(0);
  const body = useRef<HTMLDivElement>(null);
  const root = useRef<HTMLDivElement>(null);
  // A click on a global nav row holds that row on until the wheel moves the page.
  const clicked = useRef(false);
  const pendingSection = useRef<string | null>(params?.section ?? null);
  const held = useRef<string | null>(null);

  // The view, again on every settings or project change (a theme picked elsewhere, a project added).
  const load = useCallback(() => request("settings.view", { project }) as Promise<SettingsView>, [project]);
  useEffect(
    () =>
      watchView(load, ["settings_changed", "projects_changed"], (state) => {
        if (state.view) setView(state.view);
        setError(state.error ?? null);
      }),
    [load],
  );

  const edit = useCallback(
    (field: string, value: SettingsValue) => {
      request("settings.edit", { project, field, value }).then(
        (fresh) => {
          setView(fresh as SettingsView);
          setError(null);
        },
        (e: unknown) => {
          setError(asRequestError(e));
        },
      );
    },
    [project],
  );

  const act = useCallback(
    (action: SettingsButtonAction) => {
      switch (action) {
        case "cleanup":
          close();
          openOverlay("cleanup");
          return;
        case "credits":
          void request("settings.credits").then((d) => {
            setDialog({ kind: "credits", dialog: d as ThemeCreditsDialog });
          });
          return;
        case "remove_project":
          if (project === null) return;
          void request("settings.remove_preview", { project }).then(
            (d) => {
              setDialog({ kind: "remove", dialog: d as RemoveProjectDialog });
            },
            (e: unknown) => {
              setError(asRequestError(e));
            },
          );
          return;
      }
    },
    [close, project],
  );

  const page = view?.page;
  const context = useMemo<SettingsContextValue | null>(
    () => (page ? { page, project, edit, act } : null),
    [page, project, edit, act],
  );

  // The sections this page has that a screen registered, in the page's order.
  const sections = useMemo(() => {
    if (!page) return [];
    if (page.page === "global")
      return page.sections.flatMap((s) => {
        const entry = app.settings.global.get(s.id);
        return entry ? [{ id: s.id, Component: entry.Component }] : [];
      });
    return page.sections.flatMap((s) => {
      const entry = app.settings.project.get(s.id);
      return entry ? [{ id: s.id, Component: entry.Component }] : [];
    });
  }, [page, app]);

  const sectionElements = () => [
    ...(body.current?.querySelectorAll<HTMLElement>(":scope > .settings-section-slot") ?? []),
  ];

  // Puts a section at the top of the global page and keeps it there while the parts that load later (the theme
  // picker, the tools) grow the page, until the wheel moves it.
  const scrollTo = useCallback(
    (id: string) => {
      const i = sections.findIndex((s) => s.id === id);
      const element = sectionElements()[i];
      if (!element || !body.current) return false;
      body.current.scrollTop = element.offsetTop;
      held.current = id;
      clicked.current = true;
      setInView(i);
      return true;
    },
    [sections],
  );

  // Opened on a section, or a global nav row clicked from a project page: scroll once the page is drawn.
  useLayoutEffect(() => {
    const wanted = pendingSection.current;
    if (wanted === null || !page || page.page !== "global") return;
    if (scrollTo(wanted)) pendingSection.current = null;
  }, [page, scrollTo]);

  useEffect(() => {
    const box = body.current;
    if (!box) return;
    const observer = new ResizeObserver(() => {
      if (held.current !== null) scrollTo(held.current);
    });
    for (const element of sectionElements()) observer.observe(element);
    return () => {
      observer.disconnect();
    };
  }, [scrollTo]);

  const onScroll = () => {
    const box = body.current;
    if (!box || clicked.current || page?.page !== "global") return;
    const atBottom = box.scrollTop + box.clientHeight >= box.scrollHeight - 1;
    setInView(
      sectionInView(
        sectionElements().map((e) => e.offsetTop),
        box.scrollTop,
        atBottom,
      ),
    );
  };

  // Esc closes Settings, unless a menu or a dialog inside it takes it.
  useEffect(() => {
    const box = root.current;
    if (!box) return;
    box.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || (e.target as Element | null)?.closest(".menu, .popover, .dialog")) return;
      e.stopPropagation();
      close();
    };
    box.addEventListener("keydown", onKey);
    return () => {
      box.removeEventListener("keydown", onKey);
    };
  }, [close]);

  const go = (row: SettingsNavRow) => {
    if (row.target.page === "project") {
      setProject(row.target.key);
      held.current = null;
      body.current?.scrollTo({ top: 0 });
      return;
    }
    const wanted = row.target.section;
    if (project !== null) {
      pendingSection.current = wanted;
      setProject(null);
      return;
    }
    // Already on the global page: scroll at once.
    scrollTo(wanted);
  };

  const globalRows = view?.nav.filter((r) => r.target.page === "global") ?? [];
  const projectRows = view?.nav.filter((r) => r.target.page === "project") ?? [];
  const shownSections = page?.page === "global" ? sections.map((s) => s.id) : [];

  return (
    <div className="settings-backdrop" role="presentation">
      <div
        ref={root}
        className="settings"
        role="dialog"
        aria-modal="true"
        aria-label={view?.title}
        tabIndex={-1}
      >
        <nav className="settings-nav" aria-label={view?.title}>
          <h2 className="settings-title">{view?.title}</h2>
          <h3 className="settings-nav-heading">{view?.global_heading}</h3>
          {globalRows.map((row) => {
            const section = row.target.page === "global" ? row.target.section : null;
            return (
              <NavItem
                key={row.label}
                row={row}
                on={project === null && section !== null && shownSections[inView] === section}
                onClick={() => {
                  go(row);
                }}
              />
            );
          })}
          <h3 className="settings-nav-heading">{view?.projects_heading}</h3>
          {projectRows.map((row) => (
            <NavItem
              key={row.target.page === "project" ? row.target.key : row.label}
              row={row}
              on={row.target.page === "project" && row.target.key === project}
              onClick={() => {
                go(row);
              }}
            />
          ))}
        </nav>
        <div className="settings-main">
          <header className="settings-header">
            <h2>{page?.title}</h2>
            <IconButton icon="x" label={view?.close_label ?? ""} onClick={close} />
          </header>
          {error && (
            <p className="settings-error" role="alert">
              {error.message}
            </p>
          )}
          <div
            ref={body}
            className="settings-body"
            onScroll={onScroll}
            onWheel={() => {
              clicked.current = false;
              held.current = null;
            }}
          >
            {page?.page === "project" && <p className="settings-intro settings-page-intro">{page.intro}</p>}
            {context && (
              <SettingsContext value={context}>
                {sections.map(({ id, Component }) => (
                  <div key={`${project ?? ""}-${id}`} className="settings-section-slot">
                    <Component project={project ?? ""} />
                  </div>
                ))}
              </SettingsContext>
            )}
          </div>
        </div>
        {dialog?.kind === "remove" && project !== null && (
          <RemoveDialog
            dialog={dialog.dialog}
            onCancel={() => {
              setDialog(null);
            }}
            onConfirm={() => {
              setDialog(null);
              request("settings.remove", { project }).then(
                (fresh) => {
                  setProject(null);
                  setView(fresh as SettingsView);
                },
                (e: unknown) => {
                  setError(asRequestError(e));
                },
              );
            }}
          />
        )}
        {dialog?.kind === "credits" && (
          <CreditsDialog
            dialog={dialog.dialog}
            onClose={() => {
              setDialog(null);
            }}
          />
        )}
      </div>
    </div>
  );
}
