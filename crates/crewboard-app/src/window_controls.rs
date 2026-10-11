//! The window's own chrome on Windows and Linux (UI-01, T-0159; *02 · App shell → Title bar · Windows & Linux*):
//! the header is the title bar, with the caption buttons (minimize, maximize, close; 40 × 32 hit areas) at its
//! right end. On Windows the buttons are the window's control areas, so the system handles them (snap layouts
//! included). On Linux the window draws its own decorations when the desktop asks it to (Wayland without server
//! decorations): the buttons act through gpui, and the window's edges resize it. macOS keeps its traffic lights.

use gpui::{
    AnyElement, CursorStyle, Decorations, MouseButton, ResizeEdge, Window, WindowControlArea,
    WindowDecorations, div, prelude::*, px,
};

use crate::icons::{IconName, icon};
use crate::theme::Colors;

/// How the window is decorated on this platform: the app's own title bar everywhere; on Linux client-side
/// decorations (the desktop may still draw its own, then no caption buttons are shown).
pub fn window_decorations() -> Option<WindowDecorations> {
    cfg!(target_os = "linux").then_some(WindowDecorations::Client)
}

/// Whether the header carries the caption buttons: always on Windows, on Linux while the app draws the
/// decorations, never on macOS.
pub fn shows_caption_buttons(window: &Window) -> bool {
    if cfg!(target_os = "macos") {
        return false;
    }
    cfg!(windows) || matches!(window.window_decorations(), Decorations::Client { .. })
}

/// Empty space that moves the window: a system drag area on Windows, a window move elsewhere; a double click
/// maximizes (zooms on macOS), as the system title bar would.
pub fn drag_area(id: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_1()
        .h_full()
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down(MouseButton::Left, |e, window, _| {
            if e.click_count >= 2 {
                if cfg!(target_os = "macos") {
                    window.titlebar_double_click();
                } else {
                    window.zoom_window();
                }
            } else if !cfg!(windows) {
                window.start_window_move();
            }
        })
}

#[derive(Clone, Copy)]
enum Caption {
    Minimize,
    Maximize,
    Close,
}

/// The divider and the three caption buttons at the header's right end (frame: `| – □ ×`).
pub fn caption_buttons(c: &Colors) -> AnyElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .child(div().w(px(1.)).h(px(20.)).bg(c.border))
        .child(
            div().flex().h(px(32.)).children(
                [Caption::Minimize, Caption::Maximize, Caption::Close].map(|b| button(b, c)),
            ),
        )
        .into_any_element()
}

fn button(caption: Caption, c: &Colors) -> AnyElement {
    let (id, name, size, area) = match caption {
        Caption::Minimize => (
            "caption-minimize",
            IconName::Minus,
            14.,
            WindowControlArea::Min,
        ),
        Caption::Maximize => (
            "caption-maximize",
            IconName::Square,
            11.,
            WindowControlArea::Max,
        ),
        Caption::Close => ("caption-close", IconName::X, 14., WindowControlArea::Close),
    };
    let close = matches!(caption, Caption::Close);
    let hover_bg = if close {
        gpui::rgb(0xE81120).into()
    } else {
        c.bg_hover
    };
    let hover_fg = if close { gpui::white() } else { c.text_primary };
    let fg = c.text_secondary;
    div()
        .id(id)
        .w(px(40.))
        .h(px(32.))
        .flex()
        .justify_center()
        .items_center()
        .occlude()
        .text_color(fg)
        .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
        .window_control_area(area)
        .child(icon(name, px(size)).text_color(fg))
        // Windows handles the control areas itself; elsewhere the click acts.
        .when(!cfg!(windows), |el| {
            el.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |_, window, _| match caption {
                    Caption::Minimize => window.minimize_window(),
                    Caption::Maximize => window.zoom_window(),
                    Caption::Close => window.remove_window(),
                })
        })
        .into_any_element()
}

/// How far from the window's edge a press resizes it, while the app draws the decorations.
const RESIZE_BORDER: f32 = 5.;

/// The window's edges and corners as resize handles, while the app draws its own decorations on Linux; nothing
/// otherwise. Drawn over the content, on top.
pub fn resize_handles(window: &Window) -> Option<AnyElement> {
    let Decorations::Client { tiling } = window.window_decorations() else {
        return None;
    };
    if cfg!(windows) || !window.is_resizable() || window.is_maximized() {
        return None;
    }
    let b = px(RESIZE_BORDER);
    let handle = |id: &'static str, edge: ResizeEdge, cursor: CursorStyle| {
        div().id(id).absolute().cursor(cursor).on_mouse_down(
            MouseButton::Left,
            move |_, window, cx| {
                cx.stop_propagation();
                window.start_window_resize(edge);
            },
        )
    };
    let mut handles: Vec<AnyElement> = Vec::new();
    if !tiling.top {
        handles.push(
            handle("resize-top", ResizeEdge::Top, CursorStyle::ResizeUpDown)
                .top_0()
                .left(b)
                .right(b)
                .h(b)
                .into_any_element(),
        );
    }
    if !tiling.bottom {
        handles.push(
            handle(
                "resize-bottom",
                ResizeEdge::Bottom,
                CursorStyle::ResizeUpDown,
            )
            .bottom_0()
            .left(b)
            .right(b)
            .h(b)
            .into_any_element(),
        );
    }
    if !tiling.left {
        handles.push(
            handle(
                "resize-left",
                ResizeEdge::Left,
                CursorStyle::ResizeLeftRight,
            )
            .left_0()
            .top(b)
            .bottom(b)
            .w(b)
            .into_any_element(),
        );
    }
    if !tiling.right {
        handles.push(
            handle(
                "resize-right",
                ResizeEdge::Right,
                CursorStyle::ResizeLeftRight,
            )
            .right_0()
            .top(b)
            .bottom(b)
            .w(b)
            .into_any_element(),
        );
    }
    if !tiling.top && !tiling.left {
        handles.push(
            handle(
                "resize-top-left",
                ResizeEdge::TopLeft,
                CursorStyle::ResizeUpLeftDownRight,
            )
            .top_0()
            .left_0()
            .size(b)
            .into_any_element(),
        );
    }
    if !tiling.top && !tiling.right {
        handles.push(
            handle(
                "resize-top-right",
                ResizeEdge::TopRight,
                CursorStyle::ResizeUpRightDownLeft,
            )
            .top_0()
            .right_0()
            .size(b)
            .into_any_element(),
        );
    }
    if !tiling.bottom && !tiling.left {
        handles.push(
            handle(
                "resize-bottom-left",
                ResizeEdge::BottomLeft,
                CursorStyle::ResizeUpRightDownLeft,
            )
            .bottom_0()
            .left_0()
            .size(b)
            .into_any_element(),
        );
    }
    if !tiling.bottom && !tiling.right {
        handles.push(
            handle(
                "resize-bottom-right",
                ResizeEdge::BottomRight,
                CursorStyle::ResizeUpLeftDownRight,
            )
            .bottom_0()
            .right_0()
            .size(b)
            .into_any_element(),
        );
    }
    Some(
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .children(handles)
            .into_any_element(),
    )
}
