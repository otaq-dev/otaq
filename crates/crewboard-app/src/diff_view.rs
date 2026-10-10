//! The Diff tab (REV-01, REV-02; d0017, brief 07): the agent's summary (foldable), the base indicator ("diff vs
//! `main` at a1b2c3d"), the file list with status letters, counts and viewed ticks, and the diff itself with
//! syntax highlighting. Files are fetched one at a time as they scroll into view, so a large diff stays fast; the
//! data is local, so nothing shows a spinner. Tickets that aren't in Ready for Review show it read-only.

use std::collections::HashMap;

use crewboard_core::board::comments::{NewComment, ReviewComment};
use crewboard_core::board::diff::DiffSummary;
use crewboard_core::domain::TicketKey;
use crewboard_core::git::diff::{ChangedFile, DiffLine, FileDiff, FileStatus, LineKind};
use crewboard_core::protocol::{
    CommentIdParams, CommentTextParams, CommentsParams, DiffFileParams, Event, KeyParams, Reply,
    Request,
};
use gpui::{
    AnyElement, Context, Entity, Focusable, FontWeight, HighlightStyle, Hsla, IntoElement,
    ListAlignment, ListState, ParentElement, Render, SharedString, Styled, StyledText, Window, div,
    list, prelude::*, px,
};

use crate::components::button::{Button, Variant};
use crate::components::tabs::Segmented;
use crate::components::text_input::{InputEvent, TextInput};
use crate::highlight::{self, Spans, Token};
use crate::icons::{IconName, icon};
use crate::service_link::{ServiceEvent, ServiceLink};
use crate::theme::{ActiveTheme, Colors, FONT_MONO};
use crate::view_models::comments::{self as cm, Anchor, Composer};
use crate::view_models::diff::{self as vm, DiffModel, FileLoad, SideRow};
use crate::workspace::Workspace;

/// One diff line's height (the frame's 20 px rows).
const LINE_HEIGHT: f32 = 20.;
/// The file list's width (narrower in a narrow panel).
const FILE_LIST: f32 = 280.;
const FILE_LIST_NARROW: f32 = 200.;
/// Below this width the file list narrows.
const NARROW_BELOW: f32 = 820.;

pub struct DiffView {
    workspace: Entity<Workspace>,
    link: Entity<ServiceLink>,
    key: TicketKey,
    model: DiffModel,
    list: ListState,
    /// Highlighted spans per file, per hunk, per line, made once when the file arrives.
    spans: HashMap<String, Vec<Vec<Spans>>>,
    /// The panel's width, for the file list and Side-by-side.
    pub width: f32,
    /// "Filter 54 files" above a long file list.
    filter: Entity<TextInput>,
    /// Not in Ready for Review (Done, Dev): no comment affordances.
    pub read_only: bool,
    /// The ticket's review comments, every round (REV-03).
    comments: Vec<ReviewComment>,
    /// The open comment box and its text.
    composer: Option<Composer>,
    composer_input: Entity<TextInput>,
    /// A comment request is on its way (one click, one comment).
    saving: bool,
}

impl DiffView {
    pub fn new(
        workspace: Entity<Workspace>,
        key: TicketKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let link = workspace.read(cx).link.clone();
        cx.subscribe_in(&link, window, |view, _, event: &ServiceEvent, _, cx| {
            match event {
                // A new review round or a merge moves the branch: read the file list again.
                ServiceEvent::Event(Event::CardStatusChanged { key, .. }) if *key == view.key => {
                    view.load(cx)
                }
                // Comments changed (here or in another window).
                ServiceEvent::Event(Event::TicketChanged { key, .. }) if *key == view.key => {
                    view.load_comments(cx)
                }
                ServiceEvent::Resync => view.load(cx),
                _ => {}
            }
        })
        .detach();
        let filter = cx.new(|cx| TextInput::new(window, cx).placeholder("Filter files"));
        cx.subscribe(&filter, |_, _, event: &InputEvent, cx| {
            if *event == InputEvent::Changed {
                cx.notify();
            }
        })
        .detach();
        let composer_input = cx.new(|cx| {
            TextInput::new(window, cx)
                .multiline(px(64.))
                .placeholder("Leave a comment")
        });
        cx.subscribe_in(
            &composer_input,
            window,
            |view, _, event: &InputEvent, window, cx| match event {
                InputEvent::Submit => view.save_composer(cx),
                InputEvent::Cancel => view.cancel_composer(window, cx),
                _ => {}
            },
        )
        .detach();
        let mut view = Self {
            read_only: true,
            comments: Vec::new(),
            composer: None,
            composer_input,
            saving: false,
            filter,
            workspace,
            link,
            key,
            model: DiffModel::default(),
            list: ListState::new(0, ListAlignment::Top, px(600.)),
            spans: HashMap::new(),
            width: crate::view_models::panel::DEFAULT_WIDTH,
        };
        view.load(cx);
        view.load_comments(cx);
        view
    }

