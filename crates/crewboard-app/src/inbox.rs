//! The "Needs you" inbox popover (UI-06, ATT-06, ATT-09, ATT-10; d0016, d0024, d0058; *10 · Needs-you inbox →
//! Inbox popover · mixed, 2 projects; One item; Empty · nothing needs you; Resolved elsewhere; Resume all ·
//! confirm; Parent closed · new item in group 2; Dark · inbox*). 440 px under the header's Needs-you button; ⇧⌘I or
//! the button opens it, Esc closes it, ↑↓ move, ↵ opens the selected item. Every row names its project; opening a
//! row switches project if needed and opens the panel on the right tab. The view model is
//! [`crate::view_models::inbox`].

use crewboard_core::domain::TicketKey;
use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, KeyBinding,
    SharedString, Window, actions, div, prelude::*, px,
};

use crate::components::answer_options::{
    self, CancelPick, OptionList, PickOption1, PickOption2, PickOption3, PickOption4, SendPick,
};
use crate::components::button::{Button, Variant};
use crate::components::floating_shadow;
use crate::components::kbd::Kbd;
use crate::components::text_input::{InputEvent, TextInput};
use crate::icons::{IconName, icon};
use crate::keymap::{KeyAction, Platform};
use crate::theme::{ActiveTheme, FONT_MONO};
use crate::view_models::answer::{self, AnswerKind, AnswerOutcome};
use crate::view_models::answer_options::{AnswerOptions, Pick};
use crate::view_models::inbox::{InboxGroupView, InboxRow, RowAction};
use crate::view_models::panel::Tab;
use crate::view_models::terminal_strip::UnsentPrompt;
use crate::workspace::Workspace;
use crewboard_core::board::questions::HumanQuestion;
use crewboard_core::protocol::Reply;

actions!(inbox, [InboxUp, InboxDown, InboxOpen, InboxClose]);

const CONTEXT: &str = "Inbox";
/// The popover's width (frame).
pub const WIDTH: f32 = 440.;

pub fn bind_keys(cx: &mut App) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("up", InboxUp, c),
        KeyBinding::new("down", InboxDown, c),
        KeyBinding::new("enter", InboxOpen, c),
        KeyBinding::new("escape", InboxClose, c),
    ]);
}

/// What the popover asks its owner to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxEvent {
    Close,
    /// Open this ticket's panel (switching project if needed) on this tab.
    Open {
        project: String,
        key: TicketKey,
        tab: Tab,
    },
    /// "Resume all (N)": ask first.
    ResumeAll,
    /// A row's button.
    Action(TicketKey, RowAction),
    /// The head's Next (⌘J).
    Next,
    /// An answer stopped at unsent text in the terminal: ask Keep it / Clear it and send.
    Unsent(UnsentPrompt),
}

/// The reply box open in a row.
struct Draft {
    key: TicketKey,
    kind: AnswerKind,
    /// The numbered options when the question has them (ATT-12); the input is then Other…'s field.
    options: Option<AnswerOptions>,
    /// The option list's keyboard (1–4, ↵, Esc).
    list_focus: FocusHandle,
    input: Entity<TextInput>,
    /// Sent and waiting for the service.
    sending: bool,
    error: Option<String>,
}

pub struct InboxView {
    workspace: Entity<Workspace>,
    focus: FocusHandle,
    /// The selected row among the waiting rows (↑↓).
    selected: usize,
    /// The reply box, while one is open (ATT-08).
    draft: Option<Draft>,
}

impl EventEmitter<InboxEvent> for InboxView {}

impl Focusable for InboxView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl InboxView {
    pub fn new(workspace: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
        Self {
            workspace,
            focus: cx.focus_handle(),
            selected: 0,
            draft: None,
        }
    }

