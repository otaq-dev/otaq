//! Crewboard desktop app: the GPUI client of the background service (d0001, d0022).
//!
//! Closing the last window quits the app process; the background service and its agents keep running (ARC-03).

mod service_link;
mod view_models;

use gpui::{
    App, Bounds, Context, Entity, SharedString, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};

use service_link::ServiceLink;
use view_models::connection::{ConnectionViewModel, Tone};

struct Root {
    link: Entity<ServiceLink>,
}

impl Root {
    fn new(link: Entity<ServiceLink>, cx: &mut Context<Self>) -> Self {
        cx.observe(&link, |_, _, cx| cx.notify()).detach();
        Self { link }
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vm = ConnectionViewModel::new(&self.link.read(cx).state);
        let tone = match vm.tone {
            Tone::Calm => rgb(0x5B5F67),
            Tone::Attention => rgb(0xD4500A),
            Tone::Error => rgb(0xCC2F3B),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .gap_1()
            .bg(rgb(0xF6F6F4))
            .child(div().text_color(tone).child(SharedString::from(vm.label)))
            .children(vm.detail.map(|d| {
                div()
                    .text_sm()
                    .text_color(rgb(0x8C9099))
                    .child(SharedString::from(d))
            }))
    }
}

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a tokio runtime");
    let handle = runtime.handle().clone();

    gpui_platform::application().run(move |cx: &mut App| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let link = cx.new(|cx| ServiceLink::new(handle, cx));
        let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| Root::new(link, cx)),
        )
        .expect("the main window opens");
        cx.activate(true);
    });
    drop(runtime);
}
