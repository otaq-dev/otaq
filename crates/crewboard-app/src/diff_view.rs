//! The Diff tab (REV-01, REV-02; d0017, brief 07): the agent's summary (foldable), the base indicator ("diff vs
//! `main` at a1b2c3d"), the file list with status letters, counts and viewed ticks, and the diff itself with
//! syntax highlighting. Files are fetched one at a time as they scroll into view, so a large diff stays fast; the
//! data is local, so nothing shows a spinner. Tickets that aren't in Ready for Review show it read-only.

use std::collections::HashMap;

use crewboard_core::board::diff::DiffSummary;
use crewboard_core::domain::TicketKey;
use crewboard_core::git::diff::{ChangedFile, DiffLine, FileDiff, FileStatus, LineKind};
use crewboard_core::protocol::{DiffFileParams, Event, KeyParams, Reply, Request};
use gpui::{
    AnyElement, Context, Entity, FontWeight, HighlightStyle, Hsla, IntoElement, ListAlignment,
    ListState, ParentElement, Render, SharedString, Styled, StyledText, Window, div, list,
    prelude::*, px,
};

use crate::highlight::{self, Spans, Token};
use crate::icons::{IconName, icon};
use crate::service_link::{ServiceEvent, ServiceLink};
use crate::theme::{ActiveTheme, Colors, FONT_MONO};
use crate::view_models::diff::{self as vm, DiffModel, FileLoad};
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
    /// The panel's width, for the file list.
    pub width: f32,
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
                ServiceEvent::Resync => view.load(cx),
                _ => {}
            }
        })
        .detach();
        let mut view = Self {
            workspace,
            link,
            key,
            model: DiffModel::default(),
            list: ListState::new(0, ListAlignment::Top, px(600.)),
            spans: HashMap::new(),
            width: crate::view_models::panel::DEFAULT_WIDTH,
        };
        view.load(cx);
        view
    }

    /// The tab's "+412 −38".
    pub fn indicator(&self) -> Option<String> {
        vm::tab_indicator(self.model.summary.as_ref())
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
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(c.text_muted)
                    .child(vm::viewed_label(viewed, summary.files.len())),
            )
            .into_any_element()
    }

    fn file_list(&self, summary: &DiffSummary, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let width = if self.width < NARROW_BELOW {
            FILE_LIST_NARROW
        } else {
            FILE_LIST
        };
        let review = &self.workspace.read(cx).review;
        let rows: Vec<AnyElement> = summary
            .files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                let viewed = review.is_viewed(&self.key, summary.round, &file.path);
                let selected = self.model.selected.as_deref() == Some(file.path.as_str());
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
            })
            .collect();
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
        let body: AnyElement = if file.binary {
            note_row(
                &format!(
                    "Binary file{}: no text diff",
                    file.size
                        .map(|s| format!(" · {}", vm::size_label(s)))
                        .unwrap_or_default()
                ),
                &c,
            )
        } else {
            match self.model.file(&file.path) {
                Some(FileLoad::Loaded(diff)) => {
                    let spans = self.spans.get(&file.path);
                    self.unified(diff, spans, &c)
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

    fn unified(&self, diff: &FileDiff, spans: Option<&Vec<Vec<Spans>>>, c: &Colors) -> AnyElement {
        let mut rows: Vec<AnyElement> = Vec::new();
        for (h, hunk) in diff.hunks.iter().enumerate() {
            rows.push(hunk_header(
                &format!(
                    "@@ -{},{} +{},{} @@ {}",
                    hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines, hunk.header
                ),
                c,
            ));
            for (l, line) in hunk.lines.iter().enumerate() {
                let line_spans = spans.and_then(|s| s.get(h)).and_then(|s| s.get(l));
                rows.push(unified_line(line, line_spans, c));
            }
        }
        if diff.truncated {
            rows.push(note_row("This file's diff is too long to show in full.", c));
        }
        div().flex().flex_col().children(rows).into_any_element()
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

fn unified_line(line: &DiffLine, spans: Option<&Spans>, c: &Colors) -> AnyElement {
    div()
        .h(px(LINE_HEIGHT))
        .flex()
        .items_center()
        .when_some(line_background(line.kind, c), |d, bg| d.bg(bg))
        .child(number(line.old, c))
        .child(number(line.new, c))
        .child(sign(line.kind, c))
        .child(code(&line.text, spans, c))
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
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .text_color(c.text_muted)
                .child(format!(
                    "No changes yet: {} has nothing on top of {}.",
                    summary.branch, summary.base_branch
                ))
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
