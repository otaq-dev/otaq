//! The ticket comment limits the app's comment box needs (CMT-02, CMT-03, CMT-11; d0109): the 2,000-character limit
//! and the 20 MB report limit, read from core's constants (one place), so the page never keeps copies of its own. A
//! view that has a comment box carries [`CommentLimits::current`].

use serde::Serialize;
use ts_rs::TS;

use otaq_core::board::reports::{REPORT_LIMIT_BYTES, size_label};
use otaq_core::board::ticket_comments::COMMENT_LIMIT_CHARS;

/// The limits, as numbers and as the size people read ("20 MB").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct CommentLimits {
    /// The longest comment, in Unicode characters (code points, as the service counts them): 2000.
    pub chars: u32,
    /// The largest report, in bytes: 20 × 1024 × 1024.
    pub report_bytes: u64,
    /// "20 MB".
    pub report_label: String,
}

impl CommentLimits {
    pub fn current() -> Self {
        Self {
            chars: u32::try_from(COMMENT_LIMIT_CHARS).unwrap_or(u32::MAX),
            report_bytes: REPORT_LIMIT_BYTES,
            report_label: size_label(REPORT_LIMIT_BYTES),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limits_come_from_core() {
        assert_eq!(
            CommentLimits::current(),
            CommentLimits {
                chars: 2000,
                report_bytes: 20 * 1024 * 1024,
                report_label: "20 MB".into(),
            }
        );
    }
}