    /// The tab's "+412 −38".
    pub fn indicator(&self) -> Option<String> {
        vm::tab_indicator(self.model.summary.as_ref())
    }

    /// "Send back (2)" and the toolbar's "2 pending comments".
    pub fn pending(&self) -> usize {
        cm::pending(&self.comments)
    }

    /// The comments Send back would send now.
    pub fn pending_comments(&self) -> Vec<ReviewComment> {
        self.comments
            .iter()
            .filter(|c| c.is_pending())
            .cloned()
            .collect()
    }

    fn load_comments(&mut self, cx: &mut Context<Self>) {
        let key = self.key.clone();
        let request = self
            .link
            .read(cx)
            .send(Request::ReviewComments(CommentsParams {
                key: key.to_string(),
                round: None,
            }));
        cx.spawn(async move |this, cx| {
            if let Ok(Reply::ReviewComments(list)) = request.await {
                let _ = this.update(cx, |view, cx| {
                    if view.key == key {
                        view.comments = list.comments;
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    /// Opens the comment box at `anchor` (a new comment or a reply), or on a pending comment to edit it.
    fn open_composer(
        &mut self,
        anchor: Anchor,
        editing: Option<&ReviewComment>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let text = editing.map(|c| c.body.clone()).unwrap_or_default();
        self.composer = Some(Composer {
            anchor,
            editing: editing.map(|c| c.id.clone()),
        });
        self.composer_input.update(cx, |input, cx| {
            input.set_text(&text, cx);
        });
        let focus = self.composer_input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    fn cancel_composer(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.composer = None;
        cx.notify();
    }

    /// Save (or ⌘↵): adds the comment, or saves the edit; the list comes back through the ticket's change event.
    fn save_composer(&mut self, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.clone() else {
            return;
        };
        let body = self.composer_input.read(cx).text().trim().to_owned();
        if body.is_empty() || self.saving {
            return;
        }
        let request = match &composer.editing {
            Some(id) => Request::ReviewCommentEdit(CommentTextParams {
                id: id.clone(),
                body,
            }),
            None => Request::ReviewCommentAdd(NewComment {
                key: self.key.to_string(),
                file: composer.anchor.file.clone(),
                line: composer.anchor.line,
                side: composer.anchor.side,
                body,
            }),
        };
        self.saving = true;
        let send = self.link.read(cx).send(request);
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |view, cx| {
                view.saving = false;
                match result {
                    Ok(_) => {
                        view.composer = None;
                        view.load_comments(cx);
                    }
                    Err(e) => eprintln!("diff: comment not saved: {e}"),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn delete_comment(&mut self, id: String, cx: &mut Context<Self>) {
        let send = self
            .link
            .read(cx)
            .send(Request::ReviewCommentDelete(CommentIdParams { id }));
        cx.spawn(async move |this, cx| {
            let result = send.await;
            let _ = this.update(cx, |view, cx| {
                if let Err(e) = result {
                    eprintln!("diff: comment not deleted: {e}");
                }
                view.load_comments(cx);
            });
        })
        .detach();
    }

    /// The comments on one line, and the comment box when it is open there.
    fn thread_block(&self, anchor: &Anchor, cx: &mut Context<Self>) -> Option<AnyElement> {
        let thread: Vec<ReviewComment> = cm::thread(&self.comments, anchor)
            .into_iter()
            .cloned()
            .collect();
        let composing = self
            .composer
            .as_ref()
            .is_some_and(|comp| comp.anchor == *anchor);
        if thread.is_empty() && !composing {
            return None;
        }
        let c = ActiveTheme::get(cx).colors;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        let mut cards: Vec<AnyElement> = Vec::new();
        for comment in &thread {
            let edit = comment.clone();
            let delete_id = comment.id.clone();
            let anchor_for_edit = anchor.clone();
            let editable = cm::editable(comment) && !self.read_only;
            cards.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .p(px(10.))
                    .rounded(px(6.))
                    .bg(c.bg_raised)
                    .border_1()
                    .border_color(c.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                div()
                                    .size(px(16.))
                                    .rounded_full()
                                    .bg(c.accent_soft)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(10.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c.accent)
                                    .child("Y"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c.text_primary)
                                    .child("You"),
                            )
                            .child(div().text_size(px(11.)).text_color(c.text_muted).child(
                                format!(
                                    "{} ago",
                                    crate::view_models::card::age_label(now - comment.created_at)
                                ),
                            ))
                            .child(
                                div()
                                    .px(px(6.))
                                    .rounded(px(4.))
                                    .bg(if comment.is_pending() {
                                        c.accent_soft
                                    } else {
                                        c.bg_column
                                    })
                                    .text_size(px(10.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if comment.is_pending() {
                                        c.accent
                                    } else {
                                        c.text_muted
                                    })
                                    .child(cm::badge(comment)),
                            )
                            .child(div().flex_1())
                            .when(editable, |d| {
                                d.child(
                                    div()
                                        .id(SharedString::from(format!("comment-edit-{}", edit.id)))
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |view, _, window, cx| {
                                            view.open_composer(
                                                anchor_for_edit.clone(),
                                                Some(&edit),
                                                window,
                                                cx,
                                            )
                                        }))
                                        .child(
                                            icon(IconName::Pencil, px(12.))
                                                .text_color(c.text_muted),
                                        ),
                                )
                                .child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "comment-delete-{delete_id}"
                                        )))
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            view.delete_comment(delete_id.clone(), cx)
                                        }))
                                        .child(icon(IconName::X, px(12.)).text_color(c.text_muted)),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(c.text_primary)
                            .child(SharedString::from(comment.body.clone())),
                    )
                    .into_any_element(),
            );
        }
        if composing {
            let editing = self
                .composer
                .as_ref()
                .is_some_and(|comp| comp.editing.is_some());
            cards.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .p(px(10.))
                    .rounded(px(6.))
                    .bg(c.bg_raised)
                    .border_1()
                    .border_color(c.accent)
                    .child(self.composer_input.clone())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(11.))
                                    .text_color(c.text_muted)
                                    .child("Markdown \u{b7} comments are sent to the agent only on Send back"),
                            )
                            .child(
                                Button::new("comment-cancel", "Cancel")
                                    .variant(Variant::Ghost)
                                    .compact()
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.cancel_composer(window, cx)
                                    })),
                            )
                            .child(
                                Button::new(
                                    "comment-save",
                                    if editing { "Save" } else { "Add comment" },
                                )
                                .variant(Variant::Primary)
                                .compact()
                                .loading(self.saving)
                                .on_click(cx.listener(|view, _, _, cx| view.save_composer(cx))),
                            ),
                    )
                    .into_any_element(),
            );
        } else if !self.read_only {
            let anchor = anchor.clone();
            cards.push(
                div()
                    .id(SharedString::from(format!(
                        "comment-reply-{}-{}-{}",
                        anchor.file, anchor.line, anchor.side
                    )))
                    .cursor_pointer()
                    .text_size(px(11.))
                    .text_color(c.accent)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.open_composer(anchor.clone(), None, window, cx)
                    }))
                    .child("Reply")
                    .into_any_element(),
            );
        }
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .pl(px(104.))
                .pr(px(16.))
                .pt(px(6.))
                .pb(px(8.))
                .bg(c.bg_app)
                .border_t_1()
                .border_b_1()
                .border_color(c.border)
                .children(cards)
                .into_any_element(),
        )
    }

    /// Reads the file list again.
    pub fn load(&mut self, cx: &mut Context<Self>) {
        let key = self.key.clone();
        let request = self.link.read(cx).send(Request::DiffFiles(KeyParams {
            key: key.to_string(),
        }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |view, cx| {
                if view.key != key {
                    return;
                }
                match result {
                    Ok(Reply::DiffSummary(summary)) => {
                        view.spans.clear();
                        let count = summary.files.len();
                        view.model.set_summary(*summary);
                        view.list.reset(count);
                    }
                    Ok(_) => view.model.set_problem("Unexpected answer".to_owned()),
                    Err(e) => view.model.set_problem(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Fetches a file's hunks the first time it is drawn.
    fn ensure_loaded(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(path) = self.model.needs_load(index) else {
            return;
        };
        let key = self.key.clone();
        let request = self.link.read(cx).send(Request::DiffFile(DiffFileParams {
            key: key.to_string(),
            path: path.clone(),
            child: None,
            since: None,
        }));
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |view, cx| {
                if view.key != key {
                    return;
                }
                let loaded = match result {
                    Ok(Reply::FileDiff(diff)) => {
                        view.spans.insert(path.clone(), highlight_file(&diff));
                        Ok(*diff)
                    }
                    Ok(_) => Err("Unexpected answer".to_owned()),
                    Err(e) => Err(e.to_string()),
                };
                view.model.loaded(&path, loaded);
                cx.notify();
            });
        })
        .detach();
    }

    fn select(&mut self, path: String, cx: &mut Context<Self>) {
        if let Some(index) = self.model.index_of(&path) {
            self.list.scroll_to_reveal_item(index);
        }
        self.model.selected = Some(path);
        cx.notify();
    }

    fn summary(&self) -> Option<&DiffSummary> {
        self.model.summary.as_ref()
    }

    fn round(&self) -> u32 {
        self.summary().map_or(1, |s| s.round)
    }

    fn toggle_viewed(&mut self, path: &str, cx: &mut Context<Self>) {
        let (key, round) = (self.key.clone(), self.round());
        self.workspace.update(cx, |ws, _| {
            ws.review.toggle_viewed(&key, round, path);
        });
        cx.notify();
    }

    fn agent_summary(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let text = self.summary()?.review_summary.clone()?;
        let c = ActiveTheme::get(cx).colors;
        let open = self.workspace.read(cx).review.summary_open(&self.key);
        let key = self.key.clone();
        let workspace = self.workspace.clone();
        Some(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(6.))
                .px(px(14.))
                .py(px(10.))
                .bg(c.bg_app)
                .border_b_1()
                .border_color(c.border)
                .child(
                    div()
                        .id("diff-summary-toggle")
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .cursor_pointer()
                        .on_click(move |_, _, cx| {
                            workspace.update(cx, |ws, cx| {
                                ws.review.toggle_summary(&key);
                                cx.notify();
                            });
                        })
                        .child(
                            icon(
                                if open {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                },
                                px(12.),
                            )
                            .text_color(c.text_muted),
                        )
                        .child(icon(IconName::Sparkles, px(12.)).text_color(c.st_investigate))
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.text_primary)
                                .child("Agent summary"),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(c.text_muted)
                                .child(format!(
                                    "no tests are run by {}",
                                    crewboard_core::naming::PRODUCT_NAME
                                )),
                        ),
                )
                .children(open.then(|| {
                    div()
                        .pl(px(36.))
                        .text_size(px(12.))
                        .text_color(c.text_secondary)
                        .child(SharedString::from(text))
                }))
                .into_any_element(),
        )
    }

    fn toolbar(&self, summary: &DiffSummary, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let (base, rest) = vm::base_label(summary);
        let viewed =
            self.workspace
                .read(cx)
                .review
                .viewed_count(&self.key, summary.round, &summary.files);
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(14.))
            .py(px(8.))
            .border_b_1()
            .border_color(c.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .text_size(px(12.))
                    .child(icon(IconName::GitCompare, px(13.)).text_color(c.text_muted))
                    .child(div().text_color(c.text_muted).child("diff vs"))
                    .child(
                        div()
                            .font_family(FONT_MONO)
                            .text_color(c.accent)
                            .child(SharedString::from(base)),
                    )
                    .child(
                        div()
                            .text_color(c.text_muted)
                            .child(SharedString::from(rest)),
                    ),
            )
            .child(div().flex_1())
            .when(self.pending() > 0, |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .px(px(8.))
                        .py(px(3.))
                        .rounded(px(12.))
                        .bg(c.accent_soft)
                        .child(icon(IconName::MessageSquare, px(12.)).text_color(c.accent))
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(c.accent)
                                .child(cm::pending_label(self.pending())),
                        ),
                )
            })
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(c.text_muted)
                    .child(vm::viewed_label(viewed, summary.files.len())),
            )
            .child({
                let workspace = self.workspace.clone();
                let chosen = self.workspace.read(cx).review.side_by_side;
                Segmented::new(
                    "diff-view-mode",
                    vec!["Unified".into(), "Side-by-side".into()],
                    usize::from(chosen),
                )
                .on_select(move |i, _, cx| {
                    workspace.update(cx, |ws, cx| {
                        ws.review.side_by_side = i == 1;
                        cx.notify();
                    });
                })
            })
            .into_any_element()
    }

    fn file_list(&self, summary: &DiffSummary, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let width = if self.width < NARROW_BELOW {
            FILE_LIST_NARROW
        } else {
            FILE_LIST
        };
        let viewed_files: std::collections::HashSet<String> = {
            let review = &self.workspace.read(cx).review;
            summary
                .files
                .iter()
                .filter(|f| review.is_viewed(&self.key, summary.round, &f.path))
                .map(|f| f.path.clone())
                .collect()
        };
        let selected_path = self.model.selected.clone();
        let file_row = |i: usize, file: &ChangedFile, cx: &mut Context<Self>| {
            let viewed = viewed_files.contains(&file.path);
            let selected = selected_path.as_deref() == Some(file.path.as_str());
            let path = file.path.clone();
            let tick_path = file.path.clone();
            div()
                .id(("diff-file", i))
                .flex()
                .items_start()
                .gap(px(7.))
                .px(px(8.))
                .py(px(5.))
                .rounded(px(5.))
                .cursor_pointer()
                .when(selected, |d| d.bg(c.accent_soft))
                .when(!selected, |d| d.hover(|s| s.bg(c.bg_hover)))
                .on_click(cx.listener(move |view, _, _, cx| view.select(path.clone(), cx)))
                .child(status_letter(file.status, &c))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(11.))
                        .text_color(c.text_primary)
                        .child(SharedString::from(vm::file_label(file))),
                )
                .child(counts(file, &c))
                .child(
                    div()
                        .id(("diff-viewed", i))
                        .cursor_pointer()
                        .on_click(cx.listener(move |view, _, _, cx| {
                            cx.stop_propagation();
                            view.toggle_viewed(&tick_path, cx);
                        }))
                        .child(
                            icon(
                                if viewed {
                                    IconName::SquareCheck
                                } else {
                                    IconName::Square
                                },
                                px(13.),
                            )
                            .text_color(if viewed {
                                c.accent
                            } else {
                                c.text_muted
                            }),
                        ),
                )
                .into_any_element()
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        if summary.files.len() < vm::GROUP_FROM {
            for (i, file) in summary.files.iter().enumerate() {
                rows.push(file_row(i, file, cx));
            }
        } else {
            rows.push(
                div()
                    .mb(px(4.))
                    .child(self.filter.clone())
                    .into_any_element(),
            );
            let filter = self.filter.read(cx).text().to_owned();
            for group in vm::file_groups(&summary.files, &filter) {
                let folded = self.model.group_folded(&group);
                let name = group.name.clone();
                let label = if group.generated && folded {
                    "generated \u{b7} collapsed".to_owned()
                } else {
                    group.name.clone()
                };
                rows.push(
                    div()
                        .id(SharedString::from(format!("diff-group-{}", group.name)))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .px(px(6.))
                        .py(px(4.))
                        .cursor_pointer()
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.model.toggle_group(&name);
                            cx.notify();
                        }))
                        .child(
                            icon(
                                if folded {
                                    IconName::ChevronRight
                                } else {
                                    IconName::ChevronDown
                                },
                                px(11.),
                            )
                            .text_color(c.text_muted),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .font_family(FONT_MONO)
                                .text_size(px(11.))
                                .text_color(c.text_secondary)
                                .child(SharedString::from(label)),
                        )
                        .child(
                            div()
                                .text_size(px(10.))
                                .text_color(c.text_muted)
                                .child(group.files.len().to_string()),
                        )
                        .into_any_element(),
                );
                if !folded {
                    for i in group.files {
                        rows.push(file_row(i, &summary.files[i], cx));
                    }
                }
            }
        }
        div()
            .id("diff-file-list")
            .w(px(width))
            .flex_none()
            .h_full()
            .overflow_y_scroll()
            .p(px(6.))
            .flex()
            .flex_col()
            .gap(px(1.))
            .border_r_1()
            .border_color(c.border)
            .children(rows)
            .into_any_element()
    }

    /// One file's section of the diff: its header and its hunks.
    fn file_section(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        self.ensure_loaded(index, cx);
        let c = ActiveTheme::get(cx).colors;
        let Some(file) = self.model.files().get(index).cloned() else {
            return div().into_any_element();
        };
        let viewed = self
            .workspace
            .read(cx)
            .review
            .is_viewed(&self.key, self.round(), &file.path);
        let tick_path = file.path.clone();
        let header = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(8.))
            .bg(c.bg_app)
            .border_b_1()
            .border_t_1()
            .border_color(c.border)
            .child(status_letter(file.status, &c))
            .child(
                div()
                    .min_w_0()
                    .font_family(FONT_MONO)
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c.text_primary)
                    .child(SharedString::from(vm::file_label(&file))),
            )
            .children(vm::file_note(&file).map(|note| {
                div()
                    .text_size(px(11.))
                    .text_color(if file.status == FileStatus::Added {
                        c.st_approved
                    } else {
                        c.text_muted
                    })
                    .child(SharedString::from(note))
            }))
            .child(div().flex_1())
            .child(counts(&file, &c))
            .child(
                div()
                    .id(("diff-section-viewed", index))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, _, _, cx| view.toggle_viewed(&tick_path, cx)))
                    .child(
                        icon(
                            if viewed {
                                IconName::SquareCheck
                            } else {
                                IconName::Square
                            },
                            px(12.),
                        )
                        .text_color(if viewed {
                            c.accent
                        } else {
                            c.text_muted
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(c.text_secondary)
                            .child("Viewed"),
                    ),
            );
        let side_by_side =
            vm::shows_side_by_side(self.workspace.read(cx).review.side_by_side, self.width);
        let body: AnyElement = if file.binary {
            note_row(&vm::binary_label(&file), &c)
        } else if !self.model.is_open(&file) {
            let path = file.path.clone();
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(14.))
                .py(px(8.))
                .text_size(px(12.))
                .text_color(c.text_muted)
                .child(if vm::is_generated(&file.path) {
                    "Generated file: collapsed."
                } else {
                    "Large change: collapsed."
                })
                .child(
                    Button::new(("diff-load", index), "Load diff")
                        .variant(Variant::Secondary)
                        .compact()
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.model.open(&path);
                            cx.notify();
                        })),
                )
                .into_any_element()
        } else {
            match self.model.file(&file.path) {
                Some(FileLoad::Loaded(diff)) => {
                    let spans = self.spans.get(&file.path);
                    let rename = vm::rename_label(&file).map(|t| note_row(&t, &c));
                    let diff = (**diff).clone();
                    let spans = spans.cloned();
                    let lines = self.lines(index, &diff, spans.as_ref(), side_by_side, cx);
                    div()
                        .flex()
                        .flex_col()
                        .children(rename)
                        .child(lines)
                        .into_any_element()
                }
                Some(FileLoad::Failed(e)) => note_row(e, &c),
                // Local data arrives at once: an empty placeholder rather than a spinner.
                Some(FileLoad::Loading) | None => div().h(px(LINE_HEIGHT)).into_any_element(),
            }
        };
        div()
            .flex()
            .flex_col()
            .child(header)
            .child(body)
            .into_any_element()
    }
}