    /// Answer: opens the reply box in the row with the keyboard in it.
    fn open_draft(
        &mut self,
        key: TicketKey,
        kind: AnswerKind,
        question: Option<HumanQuestion>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let options = question.as_ref().and_then(AnswerOptions::of);
        let input = cx.new(|cx| TextInput::new(window, cx).placeholder("Type your answer"));
        cx.subscribe_in(
            &input,
            window,
            |view, _, event: &InputEvent, window, cx| match event {
                InputEvent::Submit => view.send(cx),
                InputEvent::Cancel => {
                    view.draft = None;
                    window.focus(&view.focus, cx);
                    cx.notify();
                }
                _ => cx.notify(),
            },
        )
        .detach();
        let list_focus = cx.focus_handle();
        if options.is_some() {
            window.focus(&list_focus, cx);
        } else {
            window.focus(&input.focus_handle(cx), cx);
        }
        self.draft = Some(Draft {
            key,
            kind,
            options,
            list_focus,
            input,
            sending: false,
            error: None,
        });
        cx.notify();
    }

    /// 1–4 or a click on an option: Other… moves the keyboard to its field.
    fn pick(&mut self, pick: Pick, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = &mut self.draft else { return };
        let Some(options) = &mut draft.options else {
            return;
        };
        let picked = match pick {
            Pick::Option(n) => options.press(n),
            Pick::Other => options.press(options.other_number()),
        };
        if picked && pick == Pick::Other {
            window.focus(&draft.input.focus_handle(cx), cx);
        }
        cx.notify();
    }

    fn press(&mut self, n: u32, window: &mut Window, cx: &mut Context<Self>) {
        let other = self
            .draft
            .as_ref()
            .and_then(|d| d.options.as_ref())
            .is_some_and(|o| o.other_number() == n);
        self.pick(
            if other { Pick::Other } else { Pick::Option(n) },
            window,
            cx,
        );
    }

