//! Form fields' validation state (*01 · Components → Badges & Inputs → Fields*): a field shows its error under
//! the input in red once the user has touched it or tried to save, never while they are still typing the first
//! time. Messages come from the rules here or from the service (shown exactly as it sent them).

/// One field's value and message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldState {
    pub value: String,
    /// Set after the field lost focus once, or a save was attempted.
    pub touched: bool,
    /// An error from the service for the current value (e.g. "not a git repo").
    pub server_error: Option<String>,
}

/// A rule for a field's value: `None` when fine, else the message.
pub type Rule = fn(&str) -> Option<String>;

impl FieldState {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            ..Self::default()
        }
    }

    /// Takes a new value; a service error belongs to the old value and goes.
    pub fn set(&mut self, value: impl Into<String>) {
        let value = value.into();
        if value != self.value {
            self.server_error = None;
        }
        self.value = value;
    }

    pub fn touch(&mut self) {
        self.touched = true;
    }

    /// The first problem with the value, whether or not it is shown yet.
    pub fn problem(&self, rules: &[Rule]) -> Option<String> {
        rules
            .iter()
            .find_map(|rule| rule(&self.value))
            .or_else(|| self.server_error.clone())
    }

    /// The message to show under the input.
    pub fn shown_error(&self, rules: &[Rule]) -> Option<String> {
        if let Some(server) = &self.server_error {
            return Some(server.clone());
        }
        if self.touched {
            self.problem(rules)
        } else {
            None
        }
    }

    pub fn is_valid(&self, rules: &[Rule]) -> bool {
        self.problem(rules).is_none()
    }
}

/// A title is required.
pub fn required_title(value: &str) -> Option<String> {
    value
        .trim()
        .is_empty()
        .then(|| "A ticket needs a title".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_show_once_touched() {
        let mut title = FieldState::new("");
        let rules: &[Rule] = &[required_title];
        assert!(!title.is_valid(rules));
        assert_eq!(
            title.shown_error(rules),
            None,
            "not while typing the first time"
        );
        title.touch();
        assert_eq!(
            title.shown_error(rules).as_deref(),
            Some("A ticket needs a title")
        );
        title.set("Checkout API");
        assert!(title.is_valid(rules));
        assert_eq!(title.shown_error(rules), None);
    }

    #[test]
    fn service_errors_show_at_once_and_clear_on_change() {
        let mut env = FieldState::new(".env.local");
        env.server_error = Some("File not found in repo root: .env.local".into());
        assert_eq!(
            env.shown_error(&[]).as_deref(),
            Some("File not found in repo root: .env.local")
        );
        env.set(".env.local");
        assert!(env.server_error.is_some(), "same value, same error");
        env.set(".env");
        assert_eq!(env.shown_error(&[]), None);
    }
}