impl DiffView {
    /// A file's lines, unified or side by side, each with its gutter (a click opens a comment box there, unless
    /// read-only) and the thread under any line that has comments.
    fn lines(
        &self,
        index: usize,
        diff: &FileDiff,
        spans: Option<&Vec<Vec<Spans>>>,
        side_by_side: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let path = diff.file.path.clone();
        let mut rows: Vec<AnyElement> = Vec::new();
        for (h, hunk) in diff.hunks.iter().enumerate() {
            rows.push(hunk_header(
                &format!(
                    "@@ -{},{} +{},{} @@ {}",
                    hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines, hunk.header
                ),
                &c,
            ));
            let hunk_spans = spans.and_then(|s| s.get(h));
            if side_by_side {
                for (r, SideRow { left, right }) in vm::side_by_side(hunk).into_iter().enumerate() {
                    let anchors: Vec<Anchor> = [&left, &right]
                        .into_iter()
                        .flatten()
                        .filter_map(|(_, l)| Anchor::of(&path, l))
                        .fold(Vec::new(), |mut all, a| {
                            if !all.contains(&a) {
                                all.push(a);
                            }
                            all
                        });
                    let left_half = self.half(left, true, hunk_spans, (index, h, r, 0), &path, cx);
                    let right_half =
                        self.half(right, false, hunk_spans, (index, h, r, 1), &path, cx);
                    rows.push(
                        div()
                            .h(px(LINE_HEIGHT))
                            .flex()
                            .child(left_half)
                            .child(div().w(px(1.)).h_full().bg(c.border))
                            .child(right_half)
                            .into_any_element(),
                    );
                    for anchor in anchors {
                        rows.extend(self.thread_block(&anchor, cx));
                    }
                }
            } else {
                for (l, line) in hunk.lines.iter().enumerate() {
                    let anchor = Anchor::of(&path, line);
                    let gutter = self.gutter(
                        (index, h, l, 2),
                        anchor.clone(),
                        div()
                            .flex()
                            .child(number(line.old, &c))
                            .child(number(line.new, &c)),
                        cx,
                    );
                    rows.push(
                        div()
                            .h(px(LINE_HEIGHT))
                            .flex()
                            .items_center()
                            .when_some(line_background(line.kind, &c), |d, bg| d.bg(bg))
                            .child(gutter)
                            .child(sign(line.kind, &c))
                            .child(code(&line.text, hunk_spans.and_then(|s| s.get(l)), &c))
                            .into_any_element(),
                    );
                    if let Some(anchor) = anchor {
                        rows.extend(self.thread_block(&anchor, cx));
                    }
                }
            }
        }
        if diff.truncated {
            rows.push(note_row(
                "This file's diff is too long to show in full.",
                &c,
            ));
        }
        div().flex().flex_col().children(rows).into_any_element()
    }

