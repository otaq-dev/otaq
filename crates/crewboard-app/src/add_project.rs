//! The add-project form (UI-08; *14 · First run & onboarding → 3 · Add project · valid*, *Not a git repo*,
//! *Already added*, *Adding a second project later*): a folder picker (or a folder dropped on the window), the
//! service's validation shown inline, the detected base branch (changeable before adding) and the key field
//! with its live check and the hint "Used in ticket IDs like SHOP-18. Can't be changed later." The same form is
//! the onboarding step and the shorter dialog that Add project opens later. Nothing is written into the repo
//! (d0023).

use std::path::PathBuf;

use crewboard_core::board::projects::AddProject;
use crewboard_core::protocol::{PathParams, Reply, Request};
use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, ExternalPaths, FocusHandle, FontWeight,
    IntoElement, PathPromptOptions, SharedString, Window, div, prelude::*, px, relative,
};

use crate::components::button::{Button, Variant};
use crate::components::floating_shadow;
use crate::components::kbd::Kbd;
use crate::components::text_input::{InputEvent, TextInput};
use crate::icons::{IconName, icon};
use crate::keymap::Platform;
use crate::shell::tilde;
use crate::theme::{ActiveTheme, Colors, FONT_MONO, radius};
use crate::view_models::add_project::AddProjectModel;
use crate::workspace::Workspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormMode {
    /// The onboarding card: "Add your first project".
    FirstRun,
    /// The dialog from the rail, ⌘P or the empty board.
    Dialog,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddProjectEvent {
    /// The project was added; its key.
    Added(String),
    Cancelled,
}

pub struct AddProjectForm {
    workspace: Entity<Workspace>,
    mode: FormMode,
    model: AddProjectModel,
    key_input: Entity<TextInput>,
    base_input: Entity<TextInput>,
    focus: FocusHandle,
}

impl EventEmitter<AddProjectEvent> for AddProjectForm {}

gpui::actions!(add_project, [CancelAddProject]);

pub const CONTEXT: &str = "AddProject";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([gpui::KeyBinding::new(
        "escape",
        CancelAddProject,
        Some(CONTEXT),
    )]);
}

impl AddProjectForm {
    pub fn new(
        workspace: Entity<Workspace>,
        mode: FormMode,
        folder: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let key_input = cx.new(|cx| TextInput::new(window, cx).mono().placeholder("KEY"));
        let base_input = cx.new(|cx| TextInput::new(window, cx).mono().placeholder("main"));
        cx.subscribe(
            &key_input,
            |form, input, event: &InputEvent, cx| match event {
                InputEvent::Changed => {
                    let typed = input.read(cx).text().to_owned();
                    form.model.set_key(&typed);
                    if form.model.key != typed {
                        let key = form.model.key.clone();
                        input.update(cx, |i, cx| i.set_text(&key, cx));
                    }
                    cx.notify();
                }
                InputEvent::Submit => form.add(cx),
                InputEvent::Cancel => cx.emit(AddProjectEvent::Cancelled),
                _ => {}
            },
        )
        .detach();
        cx.subscribe(
            &base_input,
            |form, input, event: &InputEvent, cx| match event {
                InputEvent::Changed => {
                    let text = input.read(cx).text().to_owned();
                    form.model.set_base(&text);
                    cx.notify();
                }
                InputEvent::Submit => form.add(cx),
                InputEvent::Cancel => cx.emit(AddProjectEvent::Cancelled),
                _ => {}
            },
        )
        .detach();
        cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
        let focus = cx.focus_handle();
        if mode == FormMode::Dialog {
            window.focus(&focus, cx);
        }
        let mut form = Self {
            workspace,
            mode,
            model: AddProjectModel::default(),
            key_input,
            base_input,
            focus,
        };
        if let Some(folder) = folder {
            form.pick(folder, cx);
        }
        form
    }

