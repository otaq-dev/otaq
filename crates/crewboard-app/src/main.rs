//! Crewboard desktop app: the GPUI client of the background service (d0001, d0022).
//!
//! Closing the last window quits the app process; the background service and its agents keep running (ARC-03).

mod assets;
mod components;
mod gallery;
mod gallery_components;
mod icons;
mod keymap;
mod onboarding;
mod root;
mod service_link;
mod theme;
mod ui;
mod view_models;

use gpui::{
    App, Bounds, Context, TitlebarOptions, Window, WindowBounds, WindowOptions, point, prelude::*,
    px, size,
};

use root::Root;
use service_link::ServiceLink;
use theme::{ActiveTheme, ThemeChoice};

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
            // The app draws its own 44 px title bar; the traffic lights sit in it.
            titlebar: Some(TitlebarOptions {
                title: Some(crewboard_core::naming::PRODUCT_NAME.into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(14.), px(16.))),
            }),
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
            root::bind_keys(cx);
            components::bind_keys(cx);
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