    /// One side of a side-by-side row.
    fn half(
        &self,
        side: Option<(usize, DiffLine)>,
        old: bool,
        spans: Option<&Vec<Spans>>,
        id: (usize, usize, usize, usize),
        path: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let Some((i, line)) = side else {
            return div()
                .flex_1()
                .min_w_0()
                .h_full()
                .bg(c.bg_column)
                .into_any_element();
        };
        let anchor = Anchor::of(path, &line);
        let gutter = self.gutter(
            id,
            anchor,
            number(if old { line.old } else { line.new }, &c),
            cx,
        );
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .when_some(line_background(line.kind, &c), |d, bg| d.bg(bg))
            .child(gutter)
            .child(sign(line.kind, &c))
            .child(code(&line.text, spans.and_then(|s| s.get(i)), &c))
            .into_any_element()
    }

    /// The line numbers; a click opens a comment box on the line (REV-03), unless the diff is read-only.
    fn gutter(
        &self,
        (file, hunk, line, part): (usize, usize, usize, usize),
        anchor: Option<Anchor>,
        numbers: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let gutter = div()
            .id(SharedString::from(format!(
                "gutter-{file}-{hunk}-{line}-{part}"
            )))
            .flex_none()
            .h_full()
            .flex()
            .items_center()
            .child(numbers);
        match anchor {
            Some(anchor) if !self.read_only => gutter
                .cursor_pointer()
                .hover(|s| s.bg(c.accent_soft))
                .on_click(cx.listener(move |view, _, window, cx| {
                    view.open_composer(anchor.clone(), None, window, cx)
                }))
                .into_any_element(),
            _ => gutter.into_any_element(),
        }
    }
}