    /// Opens the system folder picker.
    fn choose_folder(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add project".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |form, cx| form.pick(path, cx));
            }
        })
        .detach();
    }

    pub fn pick(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let folder = path.display().to_string();
        self.model.pick(folder.clone());
        cx.notify();
        let request = self
            .workspace
            .read(cx)
            .link
            .read(cx)
            .send(Request::ProjectProbe(PathParams { path: folder }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |form, cx| {
                let projects = form.workspace.read(cx).switcher.projects().to_vec();
                let result = match result {
                    Ok(Reply::FolderProbe(probe)) => Ok(probe),
                    Ok(other) => Err((None, format!("unexpected answer: {other:?}"))),
                    Err(e) => Err((e.refusal, e.message)),
                };
                form.model.probed(result, &projects, Platform::current());
                let (key, base) = (form.model.key.clone(), form.model.base_branch.clone());
                form.key_input.update(cx, |i, cx| i.set_text(&key, cx));
                form.base_input.update(cx, |i, cx| i.set_text(&base, cx));
                cx.notify();
            });
        })
        .detach();
    }

    fn add(&mut self, cx: &mut Context<Self>) {
        let projects = self.workspace.read(cx).switcher.projects().to_vec();
        if !self.model.can_add(&projects) {
            return;
        }
        let Some(probe) = self.model.probe.clone() else {
            return;
        };
        self.model.adding = true;
        self.model.add_error = None;
        cx.notify();
        let request = self
            .workspace
            .read(cx)
            .link
            .read(cx)
            .send(Request::ProjectAdd(AddProject {
                path: probe.repo_path,
                key: Some(self.model.key.clone()),
                base_branch: Some(self.model.base_branch.clone()),
            }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |form, cx| {
                match result {
                    Ok(Reply::Project(project)) => {
                        form.model.adding = false;
                        let key = project.key.clone();
                        form.workspace.update(cx, |ws, cx| ws.added(project, cx));
                        cx.emit(AddProjectEvent::Added(key));
                    }
                    Ok(other) => form
                        .model
                        .add_failed(None, format!("unexpected answer: {other:?}")),
                    Err(e) => form.model.add_failed(e.refusal.as_deref(), e.message),
                }
                let error = form.model.key_server_error.is_some();
                form.key_input.update(cx, |i, cx| i.set_error(error, cx));
                cx.notify();
            });
        })
        .detach();
    }

    fn folder_row(&self, c: &Colors, cx: &mut Context<Self>) -> AnyElement {
        let m = &self.model;
        let (border, width, icon_color) = if m.folder_error.is_some() {
            (c.st_error, px(1.5), c.st_error)
        } else if m.probe.is_some() {
            (c.st_approved, px(1.5), c.st_approved)
        } else {
            (c.border_strong, px(1.), c.text_muted)
        };
        let path = match &m.folder {
            Some(f) => tilde(f),
            None => "No folder chosen yet".into(),
        };
        let choose = cx.listener(|form, _, _, cx| form.choose_folder(cx));
        let button = if self.mode == FormMode::Dialog && m.folder.is_some() {
            Button::new("change-folder", "Change…")
                .variant(Variant::Ghost)
                .on_click(choose)
        } else {
            Button::new("choose-folder", "Choose folder…")
                .variant(Variant::Secondary)
                .icon(IconName::FolderOpen)
                .on_click(choose)
        };
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(14.))
            .py(px(12.))
            .rounded(px(radius::LG))
            .bg(c.bg_raised)
            .border(width)
            .border_color(border)
            .child(icon(IconName::FolderGit2, px(18.)).text_color(icon_color))
            .child(
                div()
                    .flex_1()
                    .truncate()
                    .font_family(FONT_MONO)
                    .text_size(px(13.))
                    .text_color(if m.folder.is_some() {
                        c.text_primary
                    } else {
                        c.text_muted
                    })
                    .child(SharedString::from(path)),
            )
            .child(button)
            .into_any_element()
    }

    fn key_field(&self, c: &Colors, cx: &App) -> AnyElement {
        let projects = self.workspace.read(cx).switcher.projects();
        let error = self
            .model
            .probe
            .as_ref()
            .and_then(|_| self.model.key_error(projects));
        div()
            .flex()
            .flex_col()
            .gap(px(5.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.text_primary)
                            .child("Project key"),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(c.text_muted)
                            .child("prefilled from the repo name"),
                    ),
            )
            .child(div().w(px(160.)).child(self.key_input.clone()))
            .child(match error {
                Some(e) => div()
                    .text_size(px(11.))
                    .text_color(c.st_error)
                    .child(SharedString::from(e)),
                None => div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .child(icon(IconName::Info, px(12.)).text_color(c.text_muted))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(c.text_secondary)
                            .child(SharedString::from(self.model.key_hint())),
                    ),
            })
            .into_any_element()
    }

    fn detected_row(
        label: &'static str,
        icon_name: IconName,
        value: AnyElement,
        c: &Colors,
    ) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(icon(icon_name, px(13.)).text_color(c.text_muted))
            .child(
                div()
                    .w(px(100.))
                    .text_size(px(12.))
                    .text_color(c.text_secondary)
                    .child(label),
            )
            .child(div().flex_1().child(value))
            .into_any_element()
    }
}

