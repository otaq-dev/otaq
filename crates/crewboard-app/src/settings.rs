//! The Settings window (⌘,; SET-01, PRJ-03, PRJ-04; d0003, d0023, d0040, d0055; *15 · Settings → Global ·
//! Usage, notifications, data, appearance; Global · Tools; Project · fresh; Project · configured + invalid
//! value; Remove project · confirm; Dark · project settings*). A section nav on the left (the global sections,
//! then every project), one page on the right. Text fields save when they lose focus or on ↵; choices save at
//! once. The service flags invalid values; they show in red with the reason and stay stored.

use crewboard_core::board::projects::ProjectUpdate;
use crewboard_core::board::settings::{AgentLimits, Appearance, ProjectSettings};
use crewboard_core::data_dir::DataDir;
use crewboard_core::protocol::{
    KeyParams, ProjectParams, ProjectSettingsParams, ProjectUpdateParams, Reply, Request,
};
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, IntoElement, KeyBinding, MouseButton, Point, ScrollHandle, SharedString, Window,
    actions, anchored, deferred, div, prelude::*, px, relative,
};

use crate::components::button::{Button, IconButton, Variant};
use crate::components::dialog::ConfirmDialog;
use crate::components::field::DropdownButton;
use crate::components::floating_shadow;
use crate::components::menu::{ContextMenu, MenuChosen};
use crate::components::stepper::Stepper;
use crate::components::tabs::{Segmented, Toggle};
use crate::components::text_input::{InputEvent, TextInput};
use crate::icons::{IconName, icon};
use crate::shell::tilde;
use crate::theme::{ActiveTheme, Colors, FONT_MONO, radius};
use crate::view_models::dialog::DialogKind;
use crate::view_models::editor_fields::base_choices;
use crate::view_models::limits::{
    BACK_PRESSURE_ITEMS, LimitRow, MAX_AGENTS, right_now, settings_rows, toggled,
};
use crate::view_models::menu::MenuItem;
use crate::view_models::onboarding::{OnboardingModel, RowIcon};
use crate::view_models::settings::{
    GLOBAL_SECTIONS, Page, ProjectPage, SettingsModel, data_paths, env_file_problem,
    instructions_placeholder, parse_font_size, problem_for, remove_text, theme_choice,
};
use crate::workspace::Workspace;

actions!(settings, [CloseSettings]);

const CONTEXT: &str = "Settings";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CloseSettings, Some(CONTEXT))]);
}

/// The window is done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsClosed;

/// The project page's text inputs.
#[derive(Clone)]
struct ProjectInputs {
    key: String,
    name: Entity<TextInput>,
    instructions: Entity<TextInput>,
    setup: Entity<TextInput>,
    run: Entity<TextInput>,
    open_url: Entity<TextInput>,
    env_files: Vec<Entity<TextInput>>,
    new_env: Entity<TextInput>,
}

pub struct SettingsView {
    workspace: Entity<Workspace>,
    model: SettingsModel,
    page: Page,
    tools: OnboardingModel,
    global_instructions: Entity<TextInput>,
    font_size: Entity<TextInput>,
    font_error: Option<String>,
    project: Option<ProjectInputs>,
    base_menu: Option<(Entity<ContextMenu>, Point<gpui::Pixels>)>,
    remove: Option<(String, usize)>,
    error: Option<String>,
    focus: FocusHandle,
    dialog_focus: FocusHandle,
    scroll: ScrollHandle,
    /// A project page asked for from outside ("Instructions: shop-web"), opened once its settings are loaded.
    pending_instructions: Option<String>,
}

impl EventEmitter<SettingsClosed> for SettingsView {}

/// Settings → Data → Cleanup… (REC-04): the owner closes Settings and opens the Cleanup window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenCleanup;

impl EventEmitter<OpenCleanup> for SettingsView {}