/// Highlights every hunk of a file as a run of lines.
fn highlight_file(diff: &FileDiff) -> Vec<Vec<Spans>> {
    diff.hunks
        .iter()
        .map(|hunk| {
            let lines: Vec<&str> = hunk.lines.iter().map(|l| l.text.as_str()).collect();
            highlight::highlight(&diff.file.path, &lines)
        })
        .collect()
}

pub(crate) fn token_color(token: Token, c: &Colors) -> Hsla {
    match token {
        Token::Plain => c.text_primary,
        Token::Keyword => c.st_investigate,
        Token::String => c.st_review,
        Token::Number | Token::Type => c.st_paused,
        Token::Comment => c.text_muted,
        Token::Function => c.st_working,
    }
}

/// A line's code with its highlighting.
pub(crate) fn code(text: &str, spans: Option<&Spans>, c: &Colors) -> AnyElement {
    let styled = StyledText::new(SharedString::from(text.to_owned()));
    let styled = match spans {
        Some(spans) => styled.with_highlights(spans.iter().map(|(range, token)| {
            (
                range.clone(),
                HighlightStyle {
                    color: Some(token_color(*token, c)),
                    ..Default::default()
                },
            )
        })),
        None => styled,
    };
    div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .font_family(FONT_MONO)
        .text_size(px(12.))
        .text_color(c.text_primary)
        .child(styled)
        .into_any_element()
}

