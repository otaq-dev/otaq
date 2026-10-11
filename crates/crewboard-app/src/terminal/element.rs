//! Draws a terminal grid with GPUI (SES-09): a monospace grid of cells with their colours (16, 256, truecolor),
//! bold, italic, underline, strikethrough, inverse and wide characters, background runs, and the cursor in its
//! shape. Laid out like Zed's terminal element: cells are batched into runs of the same style per line, and every
//! glyph is forced to the cell width.
//!
//! The element measures its cells and tells the view how many columns and rows fit, so the PTY follows the
//! panel's size.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::Line;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::CursorShape;
use gpui::{
    App, Bounds, Element, ElementId, Entity, FontStyle, FontWeight, GlobalElementId, Hsla,
    IntoElement, LayoutId, Pixels, Point, SharedString, StrikethroughStyle, Style, TextRun,
    UnderlineStyle, Window, fill, font, point, px, relative, rgb, size,
};

use super::TerminalView;
use super::colors::{self, Base};
use crate::theme::{ActiveTheme, FONT_MONO};
use crewboard_core::session::TerminalSize;

/// Line height as a multiple of the font size (*Type scale → Terminal: 13 mono / 1.45*).
pub const LINE_HEIGHT: f32 = 1.45;

/// A cell's size for a font size, measured on the mono font.
pub fn cell_metrics(font_size: f32, window: &Window) -> (Pixels, Pixels) {
    let text_system = window.text_system();
    let font_id = text_system.resolve_font(&font(FONT_MONO));
    let width = text_system
        .advance(font_id, px(font_size), 'm')
        .map(|a| a.width)
        .unwrap_or(px(font_size * 0.6));
    (width, px((font_size * LINE_HEIGHT).round()))
}

pub struct TerminalElement {
    view: Entity<TerminalView>,
}

impl TerminalElement {
    pub fn new(view: Entity<TerminalView>) -> Self {
        Self { view }
    }
}

impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

/// A run of cells with the same style on one line.
struct Run {
    line: usize,
    column: usize,
    /// Cells covered (a wide character covers two).
    cells: usize,
    text: String,
    style: TextRun,
}

struct Rect {
    line: usize,
    column: usize,
    cells: usize,
    color: Hsla,
}

