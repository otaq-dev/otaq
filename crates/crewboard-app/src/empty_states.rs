//! The board's empty states (UI-08; *02 · App shell → No projects yet*, *14 · First run → 4 · Done · empty
//! board*): no project at all, and a project with no tickets yet.

use std::rc::Rc;

use gpui::{
    App, FontWeight, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px, relative,
};

use crate::components::button::{Button, Variant};
use crate::components::kbd::Kbd;
use crate::icons::{IconName, icon};
use crate::keymap::{KeyAction, Platform};
use crate::shell::tilde;
use crate::theme::ActiveTheme;

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// "No projects yet": add a folder (or drop one on the window), or check the tools again.
#[derive(IntoElement)]
pub struct NoProjects {
    on_add: Handler,
    on_check_tools: Handler,
}

impl NoProjects {
    pub fn new(
        on_add: impl Fn(&mut Window, &mut App) + 'static,
        on_check_tools: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            on_add: Rc::new(on_add),
            on_check_tools: Rc::new(on_check_tools),
        }
    }
}

impl RenderOnce for NoProjects {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let add = self.on_add.clone();
        let tools = self.on_check_tools.clone();
        centered()
            .child(
                div()
                    .size(px(48.))
                    .rounded(px(12.))
                    .bg(c.bg_column)
                    .flex()
                    .justify_center()
                    .items_center()
                    .child(icon(IconName::FolderGit2, px(22.)).text_color(c.text_secondary)),
            )
            .child(title("No projects yet", &c))
            .child(paragraph(
                format!(
                    "A project is any folder that is a git repository. {} never writes into it — all settings stay in the app.",
                    crewboard_core::naming::PRODUCT_NAME
                ),
                420.,
                &c,
            ))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        Button::new("empty-add-project", "Add project folder…")
                            .icon(IconName::FolderPlus)
                            .on_click(move |_, window, cx| add(window, cx)),
                    )
                    .child(
                        Button::new("empty-check-tools", "Check tools")
                            .variant(Variant::Secondary)
                            .icon(IconName::ListChecks)
                            .on_click(move |_, window, cx| tools(window, cx)),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(c.text_muted)
                    .child("or drop a folder anywhere in this window"),
            )
    }
}

/// "shop-web is ready": write the first ticket, or open the Project terminal.
#[derive(IntoElement)]
pub struct EmptyBoard {
    name: String,
    repo_path: String,
    on_new_ticket: Handler,
}

impl EmptyBoard {
    pub fn new(
        name: impl Into<String>,
        repo_path: impl Into<String>,
        on_new_ticket: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            repo_path: repo_path.into(),
            on_new_ticket: Rc::new(on_new_ticket),
        }
    }
}

impl RenderOnce for EmptyBoard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let platform = Platform::current();
        let new_ticket = self.on_new_ticket.clone();
        let hint = |keys: Option<String>, label: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .children(keys.map(Kbd::text))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child(label),
                )
        };
        centered()
            .child(title(&format!("{} is ready", self.name), &c))
            .child(paragraph(
                format!(
                    "Write a ticket for the first thing an agent should do — or open the Project terminal (a shell in {}) and run claude to split a feature into tickets.",
                    tilde(&self.repo_path)
                ),
                440.,
                &c,
            ))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        Button::new("empty-new-ticket", "New ticket")
                            .icon(IconName::Plus)
                            .on_click(move |_, window, cx| new_ticket(window, cx)),
                    )
                    // The Project terminal arrives with T-0138; the button shows where it will be.
                    .child(
                        Button::new("empty-project-terminal", "Project terminal")
                            .variant(Variant::Secondary)
                            .icon(IconName::SquareTerminal)
                            .disabled(true),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(14.))
                    .child(hint(KeyAction::NewTicket.keys(platform), "new ticket"))
                    .child(hint(
                        KeyAction::ProjectTerminal.keys(platform),
                        "project terminal",
                    )),
            )
    }
}

fn centered() -> gpui::Div {
    div()
        .flex_1()
        .size_full()
        .flex()
        .flex_col()
        .justify_center()
        .items_center()
        .gap(px(14.))
        .p(px(32.))
}

fn title(text: &str, c: &crate::theme::Colors) -> impl IntoElement {
    div()
        .text_size(px(20.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c.text_primary)
        .child(SharedString::from(text.to_owned()))
}

fn paragraph(text: String, width: f32, c: &crate::theme::Colors) -> impl IntoElement {
    div()
        .w(px(width))
        .text_center()
        .text_size(px(13.))
        .line_height(relative(1.45))
        .text_color(c.text_secondary)
        .child(SharedString::from(text))
}