fn line_background(kind: LineKind, c: &Colors) -> Option<Hsla> {
    match kind {
        LineKind::Added => Some(c.diff_add_bg),
        LineKind::Removed => Some(c.diff_del_bg),
        LineKind::Context => None,
    }
}

pub(crate) fn number(n: Option<u32>, c: &Colors) -> AnyElement {
    div()
        .w(px(40.))
        .flex_none()
        .pr(px(8.))
        .flex()
        .justify_end()
        .font_family(FONT_MONO)
        .text_size(px(11.))
        .text_color(c.text_muted)
        .child(SharedString::from(
            n.map(|n| n.to_string()).unwrap_or_default(),
        ))
        .into_any_element()
}

pub(crate) fn sign(kind: LineKind, c: &Colors) -> AnyElement {
    let (text, color) = match kind {
        LineKind::Added => ("+", c.st_approved),
        LineKind::Removed => ("-", c.st_error),
        LineKind::Context => (" ", c.text_muted),
    };
    div()
        .w(px(18.))
        .flex_none()
        .font_family(FONT_MONO)
        .text_size(px(12.))
        .text_color(color)
        .child(text)
        .into_any_element()
}

pub(crate) fn hunk_header(text: &str, c: &Colors) -> AnyElement {
    div()
        .h(px(LINE_HEIGHT))
        .flex()
        .items_center()
        .pl(px(22.))
        .bg(c.bg_column)
        .font_family(FONT_MONO)
        .text_size(px(12.))
        .text_color(c.text_muted)
        .child(SharedString::from(text.to_owned()))
        .into_any_element()
}