    fn cancel_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Send (↵): types the answer into the session, or resumes it with the answer.
    fn send(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = &mut self.draft else { return };
        let text = draft.input.read(cx).text().to_owned();
        let choice = match &draft.options {
            Some(options) => options.choice(&text),
            None => answer::free_text(&text),
        };
        let Some(choice) = choice else {
            return;
        };
        if draft.sending {
            return;
        }
        draft.sending = true;
        draft.error = None;
        let key = draft.key.clone();
        let request = answer::request(&key, &choice, false);
        let task = self.workspace.read(cx).link.read(cx).send(request);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| {
                match result {
                    Ok(Reply::Delivery(delivery)) => {
                        view.draft = None;
                        if let AnswerOutcome::Ask(prompt) =
                            answer::outcome(&key, &choice, &delivery)
                        {
                            cx.emit(InboxEvent::Unsent(prompt));
                        }
                    }
                    Ok(_) => view.draft = None,
                    Err(e) => {
                        if let Some(draft) = &mut view.draft {
                            draft.sending = false;
                            draft.error = Some(e.message);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// The numbered options with Other… and Send (*10 → Answer · inline, Answer · options, no recommendation*).
    fn options_box(
        &self,
        draft: &Draft,
        options: &AnswerOptions,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        let text = draft.input.read(cx).text().to_owned();
        let can_send = options.can_send(&text) && !draft.sending;
        let other = options.pick == Some(Pick::Other);
        let view = cx.entity().downgrade();
        div()
            .id(SharedString::from(format!("inbox-options-{}", draft.key)))
            .key_context(answer_options::CONTEXT)
            .track_focus(&draft.list_focus)
            .on_action(cx.listener(|v, _: &PickOption1, w, cx| v.press(1, w, cx)))
            .on_action(cx.listener(|v, _: &PickOption2, w, cx| v.press(2, w, cx)))
            .on_action(cx.listener(|v, _: &PickOption3, w, cx| v.press(3, w, cx)))
            .on_action(cx.listener(|v, _: &PickOption4, w, cx| v.press(4, w, cx)))
            .on_action(cx.listener(|v, _: &SendPick, _, cx| v.send(cx)))
            .on_action(cx.listener(|v, _: &CancelPick, w, cx| v.cancel_draft(w, cx)))
            .flex()
            .flex_col()
            .gap(px(8.))
            .pt(px(6.))
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(10.))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(
                        crate::view_models::answer_options::heading(&draft.key),
                    )),
            )
            .child(OptionList::new(
                SharedString::from(format!("inbox-option-list-{}", draft.key)),
                options.rows(),
                move |pick, window, cx| {
                    let _ = view.update(cx, |v, cx| v.pick(pick, window, cx));
                },
            ))
            .when(other, |d| d.child(draft.input.clone()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().flex_1())
                    .child(
                        Button::new(
                            SharedString::from(format!("inbox-cancel-{}", draft.key)),
                            "Cancel",
                        )
                        .variant(Variant::Ghost)
                        .compact()
                        .on_click(cx.listener(|v, _, window, cx| {
                            cx.stop_propagation();
                            v.cancel_draft(window, cx);
                        })),
                    )
                    .child(
                        Button::new(
                            SharedString::from(format!("inbox-send-{}", draft.key)),
                            draft.kind.send_label(),
                        )
                        .icon(draft.kind.send_icon())
                        .disabled(!can_send)
                        .loading(draft.sending)
                        .on_click(cx.listener(|view, _, _, cx| {
                            cx.stop_propagation();
                            view.send(cx);
                        })),
                    )
                    .child(Kbd::text("↵")),
            )
            .children(draft.error.clone().map(|e| {
                div()
                    .text_size(px(11.))
                    .text_color(c.st_error)
                    .child(SharedString::from(e))
            }))
            .into_any_element()
    }

    /// The reply box under a row's text (*10 → After a reboot · answer resumes*).
    fn draft_box(&self, draft: &Draft, cx: &mut Context<Self>) -> AnyElement {
        if let Some(options) = &draft.options {
            return self.options_box(draft, options, cx);
        }
        let c = ActiveTheme::get(cx).colors;
        let empty = answer::free_text(draft.input.read(cx).text()).is_none();
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .pt(px(4.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().flex_1().min_w_0().child(draft.input.clone()))
                    .child(
                        Button::new(
                            SharedString::from(format!("inbox-send-{}", draft.key)),
                            draft.kind.send_label(),
                        )
                        .icon(draft.kind.send_icon())
                        .disabled(empty)
                        .loading(draft.sending)
                        .on_click(cx.listener(|view, _, _, cx| {
                            cx.stop_propagation();
                            view.send(cx);
                        })),
                    ),
            )
            .children(draft.error.clone().map(|e| {
                div()
                    .text_size(px(11.))
                    .text_color(c.st_error)
                    .child(SharedString::from(e))
            }))
            .into_any_element()
    }

    fn groups(&self, cx: &App) -> Vec<InboxGroupView> {
        let ws = self.workspace.read(cx);
        ws.inbox.groups(&|k| ws.closed_parent(k))
    }

    /// The waiting rows in order (what ↑↓ walk).
    fn waiting(&self, cx: &App) -> Vec<InboxRow> {
        self.groups(cx)
            .into_iter()
            .flat_map(|g| g.rows)
            .filter(|r| !r.resolved)
            .collect()
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        let n = self.waiting(cx).len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected as isize + by).clamp(0, n as isize - 1) as usize;
        cx.notify();
    }

    fn open_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(row) = self.waiting(cx).get(self.selected) {
            cx.emit(InboxEvent::Open {
                project: row.project.clone(),
                key: row.key.clone(),
                tab: row.tab,
            });
        }
    }

