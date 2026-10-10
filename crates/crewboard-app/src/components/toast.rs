//! Toast (*01 · Components → Overlays & feedback*): a 340 px floating card with a coloured icon, a title, a line of
//! text, an optional action link and a close button. The toast host (stacking, time-outs) is `crate::toasts`
//! (T-0017).

use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, Hsla, IntoElement, RenderOnce, SharedString, Window, div,
    prelude::*, px,
};

use super::floating_shadow;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, FONT_UI, Theme, radius};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
}

impl ToastKind {
    /// The default icon and its colour; a toast may pick another icon (git-merge, sparkles…).
    pub fn default_icon(self) -> IconName {
        match self {
            Self::Success => IconName::CircleCheck,
            Self::Error => IconName::TriangleAlert,
        }
    }

    pub fn color(self, t: &Theme) -> Hsla {
        match self {
            Self::Success => t.colors.st_approved,
            Self::Error => t.colors.st_error,
        }
    }
}

type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Toast {
    id: ElementId,
    kind: ToastKind,
    icon: Option<(IconName, Option<Hsla>)>,
    title: SharedString,
    text: Option<SharedString>,
    action: Option<(SharedString, Handler)>,
    on_close: Option<Handler>,
}

impl Toast {
    pub fn new(id: impl Into<ElementId>, kind: ToastKind, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            kind,
            icon: None,
            title: title.into(),
            text: None,
            action: None,
            on_close: None,
        }
    }

    /// Another icon, optionally in another colour than the kind's.
    pub fn icon(mut self, icon: IconName, color: Option<Hsla>) -> Self {
        self.icon = Some((icon, color));
        self
    }

    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// A link after the text, like "Copy command" on "gh is not logged in".
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(handler)));
        self
    }

    pub fn on_close(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Toast {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = ActiveTheme::get(cx);
        let c = t.colors;
        let (name, color) = match self.icon {
            Some((name, color)) => (name, color.unwrap_or_else(|| self.kind.color(t))),
            None => (self.kind.default_icon(), self.kind.color(t)),
        };
        let mut body = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(3.))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(c.text_primary)
                    .child(self.title),
            );
        if self.text.is_some() || self.action.is_some() {
            let mut line = div()
                .flex()
                .flex_wrap()
                .gap(px(4.))
                .text_size(px(12.))
                .text_color(c.text_secondary)
                .children(self.text);
            if let Some((label, handler)) = self.action {
                line = line.child(
                    div()
                        .id("action")
                        .cursor_pointer()
                        .text_color(c.accent)
                        .child(label)
                        .on_click(move |e, w, cx| handler(e, w, cx)),
                );
            }
            body = body.child(line);
        }
        div()
            .id(self.id)
            .occlude()
            .flex()
            .gap(px(10.))
            .w(px(340.))
            .px(px(12.))
            .py(px(10.))
            .rounded(px(radius::LG))
            .bg(c.bg_raised)
            .border_1()
            .border_color(c.border)
            .shadow(floating_shadow())
            .font_family(FONT_UI)
            .child(icon(name, px(15.)).text_color(color).mt(px(1.)))
            .child(body)
            .children(self.on_close.map(|handler| {
                div()
                    .id("close")
                    .flex_none()
                    .cursor_pointer()
                    .child(icon(IconName::X, px(13.)).text_color(c.text_muted))
                    .on_click(move |e, w, cx| handler(e, w, cx))
            }))
    }
}