pub(crate) fn note_row(text: &str, c: &Colors) -> AnyElement {
    div()
        .px(px(14.))
        .py(px(10.))
        .text_size(px(12.))
        .text_color(c.text_muted)
        .child(SharedString::from(text.to_owned()))
        .into_any_element()
}

fn status_letter(status: FileStatus, c: &Colors) -> AnyElement {
    let color = match status {
        FileStatus::Added => c.st_approved,
        FileStatus::Modified => c.st_working,
        FileStatus::Deleted => c.st_error,
        FileStatus::Renamed => c.st_investigate,
    };
    div()
        .w(px(10.))
        .flex_none()
        .font_family(FONT_MONO)
        .text_size(px(10.))
        .font_weight(FontWeight::BOLD)
        .text_color(color)
        .child(vm::status_letter(status))
        .into_any_element()
}

fn counts(file: &ChangedFile, c: &Colors) -> AnyElement {
    if file.binary {
        return div()
            .flex_none()
            .text_size(px(10.))
            .text_color(c.text_muted)
            .child("binary")
            .into_any_element();
    }
    div()
        .flex()
        .flex_none()
        .gap(px(4.))
        .font_family(FONT_MONO)
        .text_size(px(10.))
        .when(file.added > 0, |d| {
            d.child(
                div()
                    .text_color(c.st_approved)
                    .child(format!("+{}", file.added)),
            )
        })
        .when(file.removed > 0, |d| {
            d.child(
                div()
                    .text_color(c.st_error)
                    .child(format!("\u{2212}{}", file.removed)),
            )
        })
        .into_any_element()
}