pub struct Layout {
    cell: (Pixels, Pixels),
    font_size: Pixels,
    runs: Vec<Run>,
    backgrounds: Vec<Rect>,
    cursor: Option<(Bounds<Pixels>, CursorShape, Hsla)>,
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = Layout;

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
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
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
    ) -> Layout {
        let font_size = self.view.read(cx).font_size;
        let (cell_width, line_height) = cell_metrics(font_size, window);
        let fit = TerminalSize {
            cols: (bounds.size.width / cell_width).floor().clamp(2., 1000.) as u16,
            rows: (bounds.size.height / line_height).floor().clamp(1., 1000.) as u16,
        };
        self.view.update(cx, |view, cx| {
            view.geometry = Some(super::Geometry {
                origin: bounds.origin,
                cell_width,
                line_height,
            });
            view.model.refresh_find();
            view.fit(fit, cx)
        });

        let theme = ActiveTheme::get(cx);
        let c = theme.colors;
        let base = Base {
            fg: to_rgb(c.term_fg),
            bg: to_rgb(c.term_bg),
            ansi: theme.ansi,
        };
        let view = self.view.read(cx);
        let term = &view.model.term;
        let content = term.renderable_content();
        let offset = content.display_offset as i32;
        let screen_lines = term.grid().screen_lines();
        let overrides = content.colors;
        let mono = font(FONT_MONO);
        let selection = content.selection;
        let find = &view.model.find;
        let current_match = find.current.and_then(|i| find.matches.get(i)).copied();
        let matches: Vec<_> = if find.open {
            let top = Line(-offset);
            let bottom = Line(screen_lines as i32 - 1 - offset);
            find.matches
                .iter()
                .filter(|m| m.end.line >= top && m.start.line <= bottom)
                .copied()
                .collect()
        } else {
            Vec::new()
        };
        let mut selected: Vec<Rect> = Vec::new();

        let mut runs: Vec<Run> = Vec::new();
        let mut backgrounds: Vec<Rect> = Vec::new();
        for indexed in content.display_iter {
            let line = indexed.point.line.0 + offset;
            if line < 0 || line as usize >= screen_lines {
                continue;
            }
            let (line, column) = (line as usize, indexed.point.column.0);
            let cell = indexed.cell;
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let flags = cell.flags;
            let mut fg = if flags.contains(Flags::BOLD) {
                colors::brighten(cell.fg)
            } else {
                cell.fg
            };
            let mut bg = cell.bg;
            if flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            let mut fg_rgb = colors::resolve(fg, overrides, base);
            let bg_rgb = colors::resolve(bg, overrides, base);
            if flags.contains(Flags::DIM) {
                fg_rgb = colors::dim(fg_rgb, bg_rgb);
            }
            let cells = if flags.contains(Flags::WIDE_CHAR) {
                2
            } else {
                1
            };
            if selection.is_some_and(|s| s.contains(indexed.point)) {
                let color = c.selection;
                match selected.last_mut() {
                    Some(r) if r.line == line && r.column + r.cells == column => r.cells += cells,
                    _ => selected.push(Rect {
                        line,
                        column,
                        cells,
                        color,
                    }),
                }
            }
            let found = matches.iter().find(|m| m.contains(indexed.point));
            if let Some(m) = found {
                fg_rgb = to_rgb(if Some(*m) == current_match {
                    c.st_attn
                } else {
                    c.st_paused
                });
            }
            if bg_rgb != base.bg {
                let color = rgb(bg_rgb).into();
                match backgrounds.last_mut() {
                    Some(r)
                        if r.line == line && r.column + r.cells == column && r.color == color =>
                    {
                        r.cells += cells
                    }
                    _ => backgrounds.push(Rect {
                        line,
                        column,
                        cells,
                        color,
                    }),
                }
            }
            let ch = if flags.contains(Flags::HIDDEN) {
                ' '
            } else {
                cell.c
            };
            if ch == ' ' && !flags.intersects(Flags::ALL_UNDERLINES | Flags::STRIKEOUT) {
                continue;
            }
            let mut f = mono.clone();
            if flags.contains(Flags::BOLD) || found.is_some() {
                f.weight = FontWeight::BOLD;
            }
            if flags.contains(Flags::ITALIC) {
                f.style = FontStyle::Italic;
            }
            let color: Hsla = rgb(fg_rgb).into();
            let underline = flags
                .intersects(Flags::ALL_UNDERLINES)
                .then(|| UnderlineStyle {
                    color: Some(color),
                    thickness: px(1.),
                    wavy: flags.contains(Flags::UNDERCURL),
                });
            let strikethrough = flags
                .contains(Flags::STRIKEOUT)
                .then(|| StrikethroughStyle {
                    color: Some(color),
                    thickness: px(1.),
                });
            let mut text = String::new();
            text.push(ch);
            if let Some(extra) = cell.zerowidth() {
                text.extend(extra);
            }
            let style = TextRun {
                len: text.len(),
                font: f,
                color,
                background_color: None,
                underline,
                strikethrough,
            };
            // Wide characters get a run of their own, so the next cell starts at the right column.
            match runs.last_mut() {
                Some(run)
                    if run.line == line
                        && run.column + run.cells == column
                        && cells == 1
                        && run.cells == run.text.chars().count()
                        && same_style(&run.style, &style) =>
                {
                    run.text.push_str(&text);
                    run.style.len += text.len();
                    run.cells += 1;
                }
                _ => runs.push(Run {
                    line,
                    column,
                    cells,
                    text,
                    style,
                }),
            }
        }

        let cursor_point = content.cursor.point;
        let cursor_line = cursor_point.line.0 + offset;
        let cursor = (content.cursor.shape != CursorShape::Hidden
            && cursor_line >= 0
            && (cursor_line as usize) < screen_lines
            && view.cursor_visible())
        .then(|| {
            let origin = point(
                bounds.origin.x + cell_width * cursor_point.column.0 as f32,
                bounds.origin.y + line_height * cursor_line as f32,
            );
            let wide = term.grid()[cursor_point].flags.contains(Flags::WIDE_CHAR);
            let width = if wide { cell_width * 2. } else { cell_width };
            let shape = content.cursor.shape;
            let rect = match shape {
                CursorShape::Beam => Bounds::new(origin, size(px(2.), line_height)),
                CursorShape::Underline => Bounds::new(
                    point(origin.x, origin.y + line_height - px(2.)),
                    size(width, px(2.)),
                ),
                _ => Bounds::new(origin, size(width, line_height)),
            };
            (rect, shape, c.term_fg)
        });

        backgrounds.extend(selected);
        Layout {
            cell: (cell_width, line_height),
            font_size: px(font_size),
            runs,
            backgrounds,
            cursor,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        layout: &mut Layout,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (cell_width, line_height) = layout.cell;
        let at = |line: usize, column: usize| -> Point<Pixels> {
            point(
                bounds.origin.x + cell_width * column as f32,
                bounds.origin.y + line_height * line as f32,
            )
        };
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            for r in &layout.backgrounds {
                window.paint_quad(fill(
                    Bounds::new(
                        at(r.line, r.column),
                        size(cell_width * r.cells as f32, line_height),
                    ),
                    r.color,
                ));
            }
            if let Some((rect, shape, color)) = layout.cursor
                && shape != CursorShape::HollowBlock
            {
                let color = if matches!(shape, CursorShape::Block) {
                    color.opacity(0.85)
                } else {
                    color
                };
                window.paint_quad(fill(rect, color));
            }
            for run in &layout.runs {
                let shaped = window.text_system().shape_line(
                    SharedString::from(run.text.clone()),
                    layout.font_size,
                    std::slice::from_ref(&run.style),
                    Some(cell_width),
                );
                let _ = shaped.paint(
                    at(run.line, run.column),
                    line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            if let Some((rect, shape, color)) = layout.cursor
                && shape == CursorShape::HollowBlock
            {
                window.paint_quad(gpui::outline(rect, color, gpui::BorderStyle::Solid));
            }
        });
        // A block cursor hides its character: redraw it in the background colour on top.
        if let Some((rect, CursorShape::Block, _)) = layout.cursor {
            let line = ((rect.origin.y - bounds.origin.y) / line_height).round() as usize;
            let column = ((rect.origin.x - bounds.origin.x) / cell_width).round() as usize;
            let bg = ActiveTheme::get(cx).colors.term_bg;
            if let Some(run) = layout
                .runs
                .iter()
                .find(|r| r.line == line && r.column <= column && column < r.column + r.cells)
                && r_is_simple(run)
                && let Some(ch) = run.text.chars().nth(column - run.column)
            {
                let style = TextRun {
                    len: ch.len_utf8(),
                    color: bg,
                    ..run.style.clone()
                };
                let shaped = window.text_system().shape_line(
                    SharedString::from(ch.to_string()),
                    layout.font_size,
                    &[style],
                    Some(cell_width),
                );
                let _ = shaped.paint(
                    rect.origin,
                    line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
        }
    }
}

/// One character per cell (no wide or combining characters), so a column maps to a character.
fn r_is_simple(run: &Run) -> bool {
    run.text.chars().count() == run.cells
}

fn same_style(a: &TextRun, b: &TextRun) -> bool {
    a.font == b.font
        && a.color == b.color
        && a.underline == b.underline
        && a.strikethrough == b.strikethrough
}

/// An Hsla colour as 0xRRGGBB.
pub fn to_rgb(color: Hsla) -> u32 {
    let c = color.to_rgb();
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u32;
    (byte(c.r) << 16) | (byte(c.g) << 8) | byte(c.b)
}
