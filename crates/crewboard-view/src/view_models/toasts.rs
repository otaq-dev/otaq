//! The toast host's rules (*01 · Components → Overlays & feedback*): toasts stack bottom-right, at most three,
//! success toasts hide after 6 s, error toasts stay until dismissed.

use std::time::{Duration, Instant};

/// How long a success toast stays.
pub const AUTO_HIDE: Duration = Duration::from_secs(6);
/// Toasts shown at once; older ones make room.
pub const MAX_TOASTS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Success,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastItem {
    pub id: u64,
    pub level: ToastLevel,
    pub title: String,
    pub text: Option<String>,
    /// The action link's label (e.g. "Copy command") and the text it copies.
    pub copy_action: Option<(String, String)>,
    pub shown_at: Instant,
}

#[derive(Debug, Default)]
pub struct ToastModel {
    items: Vec<ToastItem>,
    next_id: u64,
}

impl ToastModel {
    pub fn push(
        &mut self,
        level: ToastLevel,
        title: impl Into<String>,
        text: Option<String>,
        now: Instant,
    ) -> u64 {
        self.push_item(level, title.into(), text, None, now)
    }

    /// An error toast with an action link that copies `command` to the clipboard.
    pub fn push_with_copy(
        &mut self,
        title: impl Into<String>,
        text: Option<String>,
        label: impl Into<String>,
        command: impl Into<String>,
        now: Instant,
    ) -> u64 {
        self.push_item(
            ToastLevel::Error,
            title.into(),
            text,
            Some((label.into(), command.into())),
            now,
        )
    }

    fn push_item(
        &mut self,
        level: ToastLevel,
        title: String,
        text: Option<String>,
        copy_action: Option<(String, String)>,
        now: Instant,
    ) -> u64 {
        self.next_id += 1;
        self.items.push(ToastItem {
            id: self.next_id,
            level,
            title,
            text,
            copy_action,
            shown_at: now,
        });
        if self.items.len() > MAX_TOASTS {
            let overflow = self.items.len() - MAX_TOASTS;
            self.items.drain(..overflow);
        }
        self.next_id
    }

    pub fn dismiss(&mut self, id: u64) {
        self.items.retain(|t| t.id != id);
    }

    /// Removes success toasts older than [`AUTO_HIDE`]. Returns whether anything changed.
    pub fn expire(&mut self, now: Instant) -> bool {
        let before = self.items.len();
        self.items
            .retain(|t| t.level == ToastLevel::Error || now.duration_since(t.shown_at) < AUTO_HIDE);
        before != self.items.len()
    }

    /// Oldest first; drawn bottom-up so the newest sits at the bottom.
    pub fn items(&self) -> &[ToastItem] {
        &self.items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_toasts_time_out_errors_stay() {
        let t0 = Instant::now();
        let mut m = ToastModel::default();
        m.push(ToastLevel::Success, "Reconnected", None, t0);
        let err = m.push_with_copy(
            "gh is not logged in",
            None,
            "Copy command",
            "gh auth login",
            t0,
        );
        assert!(!m.expire(t0 + Duration::from_secs(5)));
        assert_eq!(m.items().len(), 2);
        assert!(m.expire(t0 + AUTO_HIDE));
        assert_eq!(m.items().len(), 1);
        assert_eq!(m.items()[0].id, err);
        assert!(!m.expire(t0 + Duration::from_secs(600)), "errors stay");
        m.dismiss(err);
        assert!(m.items().is_empty());
    }

    #[test]
    fn at_most_three_newest_kept() {
        let t0 = Instant::now();
        let mut m = ToastModel::default();
        for i in 0..5 {
            m.push(ToastLevel::Success, format!("t{i}"), None, t0);
        }
        let titles: Vec<&str> = m.items().iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["t2", "t3", "t4"]);
    }

    #[test]
    fn error_toasts_carry_their_command() {
        let mut m = ToastModel::default();
        m.push_with_copy(
            "gh is not logged in",
            Some("Run gh auth login, then retry Open PR.".into()),
            "Copy command",
            "gh auth login",
            Instant::now(),
        );
        let item = &m.items()[0];
        assert_eq!(item.level, ToastLevel::Error);
        assert_eq!(
            item.copy_action,
            Some(("Copy command".into(), "gh auth login".into()))
        );
    }
}