    fn row(&self, row: &InboxRow, selected: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = ActiveTheme::get(cx);
        let c = theme.colors;
        let style = theme.status(row.legend);
        let open = InboxEvent::Open {
            project: row.project.clone(),
            key: row.key.clone(),
            tab: row.tab,
        };
        let actions: Vec<AnyElement> = row
            .actions
            .iter()
            .map(|action| {
                let (id, label, icon_name, variant) = match action {
                    RowAction::RetryStart => (
                        "retry",
                        "Retry start",
                        IconName::RotateCw,
                        Variant::Secondary,
                    ),
                    RowAction::ReopenParent(_) => (
                        "reopen",
                        "Reopen parent",
                        IconName::RotateCcw,
                        Variant::Primary,
                    ),
                    RowAction::RemoveDependency(_) => (
                        "unlink",
                        "Remove dependency",
                        IconName::Link2,
                        Variant::Secondary,
                    ),
                };
                let event = InboxEvent::Action(row.key.clone(), action.clone());
                Button::new(SharedString::from(format!("inbox-{id}-{}", row.key)), label)
                    .icon(icon_name)
                    .variant(variant)
                    .compact()
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.stop_propagation();
                        cx.emit(event.clone());
                    }))
                    .into_any_element()
            })
            .collect();
        let drafting = self.draft.as_ref().filter(|d| d.key == row.key);
        let answer_box = drafting.map(|d| self.draft_box(d, cx));
        let answer_button = row.answer.filter(|_| drafting.is_none()).map(|kind| {
            let key = row.key.clone();
            let question = row.question.clone();
            Button::new(
                SharedString::from(format!("inbox-answer-{}", row.key)),
                kind.open_label(),
            )
            .icon(if kind == AnswerKind::Resume {
                IconName::Play
            } else {
                IconName::MessageCircleReply
            })
            .variant(Variant::Primary)
            .compact()
            .on_click(cx.listener(move |view, _, window, cx| {
                cx.stop_propagation();
                view.open_draft(key.clone(), kind, question.clone(), window, cx);
            }))
            .into_any_element()
        });
        let actions: Vec<AnyElement> = answer_button.into_iter().chain(actions).collect();
        let muted = |d: gpui::Div| if row.resolved { d.opacity(0.55) } else { d };
        let title = div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if row.resolved {
                c.text_muted
            } else {
                c.text_primary
            })
            .when(row.resolved, |d| d.line_through())
            .child(SharedString::from(row.title.clone()));
        let line1 = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(
                div()
                    .flex_none()
                    .px(px(5.))
                    .rounded(px(3.))
                    .bg(c.bg_column)
                    .font_family(FONT_MONO)
                    .text_size(px(10.))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(row.project_name.clone())),
            )
            .child(
                div()
                    .flex_none()
                    .font_family(FONT_MONO)
                    .text_size(px(11.))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(row.key.to_string())),
            )
            .child(title);
        let line3 = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .font_family(FONT_MONO)
            .text_size(px(10.))
            .text_color(c.text_muted)
            .child(SharedString::from(row.age.clone()))
            .children(row.blocks.clone().map(|b| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(3.))
                    .child(icon(IconName::Link2, px(10.)).text_color(c.text_muted))
                    .child(SharedString::from(b))
            }));
        let body = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(3.))
            .child(line1)
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(gpui::relative(1.35))
                    .text_color(c.text_secondary)
                    .child(SharedString::from(row.reason.clone())),
            )
            .child(line3)
            .children(row.restart_note.map(|note| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(11.))
                    .text_color(c.st_attn)
                    .child(icon(IconName::ZapOff, px(11.)).text_color(c.st_attn))
                    .child(note)
            }))
            .when(!actions.is_empty(), |d| {
                d.child(div().flex().gap(px(8.)).pt(px(4.)).children(actions))
            })
            .children(answer_box);
        let el = div()
            .id(SharedString::from(format!("inbox-row-{}", row.key)))
            .flex()
            .gap(px(10.))
            .px(px(14.))
            .py(px(8.))
            .when(selected, |d| d.bg(c.accent_soft))
            .when(!row.resolved, |d| {
                d.cursor_pointer()
                    .hover(|s| s.bg(c.bg_hover))
                    .on_click(cx.listener(move |_, _, _, cx| cx.emit(open.clone())))
            })
            .child(
                div()
                    .flex_none()
                    .size(px(24.))
                    .rounded(px(6.))
                    .bg(style.bg)
                    .flex()
                    .justify_center()
                    .items_center()
                    .child(icon(style.icon, px(13.)).text_color(style.fg)),
            )
            .child(body)
            .children(selected.then(|| Kbd::text("↵")));
        muted(div().child(el)).into_any_element()
    }

    fn group_head(&self, group: &InboxGroupView, cx: &mut Context<Self>) -> AnyElement {
        let c = ActiveTheme::get(cx).colors;
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .pt(px(10.))
            .px(px(14.))
            .pb(px(4.))
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(10.))
                    .text_color(c.text_secondary)
                    .child(group.title),
            )
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(10.))
                    .text_color(c.text_muted)
                    .child(SharedString::from(group.count.to_string())),
            )
            .child(div().flex_1())
            .children(group.resume_all.map(|n| {
                div()
                    .id("inbox-resume-all")
                    .cursor_pointer()
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c.accent)
                    .child(SharedString::from(format!("Resume all ({n})")))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(InboxEvent::ResumeAll)))
            }))
            .into_any_element()
    }
}