impl gpui::Focusable for AddProjectForm {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for AddProjectForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let projects = self.workspace.read(cx).switcher.projects().to_vec();
        let can_add = self.model.can_add(&projects);
        let add = cx.listener(|form, _, _, cx| form.add(cx));
        let cancel = cx.listener(|_, _, _, cx| cx.emit(AddProjectEvent::Cancelled));
        let error_box = self.model.folder_error.clone().map(|e| {
            div()
                .flex()
                .gap(px(8.))
                .px(px(12.))
                .py(px(10.))
                .rounded(px(radius::LG))
                .bg(c.st_error_bg)
                .child(
                    icon(IconName::CircleX, px(13.))
                        .text_color(c.st_error)
                        .mt(px(2.)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.st_error)
                                .child(SharedString::from(e.title)),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(c.text_secondary)
                                .child(SharedString::from(e.detail)),
                        ),
                )
        });
        let detected =
            self.model.probe.clone().map(|p| {
                let mut rows = div().flex().flex_col().gap(px(8.));
                if self.mode == FormMode::FirstRun {
                    rows = rows.p(px(12.)).rounded(px(radius::LG)).bg(c.bg_app).child(
                        Self::detected_row(
                            "Name",
                            IconName::Tag,
                            div()
                                .font_family(FONT_MONO)
                                .text_size(px(12.))
                                .text_color(c.text_primary)
                                .child(SharedString::from(p.name.clone()))
                                .into_any_element(),
                            &c,
                        ),
                    );
                }
                rows.child(Self::detected_row(
                    "Base branch",
                    IconName::GitBranch,
                    div()
                        .w(px(220.))
                        .child(self.base_input.clone())
                        .into_any_element(),
                    &c,
                ))
            });
        let key = self.model.probe.as_ref().map(|_| self.key_field(&c, cx));
        let add_label = if self.model.adding {
            "Adding…"
        } else {
            "Add project"
        };
        let add_button = Button::new("add-project", add_label)
            .icon(IconName::Plus)
            .loading(self.model.adding)
            .disabled(!can_add)
            .on_click(add);
        let add_error = self.model.add_error.clone().map(|e| {
            div()
                .text_size(px(12.))
                .text_color(c.st_error)
                .child(SharedString::from(e))
        });
        let drop = cx.listener(|form, paths: &ExternalPaths, _, cx| {
            if let Some(path) = paths.paths().first() {
                form.pick(path.clone(), cx);
            }
        });
        match self.mode {
            FormMode::FirstRun => div()
                .id("add-first-project")
                .on_drop(drop)
                .w(px(600.))
                .flex()
                .flex_col()
                .gap(px(20.))
                .p(px(32.))
                .bg(c.bg_panel)
                .border_1()
                .border_color(c.border)
                .rounded(px(12.))
                .child(
                    div()
                        .text_size(px(22.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c.text_primary)
                        .child("Add your first project"),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .line_height(relative(1.45))
                        .text_color(c.text_secondary)
                        .child(SharedString::from(format!(
                            "Pick a folder that is a git repository. {} keeps everything in its own database; nothing is written into the repo.",
                            crewboard_core::naming::PRODUCT_NAME
                        ))),
                )
                .child(self.folder_row(&c, cx))
                .children(error_box)
                .children(detected)
                .children(key)
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(c.text_muted)
                        .child("Setup command, run command and env files can be added later in project settings."),
                )
                .children(add_error)
                .child(div().flex().child(div().flex_1()).child(add_button))
                .into_any_element(),
            FormMode::Dialog => div()
                .id("add-project-dialog")
                .key_context(CONTEXT)
                .track_focus(&self.focus)
                .on_action(cx.listener(|_, _: &CancelAddProject, _, cx| {
                    cx.emit(AddProjectEvent::Cancelled)
                }))
                .on_drop(drop)
                .w(px(460.))
                .flex()
                .flex_col()
                .overflow_hidden()
                .bg(c.bg_raised)
                .border_1()
                .border_color(c.border)
                .rounded(px(radius::DIALOG))
                .shadow(floating_shadow())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(14.))
                        .p(px(20.))
                        .child(
                            div()
                                .text_size(px(16.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.text_primary)
                                .child("Add project"),
                        )
                        .child(self.folder_row(&c, cx))
                        .children(error_box)
                        .children(key)
                        .children(detected)
                        .children(add_error),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(20.))
                        .py(px(12.))
                        .bg(c.bg_panel)
                        .border_t_1()
                        .border_color(c.border)
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(Kbd::text("Esc"))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(c.text_muted)
                                        .child("cancel"),
                                ),
                        )
                        .child(
                            Button::new("cancel-add-project", "Cancel")
                                .variant(Variant::Ghost)
                                .on_click(cancel),
                        )
                        .child(add_button),
                )
                .into_any_element(),
        }
    }
}
