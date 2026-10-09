//! The service restart confirmation (*11 · Banners → Service restart · confirm*): it names exactly who gets
//! interrupted, and only the confirm button restarts (d0022: the user picks the moment).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartDialog {
    pub working: u32,
    pub waiting: u32,
}

impl RestartDialog {
    pub const TITLE: &str = "Restart the background service?";
    pub const DETAIL: &str = "Waiting tickets keep their questions. Review and Approved tickets aren’t touched. Run and Shell processes stop.";
    pub const CONFIRM: &str = "Restart service";

    /// "3 working agents and 2 waiting ones will be interrupted. You can Resume all afterwards."
    pub fn message(&self) -> String {
        match (self.working, self.waiting) {
            (0, 0) => {
                "No agents are working or waiting, so nothing will be interrupted.".to_owned()
            }
            (working, waiting) => {
                let working = match working {
                    1 => "1 working agent".to_owned(),
                    n => format!("{n} working agents"),
                };
                let waiting = match waiting {
                    1 => "1 waiting one".to_owned(),
                    n => format!("{n} waiting ones"),
                };
                format!(
                    "{working} and {waiting} will be interrupted. You can Resume all afterwards."
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_who_is_interrupted() {
        assert_eq!(
            RestartDialog {
                working: 3,
                waiting: 2
            }
            .message(),
            "3 working agents and 2 waiting ones will be interrupted. You can Resume all afterwards."
        );
        assert_eq!(
            RestartDialog {
                working: 1,
                waiting: 1
            }
            .message(),
            "1 working agent and 1 waiting one will be interrupted. You can Resume all afterwards."
        );
        assert!(
            RestartDialog {
                working: 0,
                waiting: 0
            }
            .message()
            .starts_with("No agents")
        );
    }
}
