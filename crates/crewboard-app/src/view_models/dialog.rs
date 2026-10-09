//! Confirm dialogs (*01 · Components → Overlays & feedback → Confirm dialog*): Esc always cancels. ↵ presses
//! the default button: the primary action in an ordinary dialog, but Cancel in a destructive one, because the
//! primary is never pre-focused on destructive dialogs (design note; T-0162 "Cancel is the default button").

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    /// The confirm button is the accent primary and the default.
    Primary,
    /// The confirm button is destructive; Cancel is the default.
    Destructive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKey {
    Enter,
    Escape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogOutcome {
    Confirm,
    Cancel,
}

/// What a key does in a dialog of this kind.
pub fn on_key(kind: DialogKind, key: DialogKey) -> DialogOutcome {
    match (kind, key) {
        (_, DialogKey::Escape) => DialogOutcome::Cancel,
        (DialogKind::Primary, DialogKey::Enter) => DialogOutcome::Confirm,
        (DialogKind::Destructive, DialogKey::Enter) => DialogOutcome::Cancel,
    }
}

/// Which button is drawn as the focused default.
pub fn default_outcome(kind: DialogKind) -> DialogOutcome {
    on_key(kind, DialogKey::Enter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_cancels_enter_presses_the_default() {
        assert_eq!(
            on_key(DialogKind::Primary, DialogKey::Escape),
            DialogOutcome::Cancel
        );
        assert_eq!(
            on_key(DialogKind::Destructive, DialogKey::Escape),
            DialogOutcome::Cancel
        );
        assert_eq!(
            on_key(DialogKind::Primary, DialogKey::Enter),
            DialogOutcome::Confirm
        );
        assert_eq!(
            on_key(DialogKind::Destructive, DialogKey::Enter),
            DialogOutcome::Cancel,
            "destructive actions are never the default"
        );
        assert_eq!(
            default_outcome(DialogKind::Destructive),
            DialogOutcome::Cancel
        );
    }
}