impl Render for InboxView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let platform = Platform::current();
        let groups = self.groups(cx);
        let total = self.workspace.read(cx).inbox.total();
        let running =
            crate::view_models::limits::running_total(self.workspace.read(cx).switcher.projects());
        let waiting = self.waiting(cx).len();
        if waiting > 0 && self.selected >= waiting {
            self.selected = waiting - 1;
        }
        let next_key = KeyAction::NextInboxItem.keys(platform).unwrap_or_default();
        let head = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(14.))
            .py(px(12.))
            .border_b_1()
            .border_color(c.border)
            .child(icon(IconName::Inbox, px(15.)).text_color(if total > 0 {
                c.st_attn
            } else {
                c.text_secondary
            }))
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c.text_primary)
                    .child("Needs you"),
            )
            .child(
                div()
                    .font_family(FONT_MONO)
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if total > 0 { c.st_attn } else { c.text_muted })
                    .child(SharedString::from(total.to_string())),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(c.text_muted)
                    .child("· all projects"),
            )
            .child(div().flex_1())
            .when(total > 0, |d| {
                d.child(
                    Button::new("inbox-next", "Next")
                        .icon(IconName::ArrowRight)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(InboxEvent::Next))),
                )
                .child(Kbd::text(next_key.clone()))
            });
        let mut list: Vec<AnyElement> = Vec::new();
        let mut n = 0;
        for group in &groups {
            list.push(self.group_head(group, cx));
            for row in &group.rows {
                let selected = !row.resolved && n == self.selected;
                if !row.resolved {
                    n += 1;
                }
                list.push(self.row(row, selected, cx));
            }
        }
        let empty = groups.is_empty().then(|| {
            let plural = if running == 1 { "agent" } else { "agents" };
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(10.))
                .px(px(24.))
                .py(px(40.))
                .child(
                    div()
                        .size(px(44.))
                        .rounded_full()
                        .bg(c.st_approved_bg)
                        .flex()
                        .justify_center()
                        .items_center()
                        .child(icon(IconName::Check, px(20.)).text_color(c.st_approved)),
                )
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c.text_primary)
                        .child("Nothing needs you"),
                )
                .child(
                    div()
                        .w(px(280.))
                        .text_center()
                        .text_size(px(12.))
                        .line_height(gpui::relative(1.45))
                        .text_color(c.text_secondary)
                        .child(SharedString::from(format!(
                            "{running} {plural} running. You’ll get a notification when one is blocked on you."
                        ))),
                )
        });
        let hint = |key: String, label: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(Kbd::text(key))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(c.text_muted)
                        .child(label),
                )
        };
        let foot = (!groups.is_empty()).then(|| {
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .px(px(14.))
                .py(px(8.))
                .bg(c.bg_panel)
                .border_t_1()
                .border_color(c.border)
                .child(hint("↑↓".into(), "move"))
                .child(hint("↵".into(), "open"))
                .child(hint(next_key.clone(), "next item"))
        });
        div()
            .id("inbox-popover")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|v, _: &InboxUp, _, cx| v.step(-1, cx)))
            .on_action(cx.listener(|v, _: &InboxDown, _, cx| v.step(1, cx)))
            .on_action(cx.listener(|v, _: &InboxOpen, _, cx| v.open_selected(cx)))
            .on_action(cx.listener(|_, _: &InboxClose, _, cx| cx.emit(InboxEvent::Close)))
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(InboxEvent::Close)))
            .w(px(WIDTH))
            .max_h(px(640.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(c.bg_raised)
            .border_1()
            .border_color(c.border)
            .rounded(px(10.))
            .shadow(floating_shadow())
            .child(head)
            .child(
                div()
                    .id("inbox-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pb(px(6.))
                    .children(list)
                    .children(empty),
            )
            .children(foot)
    }
}
