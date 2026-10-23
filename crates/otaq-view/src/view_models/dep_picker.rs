//! The Add picker's list in the sheet's Dependencies section (DPV-04, *24 → Adding and removing* `s24-11`; the
//! editor's Depends-on picker before M38): typing opens it; picking a ticket links it and closes it (typing again
//! adds another); Esc, leaving the field or a click outside close it without picking. The highlighted row (the
//! design's accent row) is the one ↵ picks: the hovered or arrowed-to row, else the first one that can be picked.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DepPicker {
    open: bool,
    hovered: Option<usize>,
}

impl DepPicker {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The query changed.
    pub fn typed(&mut self) {
        self.open = true;
        self.hovered = None;
    }

    /// A row was picked (click or ↵).
    pub fn picked(&mut self) {
        self.close();
    }

    /// Esc: closes an open list and says so; a closed list leaves Esc to the editor (which closes).
    pub fn escape(&mut self) -> bool {
        let was_open = self.open;
        self.close();
        was_open
    }

    /// The field lost focus or the mouse went down outside the field and its list.
    pub fn dismiss(&mut self) {
        self.close();
    }

    /// The mouse is over `row`; says whether the highlight moved.
    pub fn hover(&mut self, row: usize) -> bool {
        let moved = self.hovered != Some(row);
        self.hovered = Some(row);
        moved
    }

    /// ↑ / ↓: the previous or next row that can be picked, stopping at the ends.
    pub fn step(&mut self, delta: isize, enabled: &[bool]) {
        let Some(from) = self.highlighted(enabled) else {
            return;
        };
        let mut i = from as isize + delta;
        while let Some(&e) = usize::try_from(i).ok().and_then(|u| enabled.get(u)) {
            if e {
                self.hovered = Some(i as usize);
                return;
            }
            i += delta;
        }
    }

    /// The row ↵ picks, given which rows can be picked.
    pub fn highlighted(&self, enabled: &[bool]) -> Option<usize> {
        self.hovered
            .filter(|&i| enabled.get(i).copied().unwrap_or(false))
            .or_else(|| enabled.iter().position(|&e| e))
    }

    fn close(&mut self) {
        self.open = false;
        self.hovered = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_opens_and_picking_closes() {
        let mut p = DepPicker::default();
        assert!(!p.is_open());
        p.typed();
        assert!(p.is_open());
        p.picked();
        assert!(!p.is_open(), "the list closes once a ticket is picked");
        p.typed();
        assert!(p.is_open(), "typing again opens it for the next dependency");
    }

    #[test]
    fn escape_closes_the_list_before_the_editor() {
        let mut p = DepPicker::default();
        p.typed();
        assert!(p.escape(), "the first Esc only closes the list");
        assert!(!p.is_open());
        assert!(!p.escape(), "the next Esc is the editor's");
    }

    #[test]
    fn a_click_outside_or_blur_closes() {
        let mut p = DepPicker::default();
        p.typed();
        p.dismiss();
        assert!(!p.is_open());
    }

    #[test]
    fn highlight_follows_the_mouse_else_the_first_pickable_row() {
        let mut p = DepPicker::default();
        p.typed();
        let enabled = [false, true, true];
        assert_eq!(p.highlighted(&enabled), Some(1));
        p.hover(2);
        assert_eq!(p.highlighted(&enabled), Some(2));
        p.hover(0);
        assert_eq!(
            p.highlighted(&enabled),
            Some(1),
            "the edited ticket can't be picked"
        );
        p.typed();
        p.hover(2);
        p.picked();
        p.typed();
        assert_eq!(
            p.highlighted(&enabled),
            Some(1),
            "a new query starts at the top"
        );
        assert_eq!(p.highlighted(&[false]), None);
    }

    #[test]
    fn arrows_skip_rows_that_cant_be_picked() {
        let mut p = DepPicker::default();
        p.typed();
        let enabled = [true, false, true];
        p.step(1, &enabled);
        assert_eq!(p.highlighted(&enabled), Some(2));
        p.step(1, &enabled);
        assert_eq!(p.highlighted(&enabled), Some(2), "stops at the last row");
        p.step(-1, &enabled);
        assert_eq!(p.highlighted(&enabled), Some(0));
        p.step(-1, &enabled);
        assert_eq!(p.highlighted(&enabled), Some(0), "stops at the first row");
    }
}