impl Focusable for SettingsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl SettingsView {
    pub fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let global_instructions = cx.new(|cx| {
            TextInput::new(window, cx)
                .multiline(px(64.))
                .placeholder("Your rules for every agent, e.g. “Ask before coding.”")
        });
        cx.subscribe(&global_instructions, |s, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Blur | InputEvent::Submit) {
                let text = input.read(cx).text().to_owned();
                if text != s.model.global.agent_instructions {
                    s.model.global.agent_instructions = text;
                    s.save_global(cx);
                }
            }
            if *event == InputEvent::Cancel {
                cx.emit(SettingsClosed);
            }
        })
        .detach();
        let font_size = cx.new(|cx| TextInput::new(window, cx).mono());
        cx.subscribe(&font_size, |s, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Blur | InputEvent::Submit) {
                match parse_font_size(input.read(cx).text()) {
                    Ok(n) => {
                        s.font_error = None;
                        if n != s.model.global.terminal_font_size {
                            s.model.global.terminal_font_size = n;
                            s.save_global(cx);
                        }
                    }
                    Err(e) => s.font_error = Some(e),
                }
                let error = s.font_error.is_some();
                input.update(cx, |i, cx| i.set_error(error, cx));
                cx.notify();
            }
        })
        .detach();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let view = Self {
            workspace,
            model: SettingsModel::default(),
            page: Page::Global,
            pending_instructions: None,
            tools: OnboardingModel::new(true),
            global_instructions,
            font_size,
            font_error: None,
            project: None,
            base_menu: None,
            remove: None,
            error: None,
            focus,
            dialog_focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
        };
        view.load(window, cx);
        view.check_tools(cx);
        view
    }

    fn send(
        &self,
        request: Request,
        cx: &App,
    ) -> impl std::future::Future<Output = Result<Reply, crate::service_link::RequestError>> + 'static
    {
        self.workspace.read(cx).link.read(cx).send(request)
    }

    /// Loads the global settings and every project's page.
    fn load(&self, window: &mut Window, cx: &mut Context<Self>) {
        let global = self.send(Request::SettingsGet, cx);
        let projects: Vec<_> = self
            .workspace
            .read(cx)
            .switcher
            .projects()
            .to_vec()
            .into_iter()
            .map(|info| {
                let request = self.send(
                    Request::ProjectSettingsGet(ProjectParams {
                        project: info.key.clone(),
                    }),
                    cx,
                );
                (info, request)
            })
            .collect();
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Reply::GlobalSettings(reply)) = global.await {
                let _ = this.update_in(cx, |s, _, cx| {
                    s.model.global = reply.settings.clone();
                    s.model.global_problems = reply.problems.clone();
                    let (instructions, size) = (
                        reply.settings.agent_instructions.clone(),
                        reply.settings.terminal_font_size,
                    );
                    s.global_instructions
                        .update(cx, |i, cx| i.set_text(&instructions, cx));
                    s.font_size
                        .update(cx, |i, cx| i.set_text(&format!("{size} px"), cx));
                    cx.notify();
                });
            }
            for (info, request) in projects {
                if let Ok(Reply::ProjectSettings(reply)) = request.await {
                    let _ = this.update(cx, |s, cx| {
                        s.model.set_project(ProjectPage {
                            info,
                            settings: reply.settings.clone(),
                            problems: reply.problems.clone(),
                        });
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn check_tools(&self, cx: &mut Context<Self>) {
        let request = self.send(Request::ToolsCheck, cx);
        cx.spawn(async move |this, cx| {
            let result = match request.await {
                Ok(Reply::ToolsCheck(report)) => Ok(report),
                Ok(other) => Err(format!("unexpected answer: {other:?}")),
                Err(e) => Err(e.message),
            };
            let _ = this.update(cx, |s, cx| {
                s.tools.finish_check(result);
                cx.notify();
            });
        })
        .detach();
    }

    fn save_global(&mut self, cx: &mut Context<Self>) {
        let request = self.send(Request::SettingsSet(self.model.global.clone()), cx);
        let settings = self.model.global.clone();
        self.workspace.update(cx, |ws, cx| {
            ws.global_settings = Some(settings);
            cx.notify();
        });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |s, cx| {
                match result {
                    Ok(Reply::GlobalSettings(reply)) => s.model.global_problems = reply.problems,
                    Ok(_) => {}
                    Err(e) => s.error = Some(e.message),
                }
                // The limits apply at once: show the scheduler's new view.
                s.workspace.update(cx, |ws, cx| ws.refresh_scheduler(cx));
                cx.notify();
            });
        })
        .detach();
    }

    /// Agent limits (SCH-03; *15 → Global · Agent limits OFF / ON*): the toggle and two steppers, saved at once.
    fn limits_section(&self, c: &Colors, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let limits = self.model.global.agent_limits;
        let rows = settings_rows(&limits);
        let this = cx.entity().downgrade();
        let save = move |limits: AgentLimits, cx: &mut App| {
            let _ = this.update(cx, |s, cx| {
                s.model.global.agent_limits = limits;
                s.save_global(cx);
                cx.notify();
            });
        };
        let mut out = vec![
            Self::section_title("Agent limits", c),
            div()
                .pb(px(4.))
                .text_size(px(12.))
                .text_color(c.text_muted)
                .child("Optional caps on how many agents run at once.")
                .into_any_element(),
        ];
        let toggle_save = save.clone();
        out.push(Self::row(
            rows[0].label,
            rows[0].description,
            Toggle::new("settings-limits", limits.enabled)
                .on_change(move |on, _, cx| toggle_save(toggled(&limits, on), cx))
                .into_any_element(),
            c,
        ));
        let max_save = save.clone();
        out.push(Self::dimmed_row(
            &rows[1],
            Stepper::new("settings-max-agents", limits.max_concurrency, MAX_AGENTS)
                .disabled(!rows[1].enabled)
                .on_change(move |n, _, cx| {
                    max_save(
                        AgentLimits {
                            max_concurrency: n,
                            ..limits
                        },
                        cx,
                    )
                })
                .into_any_element(),
            c,
        ));
        out.push(Self::dimmed_row(
            &rows[2],
            Stepper::new(
                "settings-back-pressure",
                limits.back_pressure,
                BACK_PRESSURE_ITEMS,
            )
            .disabled(!rows[2].enabled)
            .on_change(move |n, _, cx| {
                save(
                    AgentLimits {
                        back_pressure: n,
                        ..limits
                    },
                    cx,
                )
            })
            .into_any_element(),
            c,
        ));
        let status = self.workspace.read(cx).scheduler;
        if let (true, Some(status)) = (limits.enabled, status) {
            out.push(
                div()
                    .mt(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(px(6.))
                    .bg(c.bg_app)
                    .child(icon(IconName::Info, px(13.)).text_color(c.text_secondary))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(c.text_secondary)
                            .child(SharedString::from(right_now(&status))),
                    )
                    .into_any_element(),
            );
        }
        out
    }

    /// A row whose label and description dim with its control while the limits are off.
    fn dimmed_row(row: &LimitRow, control: AnyElement, c: &Colors) -> AnyElement {
        let el = Self::row(row.label, row.description, control, c);
        if row.enabled {
            el
        } else {
            div().opacity(0.6).child(el).into_any_element()
        }
    }

    fn save_project(&mut self, key: String, settings: ProjectSettings, cx: &mut Context<Self>) {
        let request = self.send(
            Request::ProjectSettingsSet(ProjectSettingsParams {
                project: key.clone(),
                settings,
            }),
            cx,
        );
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |s, cx| {
                match result {
                    Ok(Reply::ProjectSettings(reply)) => {
                        if let Some(page) = s.model.projects.iter_mut().find(|p| p.info.key == key)
                        {
                            page.settings = reply.settings;
                            page.problems = reply.problems;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => s.error = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn update_project(&mut self, key: String, update: ProjectUpdate, cx: &mut Context<Self>) {
        let request = self.send(
            Request::ProjectUpdate(ProjectUpdateParams {
                project: key.clone(),
                update,
            }),
            cx,
        );
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |s, cx| {
                match result {
                    Ok(Reply::Project(info)) => {
                        if let Some(page) = s.model.projects.iter_mut().find(|p| p.info.key == key)
                        {
                            page.info = info;
                        }
                        s.error = None;
                    }
                    Ok(_) => {}
                    Err(e) => s.error = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Edits the shown project's settings with `edit` and saves them.
    fn edit_project(&mut self, edit: impl FnOnce(&mut ProjectSettings), cx: &mut Context<Self>) {
        let Page::Project(key) = self.page.clone() else {
            return;
        };
        let Some(page) = self.model.projects.iter_mut().find(|p| p.info.key == key) else {
            return;
        };
        let before = page.settings.clone();
        edit(&mut page.settings);
        if page.settings != before {
            let settings = page.settings.clone();
            self.save_project(key, settings, cx);
        }
        cx.notify();
    }

    /// Opens a project's page at its Agent instructions (the terminal's "Instructions: shop-web", d0040).
    pub fn show_project_instructions(&mut self, project: &str, cx: &mut Context<Self>) {
        self.pending_instructions = Some(project.to_owned());
        cx.notify();
    }

    fn open_page(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        self.page = page.clone();
        self.base_menu = None;
        self.error = None;
        self.project = match &page {
            Page::Global => None,
            Page::Project(key) => self
                .model
                .project(key)
                .cloned()
                .map(|p| self.project_inputs(p, window, cx)),
        };
        cx.notify();
    }

    fn project_inputs(
        &self,
        page: ProjectPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ProjectInputs {
        let text = |value: &str,
                    placeholder: &str,
                    mono: bool,
                    window: &mut Window,
                    cx: &mut Context<Self>| {
            let (value, placeholder) = (value.to_owned(), placeholder.to_owned());
            cx.new(move |cx| {
                let mut input = TextInput::new(window, cx).placeholder(placeholder);
                if mono {
                    input = input.mono();
                }
                input.set_text(&value, cx);
                input
            })
        };
        let s = &page.settings;
        let name = text(&page.info.name, "Project name", false, window, cx);
        let placeholder = instructions_placeholder(&self.model.global);
        let instructions = cx.new(|cx| {
            let mut input = TextInput::new(window, cx)
                .multiline(px(64.))
                .placeholder(placeholder);
            input.set_text(&page.settings.agent_instructions, cx);
            input
        });
        let setup = text(
            &s.setup_command,
            "none — e.g. pnpm install",
            true,
            window,
            cx,
        );
        let run = text(&s.run_command, "not set", true, window, cx);
        let open_url = text(&s.open_url, "none", true, window, cx);
        let env_files: Vec<_> = s
            .env_files
            .iter()
            .map(|f| text(f, ".env.local", true, window, cx))
            .collect();
        let new_env = text("", "Add file, e.g. .env.local", true, window, cx);
        let key = page.info.key.clone();
        // Name → project.update; the rest → project settings.
        cx.subscribe(&name, {
            let key = key.clone();
            move |s, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::Submit) {
                    let value = input.read(cx).text().trim().to_owned();
                    let current = s.model.project(&key).map(|p| p.info.name.clone());
                    if !value.is_empty() && Some(&value) != current.as_ref() {
                        s.update_project(
                            key.clone(),
                            ProjectUpdate {
                                name: Some(value),
                                ..ProjectUpdate::default()
                            },
                            cx,
                        );
                    }
                }
            }
        })
        .detach();
        type Setter = fn(&mut ProjectSettings, String);
        let fields: [(&Entity<TextInput>, Setter); 4] = [
            (&instructions, |p, v| p.agent_instructions = v),
            (&setup, |p, v| p.setup_command = v),
            (&run, |p, v| p.run_command = v),
            (&open_url, |p, v| p.open_url = v),
        ];
        for (input, set) in fields {
            cx.subscribe(input, move |s, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::Submit) {
                    let value = input.read(cx).text().to_owned();
                    s.edit_project(|p| set(p, value), cx);
                }
            })
            .detach();
        }
        for (i, input) in env_files.iter().enumerate() {
            cx.subscribe(input, move |s, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::Submit) {
                    let value = input.read(cx).text().trim().to_owned();
                    s.edit_project(
                        |p| {
                            if i < p.env_files.len() {
                                p.env_files[i] = value;
                            }
                        },
                        cx,
                    );
                }
            })
            .detach();
        }
        cx.subscribe_in(
            &new_env,
            window,
            |s, input, event: &InputEvent, window, cx| {
                if *event == InputEvent::Submit {
                    let value = input.read(cx).text().trim().to_owned();
                    if !value.is_empty() {
                        s.edit_project(|p| p.env_files.push(value), cx);
                        let page = s.page.clone();
                        s.open_page(page, window, cx);
                    }
                }
            },
        )
        .detach();
        ProjectInputs {
            key,
            name,
            instructions,
            setup,
            run,
            open_url,
            env_files,
            new_env,
        }
    }

    fn open_base_menu(
        &mut self,
        key: String,
        position: Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let request = self.send(
            Request::ProjectBranches(ProjectParams {
                project: key.clone(),
            }),
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Reply::Branches(branches)) = request.await else {
                return;
            };
            let _ = this.update_in(cx, |s, window, cx| {
                let current = s
                    .model
                    .project(&key)
                    .map(|p| p.info.base_branch.clone())
                    .unwrap_or_default();
                let (local, remote) = base_choices(&branches.local, &branches.remote);
                let mut items = vec![MenuItem::header("LOCAL")];
                for b in local {
                    items.push(
                        MenuItem::new(SharedString::from(b.clone()), b.clone())
                            .icon(IconName::GitBranch)
                            .checked(b == current),
                    );
                }
                if !remote.is_empty() {
                    items.push(MenuItem::header("REMOTE"));
                    for (full, name) in remote {
                        items.push(
                            MenuItem::new(SharedString::from(name.clone()), full)
                                .icon(IconName::GitBranch)
                                .checked(name == current),
                        );
                    }
                }
                let menu = cx.new(|cx| ContextMenu::new(items, window, cx).width(280.));
                let chosen_key = key.clone();
                cx.subscribe(&menu, move |s, _, chosen: &MenuChosen, cx| {
                    s.update_project(
                        chosen_key.clone(),
                        ProjectUpdate {
                            base_branch: Some(chosen.0.to_string()),
                            ..ProjectUpdate::default()
                        },
                        cx,
                    );
                })
                .detach();
                cx.subscribe_in(&menu, window, |s, _, _: &DismissEvent, window, cx| {
                    s.base_menu = None;
                    window.focus(&s.focus, cx);
                    cx.notify();
                })
                .detach();
                s.base_menu = Some((menu, position));
                cx.notify();
            });
        })
        .detach();
    }

    /// Asks before removing: counts the project's tickets for the confirmation.
    fn ask_remove(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        let request = self.send(
            Request::BoardSnapshot(ProjectParams {
                project: key.clone(),
            }),
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let count = match request.await {
                Ok(Reply::Board(board)) => board.tickets().count(),
                _ => 0,
            };
            let _ = this.update_in(cx, |s, window, cx| {
                s.remove = Some((key, count));
                window.focus(&s.dialog_focus, cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn remove_project(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        self.remove = None;
        let request = self.send(Request::ProjectRemove(KeyParams { key: key.clone() }), cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = request.await;
            let _ = this.update_in(cx, |s, window, cx| {
                match result {
                    Ok(_) => {
                        s.model.projects.retain(|p| p.info.key != key);
                        s.open_page(Page::Global, window, cx);
                        s.workspace.update(cx, |ws, cx| ws.refresh_projects(cx));
                    }
                    Err(e) => s.error = Some(e.message),
                }
                window.focus(&s.focus, cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn row(
        label: &'static str,
        description: impl Into<SharedString>,
        control: AnyElement,
        c: &Colors,
    ) -> AnyElement {
        div()
            .flex()
            .gap(px(24.))
            .py(px(12.))
            .border_b_1()
            .border_color(c.border)
            .child(
                div()
                    .w(px(320.))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.text_primary)
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(relative(1.4))
                            .text_color(c.text_muted)
                            .child(description.into()),
                    ),
            )
            .child(div().flex_1().min_w_0().child(control))
            .into_any_element()
    }

    fn section_title(title: &'static str, c: &Colors) -> AnyElement {
        div()
            .pt(px(16.))
            .pb(px(4.))
            .font_family(FONT_MONO)
            .text_size(px(10.))
            .text_color(c.text_muted)
            .child(SharedString::from(title.to_uppercase()))
            .into_any_element()
    }

    fn path_row(
        path: String,
        reveal: std::path::PathBuf,
        id: &'static str,
        c: &Colors,
    ) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .flex_1()
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(5.))
                    .bg(c.bg_column)
                    .font_family(FONT_MONO)
                    .text_size(px(12.))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(path)),
            )
            .child(
                Button::new(id, "Reveal in Finder")
                    .variant(Variant::Secondary)
                    .icon(IconName::FolderOpen)
                    .on_click(move |_, _, cx| cx.reveal_path(&reveal)),
            )
            .into_any_element()
    }

    fn global_page(&self, c: &Colors, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let data = DataDir::from_env().ok();
        let mut out = self.limits_section(c, cx);
        out.push(Self::section_title("Agent instructions", c));
        out.push(Self::row(
            "Default instructions",
            format!(
                "Given to every agent in every project through {} prime (0040). A project can override it in its own settings.",
                crewboard_core::naming::COMMAND_NAME
            ),
            self.global_instructions.clone().into_any_element(),
            c,
        ));
        // Notifications (ATT-11; *15 → Global · Usage, notifications, data, appearance*): on by default.
        out.push(Self::section_title("Notifications", c));
        let this = cx.entity().downgrade();
        out.push(Self::row(
            "OS notifications for blocking items",
            "Permission prompts, agent questions and interrupted sessions. Nothing else ever notifies.",
            Toggle::new("settings-notifications", self.model.global.notifications)
                .on_change(move |on, _, cx| {
                    let _ = this.update(cx, |s, cx| {
                        s.model.global.notifications = on;
                        s.save_global(cx);
                        cx.notify();
                    });
                })
                .into_any_element(),
            c,
        ));
        out.push(Self::section_title("Data", c));
        if let Some(data) = data {
            let (db, logs) = data_paths(&data);
            out.push(Self::row(
                "Database file",
                format!(
                    "Tickets, runs and settings. Your normal machine backup covers it — {} has no separate backup.",
                    crewboard_core::naming::PRODUCT_NAME
                ),
                Self::path_row(db, data.database_path(), "reveal-db", c),
                c,
            ));
            out.push(Self::row(
                "Logs folder",
                "Service and session logs, one file per day.",
                Self::path_row(logs, data.logs_dir(), "reveal-logs", c),
                c,
            ));
        }
        let count = self.workspace.read(cx).cleanup_count;
        out.push(Self::row(
            "Cleanup",
            if count == 0 {
                "Leftover sessions, worktrees, branches and ports that no ticket owns. Nothing is deleted without your click.".to_owned()
            } else {
                format!(
                    "{count} {} waiting. Nothing is deleted without your click.",
                    if count == 1 { "leftover" } else { "leftovers" }
                )
            },
            Button::new("settings-cleanup", "Cleanup…")
                .icon(IconName::BrushCleaning)
                .variant(crate::components::button::Variant::Secondary)
                .on_click(cx.listener(|_, _, _, cx| cx.emit(OpenCleanup)))
                .into_any_element(),
            c,
        ));
        out.push(Self::section_title("Appearance", c));
        let selected = match self.model.global.appearance {
            Appearance::Light => 0,
            Appearance::Dark => 1,
            Appearance::System => 2,
        };
        let this = cx.entity().downgrade();
        out.push(Self::row(
            "Theme",
            "Follows the OS by default.",
            Segmented::new(
                "settings-theme",
                vec!["Light".into(), "Dark".into(), "System".into()],
                selected,
            )
            .on_select(move |i, window, cx| {
                let appearance = [Appearance::Light, Appearance::Dark, Appearance::System][i];
                crate::theme::ActiveTheme::update(
                    theme_choice(appearance),
                    window.appearance(),
                    cx,
                );
                let _ = this.update(cx, |s, cx| {
                    s.model.global.appearance = appearance;
                    s.save_global(cx);
                });
            })
            .into_any_element(),
            c,
        ));
        out.push(Self::row(
            "Terminal font size",
            "Geist Mono, used by the terminal.",
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(div().w(px(100.)).child(self.font_size.clone()))
                .children(self.font_error.clone().map(|e| {
                    div()
                        .text_size(px(11.))
                        .text_color(c.st_error)
                        .child(SharedString::from(e))
                }))
                .into_any_element(),
            c,
        ));
        out.push(Self::section_title("Tools", c));
        let recheck = cx.listener(|s, _, _, cx| {
            s.tools.start_check();
            cx.notify();
            s.check_tools(cx);
        });
        let rows = self.tools.rows().into_iter().map(|row| {
            let (icon_name, color) = match row.icon {
                RowIcon::Found => (IconName::CircleCheck, c.st_approved),
                RowIcon::MissingRequired => (IconName::CircleX, c.st_error),
                RowIcon::MissingOptional => (IconName::CircleMinus, c.text_muted),
            };
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .py(px(6.))
                .child(icon(icon_name, px(14.)).text_color(color))
                .child(
                    div()
                        .w(px(150.))
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c.text_primary)
                        .child(row.name),
                )
                .child(
                    div()
                        .w(px(70.))
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child(if row.required { "required" } else { "optional" }),
                )
                .child(
                    div()
                        .flex_1()
                        .font_family(FONT_MONO)
                        .text_size(px(12.))
                        .text_color(c.text_secondary)
                        .child(SharedString::from(row.status)),
                )
                .children(row.command.map(|cmd| {
                    div()
                        .px(px(6.))
                        .rounded(px(4.))
                        .bg(c.bg_column)
                        .font_family(FONT_MONO)
                        .text_size(px(11.))
                        .text_color(c.text_secondary)
                        .child(SharedString::from(cmd))
                }))
        });
        out.push(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(c.text_muted)
                        .child(SharedString::from(format!(
                            "{} uses the tools already on your machine. It never installs or updates them.",
                            crewboard_core::naming::PRODUCT_NAME
                        ))),
                )
                .children(rows)
                .child(
                    div().pt(px(6.)).child(
                        Button::new("settings-recheck", "Re-check")
                            .variant(Variant::Secondary)
                            .icon(IconName::RotateCcw)
                            .loading(self.tools.checking)
                            .on_click(recheck),
                    ),
                )
                .into_any_element(),
        );
        out
    }

    fn project_page(
        &self,
        page: &ProjectPage,
        inputs: &ProjectInputs,
        c: &Colors,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let p = &page.problems;
        let flagged = |input: &Entity<TextInput>, field: &str, cx: &mut Context<Self>| {
            let message = problem_for(p, field);
            let error = message.is_some();
            input.update(cx, |i, cx| i.set_error(error, cx));
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(input.clone())
                .children(message.map(|m| {
                    div()
                        .text_size(px(11.))
                        .text_color(c.st_error)
                        .child(SharedString::from(m))
                }))
                .into_any_element()
        };
        let key = page.info.key.clone();
        let mut out = vec![div()
            .text_size(px(12.))
            .text_color(c.text_muted)
            .child("Zero config works: agents can start with just the repo. Fill these in when you need them.")
            .into_any_element()];
        out.push(Self::row(
            "Name",
            "Shown in the project rail and the header.",
            inputs.name.clone().into_any_element(),
            c,
        ));
        out.push(Self::row(
            "Project key",
            format!("Used in ticket IDs like {key}-18. Set when the project was added; it can’t be changed."),
            div()
                .w(px(120.))
                .px(px(10.))
                .py(px(6.))
                .rounded(px(5.))
                .bg(c.bg_column)
                .font_family(FONT_MONO)
                .text_size(px(12.))
                .text_color(c.text_secondary)
                .child(SharedString::from(key.clone()))
                .into_any_element(),
            c,
        ));
        out.push(Self::row(
            "Repository",
            "The repository this project works on. Nothing is ever written into it.",
            div()
                .font_family(FONT_MONO)
                .text_size(px(12.))
                .text_color(c.text_secondary)
                .child(SharedString::from(tilde(&page.info.repo_path)))
                .into_any_element(),
            c,
        ));
        let base_key = key.clone();
        out.push(Self::row(
            "Base branch",
            "Prefilled from the remote’s default branch (origin/HEAD); editable. Tickets and epics branch from here, and Local merge squashes into it.",
            div()
                .w(px(260.))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |s, e: &gpui::MouseDownEvent, window, cx| {
                        s.open_base_menu(base_key.clone(), e.position, window, cx)
                    }),
                )
                .child(
                    DropdownButton::new("settings-base", page.info.base_branch.clone())
                        .icon(IconName::GitBranch)
                        .mono()
                        .open(self.base_menu.is_some()),
                )
                .into_any_element(),
            c,
        ));
        out.push(Self::row(
            "Agent instructions",
            format!(
                "Per-project override, given to every agent in this project through {} prime. Empty = the global default is used.",
                crewboard_core::naming::COMMAND_NAME
            ),
            inputs.instructions.clone().into_any_element(),
            c,
        ));
        out.push(Self::row(
            "Setup command",
            "Runs once in every new worktree, before the agent starts.",
            flagged(&inputs.setup, "setup_command", cx),
            c,
        ));
        out.push(Self::row(
            "Run command",
            "Runs in the ticket’s worktree when you press Run. Use $PORT if it listens on a port.",
            flagged(&inputs.run, "run_command", cx),
            c,
        ));
        out.push(Self::row(
            "Open URL",
            "Optional. Shows an Open button while it runs. Leave empty for backends, CLIs and libraries.",
            flagged(&inputs.open_url, "open_url", cx),
            c,
        ));
        let env_rows: Vec<AnyElement> = inputs
            .env_files
            .iter()
            .enumerate()
            .map(|(i, input)| {
                let file = page.settings.env_files.get(i).cloned().unwrap_or_default();
                let message = env_file_problem(p, &file);
                let error = message.is_some();
                input.update(cx, |inp, cx| inp.set_error(error, cx));
                let remove = cx.listener(move |s, _, window, cx| {
                    s.edit_project(
                        |p| {
                            if i < p.env_files.len() {
                                p.env_files.remove(i);
                            }
                        },
                        cx,
                    );
                    let page = s.page.clone();
                    s.open_page(page, window, cx);
                });
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(div().flex_1().child(input.clone()))
                            .child(
                                IconButton::new(("remove-env", i), IconName::X).on_click(remove),
                            ),
                    )
                    .children(message.map(|m| {
                        div()
                            .text_size(px(11.))
                            .text_color(c.st_error)
                            .child(SharedString::from(m))
                    }))
                    .into_any_element()
            })
            .collect();
        out.push(Self::row(
            "Env files to copy into worktrees",
            "Copied from the repo root into each new worktree. Never committed. ↵ adds a file.",
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .children(env_rows)
                .child(inputs.new_env.clone())
                .into_any_element(),
            c,
        ));
        let remove_key = key.clone();
        out.push(Self::row(
            "Remove project",
            format!(
                "Deletes this project’s tickets and history from {}. Never touches the repository.",
                crewboard_core::naming::PRODUCT_NAME
            ),
            Button::new("settings-remove", "Remove project…")
                .variant(Variant::Destructive)
                .icon(IconName::Trash2)
                .on_click(
                    cx.listener(move |s, _, window, cx| {
                        s.ask_remove(remove_key.clone(), window, cx)
                    }),
                )
                .into_any_element(),
            c,
        ));
        out
    }

    fn remove_dialog(
        &self,
        key: &str,
        tickets: usize,
        c: &Colors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(page) = self.model.project(key) else {
            return div().into_any_element();
        };
        let text = remove_text(&page.info, tickets);
        let list = |title: &'static str, items: &[String], ok: bool| {
            let (fg, bg, mark) = if ok {
                (c.st_approved, c.st_approved_bg, IconName::Check)
            } else {
                (c.st_error, c.st_error_bg, IconName::X)
            };
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .p(px(10.))
                .rounded(px(radius::MD))
                .bg(bg)
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c.text_primary)
                        .child(title),
                )
                .children(items.iter().map(|item| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(icon(mark, px(12.)).text_color(fg))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(c.text_secondary)
                                .child(SharedString::from(item.clone())),
                        )
                }))
                .into_any_element()
        };
        let confirm = {
            let this = cx.entity().downgrade();
            let key = key.to_owned();
            move |window: &mut Window, cx: &mut App| {
                let _ = this.update(cx, |s, cx| s.remove_project(key.clone(), window, cx));
            }
        };
        let cancel = {
            let this = cx.entity().downgrade();
            move |window: &mut Window, cx: &mut App| {
                let _ = this.update(cx, |s, cx| {
                    s.remove = None;
                    window.focus(&s.focus, cx);
                    cx.notify();
                });
            }
        };
        ConfirmDialog::new(
            "remove-project",
            &self.dialog_focus,
            DialogKind::Destructive,
            text.title.clone(),
            "Remove project",
            confirm,
            cancel,
        )
        .confirm_icon(IconName::Trash2)
        .child(list("Deleted from the app", &text.deleted, false))
        .child(list("Never touched", &text.kept, true))
        .children_boxes(
            text.agents
                .clone()
                .into_iter()
                .map(|a| {
                    div()
                        .flex()
                        .gap(px(8.))
                        .p(px(10.))
                        .rounded(px(radius::MD))
                        .bg(c.st_paused_bg)
                        .child(icon(IconName::TriangleAlert, px(13.)).text_color(c.st_paused))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(c.text_primary)
                                .child(SharedString::from(a)),
                        )
                        .into_any_element()
                })
                .collect(),
        )
        .into_any_element()
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(key) = self.pending_instructions.clone()
            && self.model.project(&key).is_some()
        {
            self.pending_instructions = None;
            self.open_page(Page::Project(key), window, cx);
            if let Some(inputs) = &self.project {
                let focus = inputs.instructions.read(cx).focus_handle(cx);
                window.focus(&focus, cx);
            }
        }
        let c = ActiveTheme::get(cx).colors;
        let nav_item =
            |id: SharedString, label: SharedString, on: bool, alert: bool, c: &Colors| {
                let mut el = div()
                    .id(id)
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(10.))
                    .py(px(5.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .text_color(if on { c.text_primary } else { c.text_secondary })
                    .child(div().flex_1().truncate().child(label))
                    .children(
                        alert.then(|| icon(IconName::CircleAlert, px(12.)).text_color(c.st_error)),
                    );
                el = if on {
                    el.bg(c.accent_soft).font_weight(FontWeight::MEDIUM)
                } else {
                    el.hover(|s| s.bg(c.bg_hover))
                };
                el
            };
        let global_on = self.page == Page::Global;
        let globals = GLOBAL_SECTIONS
            .iter()
            .enumerate()
            .map(|(i, name)| {
                nav_item(
                    SharedString::from(format!("nav-global-{i}")),
                    SharedString::from(*name),
                    global_on,
                    false,
                    &c,
                )
                .on_click(cx.listener(|s, _, window, cx| s.open_page(Page::Global, window, cx)))
            })
            .collect::<Vec<_>>();
        let projects: Vec<_> = self.workspace.read(cx).switcher.projects().to_vec();
        let project_items = projects
            .iter()
            .map(|p| {
                let key = p.key.clone();
                let on = self.page == Page::Project(key.clone());
                nav_item(
                    SharedString::from(format!("nav-project-{key}")),
                    SharedString::from(p.name.clone()),
                    on,
                    self.model.needs_attention(&key),
                    &c,
                )
                .on_click(cx.listener(move |s, _, window, cx| {
                    s.open_page(Page::Project(key.clone()), window, cx)
                }))
            })
            .collect::<Vec<_>>();
        let (title, body) = match (&self.page, &self.project) {
            (Page::Project(key), Some(inputs)) if inputs.key == *key => {
                let page = self.model.project(key).cloned();
                match page {
                    Some(page) => {
                        let body = self
                            .project
                            .clone()
                            .map(|i| self.project_page(&page, &i, &c, cx))
                            .unwrap_or_default();
                        (page.info.name.clone(), body)
                    }
                    None => (key.clone(), Vec::new()),
                }
            }
            _ => ("General".to_owned(), self.global_page(&c, cx)),
        };
        let menu = self.base_menu.as_ref().map(|(menu, at)| {
            deferred(anchored().position(*at).child(menu.clone())).with_priority(2)
        });
        let dialog = self
            .remove
            .clone()
            .map(|(key, n)| self.remove_dialog(&key, n, &c, cx));
        div()
            .id("settings")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &CloseSettings, _, cx| cx.emit(SettingsClosed)))
            .relative()
            .w(px(1040.))
            .h(px(720.))
            .flex()
            .overflow_hidden()
            .bg(c.bg_raised)
            .border_1()
            .border_color(c.border)
            .rounded(px(radius::DIALOG))
            .shadow(floating_shadow())
            .child(
                div()
                    .id("settings-nav")
                    .w(px(220.))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .p(px(12.))
                    .bg(c.bg_panel)
                    .border_r_1()
                    .border_color(c.border)
                    .overflow_y_scroll()
                    .child(
                        div()
                            .px(px(10.))
                            .pb(px(10.))
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.text_primary)
                            .child("Settings"),
                    )
                    .child(Self::section_title("Global", &c))
                    .children(globals)
                    .child(Self::section_title("Projects", &c))
                    .children(project_items),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .px(px(24.))
                            .py(px(14.))
                            .border_b_1()
                            .border_color(c.border)
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c.text_primary)
                                    .child(SharedString::from(title)),
                            )
                            .child(
                                IconButton::new("settings-close", IconName::X)
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsClosed))),
                            ),
                    )
                    .children(self.error.clone().map(|e| {
                        div()
                            .mx(px(24.))
                            .mt(px(10.))
                            .px(px(10.))
                            .py(px(6.))
                            .rounded(px(5.))
                            .bg(c.st_error_bg)
                            .text_size(px(12.))
                            .text_color(c.st_error)
                            .child(SharedString::from(e))
                    }))
                    .child(
                        div()
                            .id("settings-body")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .px(px(24.))
                            .pb(px(24.))
                            .children(body),
                    ),
            )
            .children(menu)
            .children(dialog)
    }
}
