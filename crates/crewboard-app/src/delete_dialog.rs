//! The reusable delete confirmation (TKT-08; d0060, d0018): give it a ticket key; it asks the service what the
//! delete would remove (`ticket.delete_preview`), shows the confirmation, and deletes only on the confirm
//! click. A refusal from the service is shown in the dialog, never swallowed. Used by the card menu (T-0162),
//! the panel's "…" menu (T-0060) and the Project terminal's summary (T-0167).

use crewboard_core::board::delete::DeletePreview;
use crewboard_core::protocol::{KeyParams, Reply, Request};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, IntoElement, SharedString, Window, div,
    prelude::*, px,
};

use crate::components::dialog::ConfirmDialog;
use crate::icons::IconName;
use crate::service_link::ServiceLink;
use crate::theme::ActiveTheme;
use crate::view_models::delete::text;
use crate::view_models::dialog::DialogKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteOutcome {
    /// Deleted; what was removed.
    Deleted(DeletePreview),
    Cancelled,
}

pub struct DeleteDialog {
    link: Entity<ServiceLink>,
    key: String,
    preview: Option<DeletePreview>,
    error: Option<String>,
    deleting: bool,
    focus: FocusHandle,
}

impl EventEmitter<DeleteOutcome> for DeleteDialog {}

impl DeleteDialog {
    pub fn new(
        link: Entity<ServiceLink>,
        key: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let request = link
            .read(cx)
            .send(Request::TicketDeletePreview(KeyParams { key: key.clone() }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |d, cx| {
                match result {
                    Ok(Reply::DeletePreview(p)) => d.preview = Some(p),
                    Ok(other) => d.error = Some(format!("unexpected answer: {other:?}")),
                    Err(e) => d.error = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
        Self {
            link,
            key,
            preview: None,
            error: None,
            deleting: false,
            focus,
        }
    }

    fn delete(&mut self, cx: &mut Context<Self>) {
        if self.deleting || self.preview.is_none() {
            return;
        }
        self.deleting = true;
        cx.notify();
        let request = self.link.read(cx).send(Request::TicketDelete(KeyParams {
            key: self.key.clone(),
        }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |d, cx| {
                d.deleting = false;
                match result {
                    Ok(Reply::DeletePreview(p)) => cx.emit(DeleteOutcome::Deleted(p)),
                    Ok(other) => d.error = Some(format!("unexpected answer: {other:?}")),
                    Err(e) => d.error = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for DeleteDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let (title, body, confirm) = match &self.preview {
            Some(p) => {
                let t = text(p);
                (t.title, t.body, t.confirm)
            }
            None => (
                format!("Delete {}?", self.key),
                Vec::new(),
                "Delete ticket".to_owned(),
            ),
        };
        let this = cx.entity().downgrade();
        let cancel = cx.entity().downgrade();
        let mut dialog = ConfirmDialog::new(
            "delete-ticket",
            &self.focus,
            DialogKind::Destructive,
            title,
            confirm,
            move |_: &mut Window, cx: &mut App| {
                let _ = this.update(cx, |d, cx| d.delete(cx));
            },
            move |_: &mut Window, cx: &mut App| {
                let _ = cancel.update(cx, |_, cx| cx.emit(DeleteOutcome::Cancelled));
            },
        )
        .confirm_icon(IconName::Trash2)
        .confirm_disabled(self.preview.is_none() || self.deleting);
        if !body.is_empty() {
            dialog = dialog.text(body.join(" "));
        }
        if let Some(e) = &self.error {
            dialog = dialog.child(
                div()
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .bg(c.st_error_bg)
                    .text_size(px(12.))
                    .text_color(c.st_error)
                    .child(SharedString::from(e.clone())),
            );
        }
        dialog
    }
}
