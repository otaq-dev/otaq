//! Crewboard desktop app: the GPUI client of the background service (d0001, d0022).
//!
//! Closing the last window quits the app process; the background service and its agents keep running (ARC-03).

mod assets;
mod gallery;
mod icons;
mod service_link;
mod theme;
mod ui;
mod view_models;

use gpui::{
    App, Bounds, Context, Entity, SharedString, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, size,
};

use service_link::ServiceLink;
use theme::{ActiveTheme, ThemeChoice, text};
use ui::TextStyleExt;
use view_models::connection::{ConnectionViewModel, Tone};

struct Root {
    link: Entity<ServiceLink>,
}

impl Root {
    fn new(link: Entity<ServiceLink>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&link, |_, _, cx| cx.notify()).detach();
        cx.observe_window_appearance(window, |_, window, cx| {
            let choice = cx.global::<ActiveTheme>().choice;
            ActiveTheme::update(choice, window.appearance(), cx);
        })
        .detach();
        Self { link }
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vm = ConnectionViewModel::new(&self.link.read(cx).state);
        let colors = ActiveTheme::get(cx).colors;
        let tone = match vm.tone {
            Tone::Calm => colors.text_secondary,
            Tone::Attention => colors.st_attn,
            Tone::Error => colors.st_error,
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .gap_1()
            .bg(colors.bg_app)
            .font_family(theme::FONT_UI)
            .child(
                div()
                    .text_style_of(text::HEADER)
                    .text_color(tone)
                    .child(SharedString::from(vm.label)),
            )
            .children(vm.detail.map(|d| {
                div()
                    .text_style_of(text::METADATA)
                    .text_color(colors.text_muted)
                    .child(SharedString::from(d))
            }))
    }
}

fn open_window<V: Render + 'static>(
    cx: &mut App,
    width: f32,
    height: f32,
    build: impl FnOnce(&mut Window, &mut Context<V>) -> V + 'static,
) {
    let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| build(window, cx)),
    )
    .expect("a window opens");
}

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a tokio runtime");
    let handle = runtime.handle().clone();

    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            if let Err(e) = assets::load_fonts(cx) {
                eprintln!("can't load the bundled fonts: {e}");
            }
            ActiveTheme::init(ThemeChoice::System, cx.window_appearance(), cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let link = cx.new(|cx| ServiceLink::new(handle, cx));
            open_window(cx, 1440., 900., move |window, cx| {
                Root::new(link, window, cx)
            });
            if gallery::requested() {
                open_window(cx, 1280., 900., gallery::Gallery::new);
            }
            cx.activate(true);
        });
    drop(runtime);
}
