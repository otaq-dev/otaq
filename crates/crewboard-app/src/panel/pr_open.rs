//! Approved · PR open in the side panel (REV-13, REV-14; d0017, d0041; *05 → SHOP-26 · Approved, PR open*): the PR
//! box with the note that Crewboard doesn't watch PRs, Open PR in browser, Mark merged (toast "SHOP-26 merged · 2
//! waiting tickets started") and Mark closed (asks first).

use crewboard_core::domain::Status;
use crewboard_core::protocol::{CloseParams, KeyParams, Reply, Request};
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div,
    px,
};

use super::{PanelEvent, SidePanel};
use crate::board::Notice;
use crate::components::dialog::ConfirmDialog;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveTheme, FONT_MONO};
use crate::view_models::card::{age_label, pr_provider_label};
use crate::view_models::dialog::DialogKind;
use crate::view_models::pr_dialog as pd;

impl SidePanel {
    /// Open PR in browser.
    pub(super) fn open_pr_in_browser(&self, cx: &mut Context<Self>) {
        if let Some(url) = self.ticket(cx).and_then(|t| t.work.and_then(|w| w.pr_url)) {
            cx.open_url(&url);
        }
    }

    /// **Mark merged** (REV-13): Done · Merged; the base is fetched and waiting tickets start.
    pub fn mark_merged(&mut self, cx: &mut Context<Self>) {
        let key = self.key.clone();
        let send = self
            .link
            .read(cx)
            .send(Request::TicketMarkMerged(KeyParams {
                key: key.to_string(),
            }));
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |_, cx| {
                let notice = match result {
                    Ok(Reply::MarkedMerged(marked)) => Notice {
                        error: false,
                        title: pd::merged_toast(&key.to_string(), marked.started.len()),
                        text: None,
                    },
                    Ok(_) => return,
                    Err(e) => Notice {
                        error: true,
                        title: format!("Mark merged of {key} failed"),
                        text: Some(e.message),
                    },
                };
                cx.emit(PanelEvent::Toast(notice));
            });
        })
        .detach();
    }

    /// Mark closed asks first.
    pub fn ask_mark_closed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.mark_closed = Some(focus);
        cx.notify();
    }

    fn mark_closed(&mut self, cx: &mut Context<Self>) {
        self.mark_closed = None;
        let key = self.key.clone();
        let send = self
            .link
            .read(cx)
            .send(Request::TicketMarkClosed(CloseParams {
                key: key.to_string(),
                confirm: true,
            }));
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |_, cx| {
                let notice = match result {
                    Ok(_) => Notice {
                        error: false,
                        title: format!("{key} closed \u{b7} not merged"),
                        text: Some("The branch stays; you can reopen it later.".into()),
                    },
                    Err(e) => Notice {
                        error: true,
                        title: format!("Mark closed of {key} failed"),
                        text: Some(e.message),
                    },
                };
                cx.emit(PanelEvent::Toast(notice));
            });
        })
        .detach();
        cx.emit(PanelEvent::TakeFocus);
        cx.notify();
    }

    /// "Mark PR #481 as closed?".
    pub(super) fn mark_closed_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let focus = self.mark_closed.as_ref()?;
        let n = self.ticket(cx).and_then(|t| t.flags.pr_number)?;
        let this = cx.entity().downgrade();
        let cancel = this.clone();
        Some(
            ConfirmDialog::new(
                "mark-closed",
                focus,
                DialogKind::Destructive,
                format!("Mark PR #{n} as closed?"),
                "Mark closed",
                move |_, cx| {
                    let _ = this.update(cx, |p, cx| p.mark_closed(cx));
                },
                move |_, cx| {
                    let _ = cancel.update(cx, |p, cx| {
                        p.mark_closed = None;
                        cx.emit(PanelEvent::TakeFocus);
                        cx.notify();
                    });
                },
            )
            .text(
                "The ticket moves to Done as \u{201c}Closed \u{b7} not merged\u{201d}. The branch stays; nothing is \
                 pushed or deleted. You can reopen it later.",
            )
            .into_any_element(),
        )
    }

    /// The PR box above the tabs while the PR is open.
    pub(super) fn pr_section(
        &self,
        ticket: &crewboard_core::board::tickets::Ticket,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if ticket.status != Status::PrOpen {
            return None;
        }
        let n = ticket.flags.pr_number?;
        let c = ActiveTheme::get(cx).colors;
        let work = ticket.work.as_ref();
        let url = work.and_then(|w| w.pr_url.clone()).unwrap_or_default();
        let branch = work.and_then(|w| w.branch.clone()).unwrap_or_default();
        let target = match &ticket.base {
            crewboard_core::board::tickets::Base::Branch(b) => b.clone(),
            crewboard_core::board::tickets::Base::Epic(e) => e.to_string(),
        };
        let provider = pr_provider_label(ticket);
        let opened = ticket
            .status_since
            .map(|since| age_label(super::now_ms() - since))
            .unwrap_or_default();
        Some(
            div()
                .mx(px(16.))
                .my(px(8.))
                .p(px(12.))
                .rounded(px(8.))
                .bg(c.st_approved_bg)
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.text_primary)
                                .child(icon(IconName::GitPullRequest, px(13.)))
                                .child(SharedString::from(pd::pr_label(&url, n))),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(c.text_muted)
                                .child(SharedString::from(format!("opened {opened} ago"))),
                        ),
                )
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(11.))
                        .text_color(c.text_secondary)
                        .child(SharedString::from(format!("{branch} \u{2192} {target}"))),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(icon(IconName::Info, px(12.)).text_color(c.text_muted))
                        .child(div().text_size(px(12.)).text_color(c.text_secondary).child(
                            SharedString::from(pd::pr_open_note(provider.as_deref(), &target)),
                        )),
                )
                .into_any_element(),
        )
    }
}
