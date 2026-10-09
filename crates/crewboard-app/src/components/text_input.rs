//! Text inputs (*01 · Components → Badges & Inputs → Fields*): a single-line input (30 px, 5 px radius) and a
//! multi-line one, with placeholder, a 1.5 px focus ring, an error state, and an optional mono face for paths
//! and IDs. GPUI has no text field, so this one follows gpui's input example: the editing rules live in
//! [`EditBuffer`] (unit-tested), this file draws it and talks to the platform's input method.
//!
//! Keys (context `TextInput`): arrows, ⇧+arrows, ⌘A, Home/End (⌘←/⌘→), ⌫, Delete, ⌘C/⌘X/⌘V; ↵ submits a
//! single-line input and breaks the line in a multi-line one (⌘↵ submits there); Esc cancels.

use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable, FontWeight, GlobalElementId,
    KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window, WrappedLine, actions,
    div, fill, font, point, prelude::*, px, relative, size,
};

use crate::theme::{ActiveTheme, FONT_MONO, FONT_UI};
use crate::view_models::edit_buffer::EditBuffer;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        SelectHome,
        SelectEnd,
        Paste,
        Cut,
        Copy,
        Enter,
        AltEnter,
        SubmitInput,
        CancelInput,
    ]
);

const CONTEXT: &str = "TextInput";

/// Binds the input keys. Call once at start-up.
pub fn bind_keys(cx: &mut App) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("up", Up, c),
        KeyBinding::new("down", Down, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("secondary-a", SelectAll, c),
        KeyBinding::new("home", Home, c),
        KeyBinding::new("end", End, c),
        KeyBinding::new("secondary-left", Home, c),
        KeyBinding::new("secondary-right", End, c),
        KeyBinding::new("shift-home", SelectHome, c),
        KeyBinding::new("shift-end", SelectEnd, c),
        KeyBinding::new("secondary-v", Paste, c),
        KeyBinding::new("secondary-c", Copy, c),
        KeyBinding::new("secondary-x", Cut, c),
        KeyBinding::new("enter", Enter, c),
        KeyBinding::new("shift-enter", AltEnter, c),
        KeyBinding::new("secondary-enter", SubmitInput, c),
        KeyBinding::new("escape", CancelInput, c),
    ]);
}

/// What an input tells its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    Changed,
    /// ↵ in a single-line input, ⌘↵ in a multi-line one.
    Submit,
    /// Esc.
    Cancel,
    /// ⇧↵ in a single-line input ("open the full form").
    AltSubmit,
    /// The input lost focus.
    Blur,
    /// ↑ / ↓ in a single-line input that hands arrows to its owner (a list under it, e.g. ⌘P).
    ArrowUp,
    ArrowDown,
}

pub struct TextInput {
    focus: FocusHandle,
    buffer: EditBuffer,
    placeholder: SharedString,
    mono: bool,
    disabled: bool,
    error: bool,
    /// Height of a multi-line input's box.
    height: Pixels,
    lines: Vec<(usize, WrappedLine)>,
    bounds: Option<Bounds<Pixels>>,
    scroll: Point<Pixels>,
    selecting: bool,
    /// Single-line only: ↑/↓ are emitted as events instead of moving the cursor.
    arrows_out: bool,
    /// No box: border, background and padding come from the owner.
    bare: bool,
    /// Lines inside ``` fences are drawn in the mono face on the terminal background (Markdown bodies).
    code_fences: bool,
}

