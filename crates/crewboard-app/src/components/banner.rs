//! Banner (*01 · Components → Banners → Banner*): a full-width 36 px strip with an icon, a title in the banner's
//! colour, the text, optional compact secondary buttons and an optional close button. The banner area and its
//! one-banner-per-situation rules are in `crate::banners` (T-0017).

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, Hsla, IntoElement, RenderOnce, SharedString, Window,
    div, prelude::*, px,
};

use super::button::{Button, Variant};
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, FONT_UI};
use crate::ui::TextStyleExt;

/// The banner's colour family (fill, bottom border, icon and title colour).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerTone {
    /// Orange: the usage limit.
    Attention,
    /// Amber: warnings (permission prompts, restart required).
    Warning,
    /// Accent: calm news (service update ready).
    Info,
    /// Red: something broke and waits for a click (service stopped).
    Error,
    /// Grey, not alarming: reconnecting.
    Neutral,
}

impl BannerTone {
    /// (icon and title colour, fill, bottom border).
    fn colors(self, cx: &App) -> (Hsla, Hsla, Hsla) {
        let c = ActiveTheme::get(cx).colors;
        match self {
            Self::Attention => (c.st_attn, c.st_attn_bg, c.st_attn),
            Self::Warning => (c.st_paused, c.st_paused_bg, c.st_paused),
            Self::Info => (c.accent, c.accent_soft, c.accent),
            Self::Error => (c.st_error, c.st_error_bg, c.st_error),
            Self::Neutral => (c.text_secondary, c.bg_panel, c.border),
        }
    }
}

type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

pub struct BannerAction {
    pub label: SharedString,
    pub icon: Option<IconName>,
    pub disabled: bool,
    /// Highlighted: the banner's call to action (Resume all after the usage limit reset).
    pub primary: bool,
    pub on_click: Handler,
}

impl BannerAction {
    pub fn new(
        label: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            icon: None,
            disabled: false,
            primary: false,
            on_click: Rc::new(on_click),
        }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn primary(mut self, primary: bool) -> Self {
        self.primary = primary;
        self
    }
}

#[derive(IntoElement)]
pub struct Banner {
    id: ElementId,
    tone: BannerTone,
    icon: IconName,
    title: SharedString,
    text: SharedString,
    actions: Vec<BannerAction>,
    on_dismiss: Option<Handler>,
}

impl Banner {
    pub fn new(
        id: impl Into<ElementId>,
        tone: BannerTone,
        icon: IconName,
        title: impl Into<SharedString>,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            tone,
            icon,
            title: title.into(),
            text: text.into(),
            actions: Vec::new(),
            on_dismiss: None,
        }
    }

    pub fn action(mut self, action: BannerAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Shows the close button; without this the banner can't be dismissed.
    pub fn on_dismiss(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Banner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (fg, bg, line) = self.tone.colors(cx);
        let c = ActiveTheme::get(cx).colors;
        let actions: Vec<AnyElement> = self
            .actions
            .into_iter()
            .enumerate()
            .map(|(i, a)| {
                let handler = a.on_click.clone();
                let mut b = Button::new(("action", i), a.label)
                    .variant(if a.primary {
                        Variant::Primary
                    } else {
                        Variant::Secondary
                    })
                    .compact()
                    .disabled(a.disabled)
                    .on_click(move |e, w, cx| handler(e, w, cx));
                if let Some(i) = a.icon {
                    b = b.icon(i);
                }
                b.into_any_element()
            })
            .collect();
        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(px(10.))
            .w_full()
            .h(px(36.))
            .px(px(16.))
            .bg(bg)
            .border_b(px(1.))
            .border_color(line)
            .font_family(FONT_UI)
            .child(icon(self.icon, px(15.)).text_color(fg))
            .child(
                div()
                    .flex_none()
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(fg)
                    .child(self.title),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_style_of(crate::theme::text::BODY)
                    .line_height(px(18.))
                    .text_color(c.text_primary)
                    .child(self.text),
            )
            .children(actions)
            .children(self.on_dismiss.map(|handler| {
                div()
                    .id("dismiss")
                    .flex_none()
                    .cursor_pointer()
                    .child(icon(IconName::X, px(14.)).text_color(c.text_muted))
                    .on_click(move |e, w, cx| handler(e, w, cx))
            }))
    }
}