impl Render for DiffView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let Some(summary) = self.model.summary.clone() else {
            let text = self.model.problem.clone().unwrap_or_default();
            return div()
                .flex_1()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .text_color(c.text_muted)
                .child(SharedString::from(text))
                .into_any_element();
        };
        let main: AnyElement = if summary.files.is_empty() {
            div()
                .flex_1()
                .p(px(16.))
                .child(
                    div()
                        .max_w(px(440.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(c.border)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .px(px(12.))
                                .py(px(8.))
                                .bg(c.bg_app)
                                .border_b_1()
                                .border_color(c.border)
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(12.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(c.text_primary)
                                        .child(vm::EMPTY_TITLE),
                                )
                                .child(div().text_size(px(11.)).text_color(c.text_muted).child(
                                    format!("vs {} at {}", summary.base_branch, summary.base_short),
                                )),
                        )
                        .child(
                            div()
                                .px(px(14.))
                                .py(px(14.))
                                .text_size(px(12.))
                                .text_color(c.text_secondary)
                                .child(vm::EMPTY_TEXT),
                        ),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .child(self.file_list(&summary, cx))
                .child(
                    list(
                        self.list.clone(),
                        cx.processor(|view, index, _window, cx| view.file_section(index, cx)),
                    )
                    .flex_1()
                    .h_full(),
                )
                .into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .children(self.agent_summary(cx))
            .child(self.toolbar(&summary, cx))
            .child(main)
            .into_any_element()
    }
}