impl EventEmitter<InputEvent> for TextInput {}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextInput {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        cx.on_blur(&focus, window, |_, _, cx| {
            cx.emit(InputEvent::Blur);
            cx.notify();
        })
        .detach();
        Self {
            focus,
            buffer: EditBuffer::new(false),
            placeholder: SharedString::default(),
            mono: false,
            disabled: false,
            error: false,
            height: px(72.),
            lines: Vec::new(),
            bounds: None,
            scroll: Point::default(),
            selecting: false,
            arrows_out: false,
            bare: false,
            code_fences: false,
        }
    }

    /// Draws fenced code blocks in Geist Mono on the terminal background.
    pub fn code_fences(mut self) -> Self {
        self.code_fences = true;
        self
    }

    /// Hands ↑/↓ to the owner as [`InputEvent::ArrowUp`] / [`InputEvent::ArrowDown`].
    pub fn arrows_out(mut self) -> Self {
        self.arrows_out = true;
        self
    }

    /// Draws only the text: the owner draws the box.
    pub fn bare(mut self) -> Self {
        self.bare = true;
        self
    }

    /// A multi-line input whose box is `height` tall (the text scrolls inside).
    pub fn multiline(mut self, height: Pixels) -> Self {
        let text = self.buffer.text().to_owned();
        self.buffer = EditBuffer::new(true);
        self.buffer.set_text(&text);
        self.height = height;
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Geist Mono 12 (paths, keys, IDs).
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    pub fn text(&self) -> &str {
        self.buffer.text()
    }

    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.buffer.set_text(text);
        cx.notify();
    }

    pub fn set_error(&mut self, error: bool, cx: &mut Context<Self>) {
        if self.error != error {
            self.error = error;
            cx.notify();
        }
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    fn edited(&mut self, cx: &mut Context<Self>) {
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn line_height(&self) -> Pixels {
        if self.buffer.is_multiline() {
            px(19.)
        } else {
            px(18.)
        }
    }

    /// Where an offset is drawn, relative to the text's top-left (before scrolling).
    fn position_for(&self, offset: usize) -> Point<Pixels> {
        let lh = self.line_height();
        let mut y = px(0.);
        for (start, line) in &self.lines {
            let end = start + line.len();
            if offset <= end {
                let p = line
                    .position_for_index(offset - start, lh)
                    .unwrap_or_default();
                return point(p.x, y + p.y);
            }
            y += line.size(lh).height;
        }
        point(px(0.), y)
    }

    /// The offset nearest a point relative to the text's top-left (before scrolling).
    fn offset_for(&self, position: Point<Pixels>) -> usize {
        let lh = self.line_height();
        let mut y = px(0.);
        for (start, line) in &self.lines {
            let h = line.size(lh).height;
            if position.y < y + h {
                let local = point(position.x.max(px(0.)), (position.y - y).max(px(0.)));
                let i = match line.closest_index_for_position(local, lh) {
                    Ok(i) | Err(i) => i,
                };
                return start + i.min(line.len());
            }
            y += h;
        }
        self.buffer.text().len()
    }

    fn vertical(&mut self, lines: f32, select: bool, cx: &mut Context<Self>) {
        if self.arrows_out && !self.buffer.is_multiline() && !select {
            cx.emit(if lines < 0. {
                InputEvent::ArrowUp
            } else {
                InputEvent::ArrowDown
            });
            return;
        }
        if !self.buffer.is_multiline() {
            if lines < 0. {
                self.buffer.home(select);
            } else {
                self.buffer.end(select);
            }
        } else {
            let p = self.position_for(self.buffer.cursor());
            let target = point(
                p.x,
                p.y + self.line_height() * lines + self.line_height() / 2.,
            );
            let offset = if target.y < px(0.) {
                0
            } else {
                self.offset_for(target)
            };
            self.buffer.move_to(offset, select);
        }
        cx.notify();
    }

    fn mouse_offset(&self, position: Point<Pixels>) -> usize {
        let Some(bounds) = self.bounds else { return 0 };
        self.offset_for(position - bounds.origin + self.scroll)
    }

    fn on_mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        window.focus(&self.focus, cx);
        self.selecting = true;
        let offset = self.mouse_offset(e.position);
        if e.click_count >= 2 {
            self.buffer.select_all();
        } else {
            self.buffer.move_to(offset, e.modifiers.shift);
        }
        cx.notify();
    }

    fn on_mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            let offset = self.mouse_offset(e.position);
            self.buffer.move_to(offset, true);
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let b = &self.buffer;
        let range = b.offset_from_utf16(range.start)..b.offset_from_utf16(range.end);
        actual.replace(b.offset_to_utf16(range.start)..b.offset_to_utf16(range.end));
        Some(b.text()[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let b = &self.buffer;
        let s = b.selection();
        Some(UTF16Selection {
            range: b.offset_to_utf16(s.start)..b.offset_to_utf16(s.end),
            reversed: b.is_reversed(),
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let b = &self.buffer;
        b.marked()
            .map(|m| b.offset_to_utf16(m.start)..b.offset_to_utf16(m.end))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.buffer.unmark();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        match range {
            Some(r) => {
                let r =
                    self.buffer.offset_from_utf16(r.start)..self.buffer.offset_from_utf16(r.end);
                self.buffer.replace(r, text);
            }
            None => self.buffer.insert(text),
        }
        self.edited(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let b = &self.buffer;
        let range = range
            .map(|r| b.offset_from_utf16(r.start)..b.offset_from_utf16(r.end))
            .or(b.marked())
            .unwrap_or(b.selection());
        self.buffer.replace_and_mark(range, text, selected);
        self.edited(cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let start = self.position_for(self.buffer.offset_from_utf16(range.start));
        let end = self.position_for(self.buffer.offset_from_utf16(range.end));
        let origin = bounds.origin - self.scroll;
        Some(Bounds::from_corners(
            origin + start,
            origin + point(end.x, end.y + self.line_height()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let offset = self.mouse_offset(p);
        Some(self.buffer.offset_to_utf16(offset))
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = ActiveTheme::get(cx).colors;
        let focused = self.focus.is_focused(window);
        let multiline = self.buffer.is_multiline();
        let (border, width) = if self.error {
            (c.st_error, px(1.))
        } else if focused {
            (c.focus, px(1.5))
        } else {
            (c.border_strong, px(1.))
        };
        let mut el = div()
            .id(ElementId::from(&self.focus))
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(|this, _: &Backspace, _, cx| {
                if this.buffer.backspace() {
                    this.edited(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Delete, _, cx| {
                if this.buffer.delete() {
                    this.edited(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Left, _, cx| {
                this.buffer.left(false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Right, _, cx| {
                this.buffer.right(false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectLeft, _, cx| {
                this.buffer.left(true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectRight, _, cx| {
                this.buffer.right(true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Up, _, cx| this.vertical(-1., false, cx)))
            .on_action(cx.listener(|this, _: &Down, _, cx| this.vertical(1., false, cx)))
            .on_action(cx.listener(|this, _: &SelectUp, _, cx| this.vertical(-1., true, cx)))
            .on_action(cx.listener(|this, _: &SelectDown, _, cx| this.vertical(1., true, cx)))
            .on_action(cx.listener(|this, _: &SelectAll, _, cx| {
                this.buffer.select_all();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Home, _, cx| {
                this.buffer.home(false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &End, _, cx| {
                this.buffer.end(false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectHome, _, cx| {
                this.buffer.home(true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectEnd, _, cx| {
                this.buffer.end(true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Paste, _, cx| {
                if this.disabled {
                    return;
                }
                if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    this.buffer.insert(&text);
                    this.edited(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                let selected = this.buffer.selected_text();
                if !selected.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(selected.to_owned()));
                }
            }))
            .on_action(cx.listener(|this, _: &Cut, _, cx| {
                let selected = this.buffer.selected_text().to_owned();
                if !selected.is_empty() && !this.disabled {
                    cx.write_to_clipboard(ClipboardItem::new_string(selected));
                    this.buffer.insert("");
                    this.edited(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Enter, _, cx| {
                if this.buffer.is_multiline() {
                    if !this.disabled {
                        this.buffer.newline();
                        this.edited(cx);
                    }
                } else {
                    cx.emit(InputEvent::Submit);
                }
            }))
            .on_action(cx.listener(|this, _: &AltEnter, _, cx| {
                if this.buffer.is_multiline() {
                    if !this.disabled {
                        this.buffer.newline();
                        this.edited(cx);
                    }
                } else {
                    cx.emit(InputEvent::AltSubmit);
                }
            }))
            .on_action(cx.listener(|_, _: &SubmitInput, _, cx| cx.emit(InputEvent::Submit)))
            .on_action(cx.listener(|_, _: &CancelInput, _, cx| cx.emit(InputEvent::Cancel)))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .flex()
            .w_full()
            .px(px(10.))
            .bg(c.bg_raised)
            .rounded(px(5.))
            .border(width)
            .border_color(border)
            .overflow_hidden()
            .child(TextElement { input: cx.entity() });
        el = if multiline {
            el.h(self.height).py(px(8.))
        } else {
            el.h(px(30.)).items_center()
        };
        if self.bare {
            el = el.bg(gpui::transparent_black()).border_0().px_0();
        }
        if self.disabled {
            el = el.opacity(0.6);
        }
        el
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct Prepaint {
    lines: Vec<(usize, WrappedLine)>,
    cursor: Option<Bounds<Pixels>>,
    selection: Vec<Bounds<Pixels>>,
    scroll: Point<Pixels>,
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let input = self.input.read(cx);
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = if input.buffer.is_multiline() {
            relative(1.).into()
        } else {
            input.line_height().into()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepaint {
        let c = ActiveTheme::get(cx).colors;
        let input = self.input.read(cx);
        let multiline = input.buffer.is_multiline();
        let lh = input.line_height();
        let text = input.buffer.text().to_owned();
        let (shown, color) = if text.is_empty() {
            (input.placeholder.to_string(), c.text_muted)
        } else {
            (text.clone(), c.text_primary)
        };
        let (family, font_size) = if input.mono {
            (FONT_MONO, px(12.))
        } else {
            (FONT_UI, px(13.))
        };
        let mut base = font(family);
        base.weight = FontWeight::NORMAL;
        let run = |len: usize| TextRun {
            len,
            font: base.clone(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let marked = if text.is_empty() {
            None
        } else {
            input.buffer.marked()
        };
        let runs: Vec<TextRun> = if input.code_fences && !text.is_empty() && marked.is_none() {
            fence_runs(&text, base.clone(), font(FONT_MONO), color, c.term_bg)
        } else {
            match marked {
                Some(m) => vec![
                    run(m.start),
                    TextRun {
                        underline: Some(UnderlineStyle {
                            color: Some(color),
                            thickness: px(1.),
                            wavy: false,
                        }),
                        ..run(m.end - m.start)
                    },
                    run(shown.len() - m.end),
                ]
                .into_iter()
                .filter(|r| r.len > 0)
                .collect(),
                None => vec![run(shown.len())],
            }
        };
        let wrap = multiline.then_some(bounds.size.width);
        let shaped = window
            .text_system()
            .shape_text(shown.into(), font_size, &runs, wrap, None)
            .unwrap_or_default();
        let mut start = 0;
        let lines: Vec<(usize, WrappedLine)> = shaped
            .into_iter()
            .map(|line| {
                let s = start;
                start += line.len() + 1;
                (s, line)
            })
            .collect();

        // Positions are computed against the real text's layout (the placeholder has none).
        let input = self.input.read(cx);
        let selection = input.buffer.selection();
        let cursor_offset = input.buffer.cursor();
        let scroll = input.scroll;
        let geometry = TextGeometry {
            lines: if text.is_empty() { &[] } else { &lines },
            lh,
        };
        let cursor_at = geometry.position(cursor_offset);
        // Keep the cursor in view.
        let mut scroll = scroll;
        if multiline {
            if cursor_at.y < scroll.y {
                scroll.y = cursor_at.y;
            } else if cursor_at.y + lh > scroll.y + bounds.size.height {
                scroll.y = cursor_at.y + lh - bounds.size.height;
            }
            scroll.x = px(0.);
        } else {
            if cursor_at.x < scroll.x {
                scroll.x = cursor_at.x;
            } else if cursor_at.x + px(2.) > scroll.x + bounds.size.width {
                scroll.x = cursor_at.x + px(2.) - bounds.size.width;
            }
            scroll.y = px(0.);
        }
        let origin = bounds.origin - scroll;
        let cursor = selection.is_empty().then(|| {
            Bounds::new(
                point(origin.x + cursor_at.x, origin.y + cursor_at.y),
                size(px(1.5), lh),
            )
        });
        let selection_rects = if selection.is_empty() {
            Vec::new()
        } else {
            geometry.selection_rects(selection, origin)
        };
        Prepaint {
            lines,
            cursor,
            selection: selection_rects,
            scroll,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let c = ActiveTheme::get(cx).colors;
        let input = self.input.read(cx);
        let focus = input.focus.clone();
        let lh = input.line_height();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            for rect in prepaint.selection.drain(..) {
                window.paint_quad(fill(rect, c.selection));
            }
            let mut y = bounds.origin.y - prepaint.scroll.y;
            for (_, line) in &prepaint.lines {
                let origin = point(bounds.origin.x - prepaint.scroll.x, y);
                let _ = line.paint(origin, lh, gpui::TextAlign::Left, None, window, cx);
                y += line.size(lh).height;
            }
            if focus.is_focused(window)
                && let Some(cursor) = prepaint.cursor.take()
            {
                window.paint_quad(fill(cursor, c.focus));
            }
        });
        let lines = std::mem::take(&mut prepaint.lines);
        let scroll = prepaint.scroll;
        self.input.update(cx, |input, _| {
            input.lines = lines;
            input.bounds = Some(bounds);
            input.scroll = scroll;
        });
    }
}

/// Text runs for a Markdown body: fenced code lines (fences included) in the mono face on `code_bg`.
fn fence_runs(
    text: &str,
    body: gpui::Font,
    mono: gpui::Font,
    color: gpui::Hsla,
    code_bg: gpui::Hsla,
) -> Vec<TextRun> {
    let mut runs: Vec<TextRun> = Vec::new();
    let mut in_code = false;
    for line in text.split_inclusive('\n') {
        let fence = line.trim_start().starts_with("```");
        let code = in_code || fence;
        if fence {
            in_code = !in_code;
        }
        let run = TextRun {
            len: line.len(),
            font: if code { mono.clone() } else { body.clone() },
            color,
            background_color: code.then_some(code_bg),
            underline: None,
            strikethrough: None,
        };
        match runs.last_mut() {
            Some(last)
                if last.font == run.font && last.background_color == run.background_color =>
            {
                last.len += run.len
            }
            _ => runs.push(run),
        }
    }
    runs
}

/// Positions within shaped lines (shared by prepaint and the input itself).
struct TextGeometry<'a> {
    lines: &'a [(usize, WrappedLine)],
    lh: Pixels,
}

impl TextGeometry<'_> {
    fn position(&self, offset: usize) -> Point<Pixels> {
        let mut y = px(0.);
        for (start, line) in self.lines {
            if offset <= start + line.len() {
                let p = line
                    .position_for_index(offset - start, self.lh)
                    .unwrap_or_default();
                return point(p.x, y + p.y);
            }
            y += line.size(self.lh).height;
        }
        point(px(0.), y.max(px(0.)))
    }

    /// One rectangle per visual line the selection touches.
    fn selection_rects(&self, range: Range<usize>, origin: Point<Pixels>) -> Vec<Bounds<Pixels>> {
        let start = self.position(range.start);
        let end = self.position(range.end);
        let mut rects = Vec::new();
        let mut y = start.y;
        while y <= end.y {
            let x0 = if y == start.y { start.x } else { px(0.) };
            let x1 = if y == end.y { end.x } else { px(10_000.) };
            rects.push(Bounds::from_corners(
                point(origin.x + x0, origin.y + y),
                point(origin.x + x1, origin.y + y + self.lh),
            ));
            y += self.lh;
        }
        rects
    }
}
