//! Anonymous usage data (d0085): the "Help improve Otaq" dialog, asked once after first run, and the words of its row
//! in Settings → Data. The desktop app decides when the dialog shows and what is sent; this only words them.

use serde::Serialize;
use ts_rs::TS;

use otaq_core::naming::PRODUCT_NAME;

/// The Settings → Data row's label.
pub const ROW_LABEL: &str = "Send anonymous usage data";

/// The Settings → Data row's description.
pub fn row_description() -> String {
    format!(
        "Which features are used and how often, with the {PRODUCT_NAME} version and the OS. Never ticket text, code, \
         names, paths or anything that identifies you. Off unless you turned it on."
    )
}

/// *Help improve Otaq · dialog*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "UsageDataDialog")]
pub struct Dialog {
    pub title: String,
    pub text: String,
    /// "Sent" and its lines.
    pub sent_heading: String,
    pub sent: Vec<String>,
    /// "Never sent" and its lines.
    pub never_heading: String,
    pub never: Vec<String>,
    /// "Change it anytime in Settings → Data."
    pub foot: String,
    pub decline_label: String,
    pub accept_label: String,
}

/// The dialog's words.
pub fn dialog() -> Dialog {
    Dialog {
        title: format!("Help improve {PRODUCT_NAME}"),
        text: format!(
            "Send anonymous data about which features you use, so we know what to improve. Nothing is sent unless you \
             say yes, and {PRODUCT_NAME} works the same either way."
        ),
        sent_heading: "Sent".into(),
        sent: vec![
            "Which features are used, and how often".into(),
            format!("The {PRODUCT_NAME} version and the OS"),
        ],
        never_heading: "Never sent".into(),
        never: vec![
            "Ticket text, code, diffs or prompts".into(),
            "Project, file or branch names, and paths".into(),
            "Any ID that follows you across launches".into(),
        ],
        foot: "Change it anytime in Settings → Data.".into(),
        decline_label: "No thanks".into(),
        accept_label: "Send anonymous data".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dialog_names_the_product_and_offers_both_answers() {
        let d = dialog();
        assert_eq!(d.title, format!("Help improve {PRODUCT_NAME}"));
        assert_eq!(d.decline_label, "No thanks");
        assert_eq!(d.accept_label, "Send anonymous data");
        assert!(!d.sent.is_empty() && !d.never.is_empty());
        assert!(row_description().contains("Off unless you turned it on"));
    }
}
