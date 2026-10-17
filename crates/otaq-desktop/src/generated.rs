//! The types the page reads are generated into TypeScript with `ts-rs`, never written by hand on that side (d0075
//! point 3, UIT-04). They land in `ui/src/generated/`, one file per type, committed; that folder holds only generated
//! files. One command regenerates them all:
//!
//! ```sh
//! cargo test -p otaq-desktop regenerate_types -- --ignored
//! ```
//!
//! `generated_types_are_fresh` (part of `cargo test`, so of `scripts/check.sh`) generates them into a temporary folder
//! and fails, naming the stale files, when the committed ones differ.
//!
//! A screen adds the `TS` derive (with serde) to its own types and one line per type to [`export_all`]; ts-rs also
//! writes every type a listed type refers to.

use std::collections::BTreeMap;
use std::path::Path;

use ts_rs::{Config, ExportError, TS};

/// The committed folder, relative to this crate.
const FOLDER: &str = "ui/src/generated";

fn export_all(dir: &Path) -> Result<(), ExportError> {
    // 64-bit integers arrive as JSON numbers (serde_json), not as `bigint`.
    let cfg = Config::new().with_out_dir(dir).with_large_int("number");
    otaq_view::keymap::Modifiers::export_all(&cfg)?;
    otaq_view::keymap::KeyAction::export_all(&cfg)?;
    otaq_view::view_models::panel::Tab::export_all(&cfg)?;
    otaq_view::terminal::Phase::export_all(&cfg)?;
    // Also writes `ServiceConnection` and `ServiceCompatibility`, which it refers to.
    crate::events::PageEvent::export_all(&cfg)?;
    crate::api::RequestError::export_all(&cfg)?;
    otaq_view::icons::IconName::export_all(&cfg)?;
    crate::api::ThemeParams::export_all(&cfg)?;
    crate::api::ActiveTheme::export_all(&cfg)?;
    crate::window::Chrome::export_all(&cfg)?;
    crate::window::Action::export_all(&cfg)?;
    // Also writes `KeyBinding`, `Keystroke` and `KeyPlatform`.
    crate::api::Keymap::export_all(&cfg)?;
    // Also writes `SizeClass`, `SizeClassBound`, `ClassWidths`, `MainTab` and `MainTabView`.
    otaq_view::view_models::layout::View::export_all(&cfg)?;
    crate::terminal::Feed::export_all(&cfg)?;
    crate::api::TerminalInput::export_all(&cfg)?;
    crate::api::TerminalResize::export_all(&cfg)?;
    crate::api::TerminalFocus::export_all(&cfg)?;
    crate::api::TerminalDetach::export_all(&cfg)?;
    // Also writes `TerminalRenderer`.
    crate::api::TerminalSettings::export_all(&cfg)?;
    crate::api::TerminalLink::export_all(&cfg)?;
    crate::api::ThemePick::export_all(&cfg)?;
    // The "Help improve Otaq" dialog (d0085); also writes `UsageDataDialog`.
    crate::api::UsageDataAsk::export_all(&cfg)?;
    crate::api::UsageChoice::export_all(&cfg)?;
    otaq_view::view_models::theme_picker::PickerSection::export_all(&cfg)?;
    otaq_view::view_models::status::PillView::export_all(&cfg)?;
    otaq_view::view_models::priority::PriorityView::export_all(&cfg)?;
    otaq_view::view_models::menu::MenuItem::export_all(&cfg)?;
    // Also writes the legend, keyboard-map, type-scale and icon rows it is made of.
    otaq_view::view_models::gallery::GalleryView::export_all(&cfg)?;
    // The header, rail and quick switcher (T-0219); each also writes the view-model types it is made of.
    crate::api::ShellParams::export_all(&cfg)?;
    crate::api::HeaderView::export_all(&cfg)?;
    crate::api::RailView::export_all(&cfg)?;
    crate::api::QuickParams::export_all(&cfg)?;
    crate::api::QuickSwitcherView::export_all(&cfg)?;
    crate::api::DigitParams::export_all(&cfg)?;
    crate::api::StepParams::export_all(&cfg)?;
    crate::api::CleanupBroom::export_all(&cfg)?;
    // T-0231: `PageEvent` writes `Notices` with its banner and toast types; these are the requests' own.
    otaq_view::view_models::toasts::NewToast::export_all(&cfg)?;
    otaq_view::view_models::restart::RestartDialogView::export_all(&cfg)?;
    crate::api::DismissBanner::export_all(&cfg)?;
    crate::api::ToastId::export_all(&cfg)?;
    crate::api::HoldToast::export_all(&cfg)?;
    crate::api::ToastPress::export_all(&cfg)?;
    crate::api::NotificationKey::export_all(&cfg)?;
    crate::notices::OpenTarget::export_all(&cfg)?;
    // Also writes the Ticket Card and Epic Card shapes it is made of.
    otaq_view::view_models::card_page::CardGallery::export_all(&cfg)?;
    // The side panel (T-0222): the view writes its header, buttons, tabs and callouts; then the requests' own types.
    crate::api::PanelLoad::export_all(&cfg)?;
    crate::api::PanelParams::export_all(&cfg)?;
    crate::api::PanelActionParams::export_all(&cfg)?;
    crate::api::PanelMenuParams::export_all(&cfg)?;
    crate::api::PanelPriorityParams::export_all(&cfg)?;
    crate::api::PanelCloseDialog::export_all(&cfg)?;
    crate::api::PanelSubmitDialog::export_all(&cfg)?;
    crate::api::PanelSubmitParams::export_all(&cfg)?;
    crate::api::PanelAnswerParams::export_all(&cfg)?;
    crate::api::PanelSent::export_all(&cfg)?;
    crate::api::PanelSendToFix::export_all(&cfg)?;
    crate::api::PanelParentParams::export_all(&cfg)?;
    crate::api::PanelCardParams::export_all(&cfg)?;
    crate::api::PanelAnswerAgentParams::export_all(&cfg)?;
    // The Terminals drawer and its header button (T-0242); the view also writes its tabs.
    crate::api::DrawerParams::export_all(&cfg)?;
    crate::api::DrawerButton::export_all(&cfg)?;
    crate::api::DrawerView::export_all(&cfg)?;
    crate::api::DrawerTabParams::export_all(&cfg)?;
    crate::api::DrawerNewParams::export_all(&cfg)?;
    // The Project agent pane (T-0326): its view, and the requests' params and answers.
    otaq_view::view_models::agent_pane::PaneView::export_all(&cfg)?;
    crate::api::PaneParams::export_all(&cfg)?;
    crate::api::PaneSendParams::export_all(&cfg)?;
    crate::api::PaneRenameParams::export_all(&cfg)?;
    crate::api::PaneEndParams::export_all(&cfg)?;
    crate::api::PaneEnded::export_all(&cfg)?;
    crate::api::PaneProjectParams::export_all(&cfg)?;
    crate::api::PaneResumeParams::export_all(&cfg)?;
    // The board (T-0220): its view with the cards, the drag's one answer, the drop's outcome, and the requests'.
    otaq_view::view_models::board_page::BoardPage::export_all(&cfg)?;
    otaq_view::view_models::board_page::Params::export_all(&cfg)?;
    otaq_view::view_models::board_page::DragAnswer::export_all(&cfg)?;
    otaq_view::view_models::board_page::DropOutcome::export_all(&cfg)?;
    crate::api::BoardTicketParams::export_all(&cfg)?;
    crate::api::BoardDropParams::export_all(&cfg)?;
    // First run and Add project (T-0233).
    otaq_view::view_models::onboarding_page::Texts::export_all(&cfg)?;
    otaq_view::view_models::onboarding_page::Tools::export_all(&cfg)?;
    otaq_view::view_models::onboarding_page::StepId::export_all(&cfg)?;
    crate::api::OnboardingStart::export_all(&cfg)?;
    crate::api::AddProjectParams::export_all(&cfg)?;
    crate::api::AddProjectOutcome::export_all(&cfg)?;
    // The board's search and filter, card menu and delete confirmation (T-0221).
    otaq_view::view_models::search_page::SearchView::export_all(&cfg)?;
    otaq_view::view_models::card_menu::Choice::export_all(&cfg)?;
    otaq_view::view_models::delete::DeleteText::export_all(&cfg)?;
    crate::api::SearchParams::export_all(&cfg)?;
    crate::api::CardMenuParams::export_all(&cfg)?;
    crate::api::BoardFocusParams::export_all(&cfg)?;
    // Settings (T-0234); the view also writes its sections, rows, controls and nav.
    otaq_view::view_models::settings_page::View::export_all(&cfg)?;
    otaq_view::view_models::settings_page::ToolsView::export_all(&cfg)?;
    otaq_view::view_models::settings_page::ProviderView::export_all(&cfg)?;
    otaq_view::view_models::settings_page::RemoveView::export_all(&cfg)?;
    otaq_view::view_models::settings_page::CreditsView::export_all(&cfg)?;
    otaq_view::view_models::settings_page::EnvVarsGallery::export_all(&cfg)?;
    otaq_view::view_models::settings_page::WarmSwitchGallery::export_all(&cfg)?;
    otaq_view::view_models::about::View::export_all(&cfg)?;
    otaq_view::view_models::about::LicenseView::export_all(&cfg)?;
    // Also writes its bundled rows, groups and items.
    otaq_view::view_models::licenses::View::export_all(&cfg)?;
    crate::api::SettingsOpen::export_all(&cfg)?;
    crate::api::SettingsEdit::export_all(&cfg)?;
    crate::api::SettingsProject::export_all(&cfg)?;
    crate::api::SettingsLink::export_all(&cfg)?;
    crate::api::SettingsRevealParams::export_all(&cfg)?;
    // The panel's Stack, Messages and epic sections, the History tab and the epic dialogs (T-0223).
    crate::api::SectionParams::export_all(&cfg)?;
    crate::api::StackView::export_all(&cfg)?;
    crate::api::MessagesView::export_all(&cfg)?;
    crate::api::AnswerMessageParams::export_all(&cfg)?;
    crate::api::HistoryView::export_all(&cfg)?;
    crate::api::EpicView::export_all(&cfg)?;
    crate::api::MoveDialogView::export_all(&cfg)?;
    crate::api::MoveToEpicParams::export_all(&cfg)?;
    crate::api::EpicParams::export_all(&cfg)?;
    crate::api::CloseEpicDialogView::export_all(&cfg)?;
    // The ticket editor and the Ticket tab (T-0228); the view also writes its form, texts, fields and read mode.
    crate::api::EditorView::export_all(&cfg)?;
    crate::api::EditorSaveParams::export_all(&cfg)?;
    crate::api::EditorSaved::export_all(&cfg)?;
    crate::api::EditorPickerParams::export_all(&cfg)?;
    crate::api::EditorPickerRow::export_all(&cfg)?;
    crate::api::EditorLinkParams::export_all(&cfg)?;
    crate::api::EditorLinked::export_all(&cfg)?;
    crate::api::EditorBranchesParams::export_all(&cfg)?;
    crate::api::EditorCompareParams::export_all(&cfg)?;
    crate::api::EditorCompareEntry::export_all(&cfg)?;
    crate::api::EditorSourceModeParams::export_all(&cfg)?;
    // The Diff tab (T-0225): the tab, a file's lines and the comments; then the requests' own types.
    crate::api::DiffLoad::export_all(&cfg)?;
    otaq_view::view_models::diff_page::FileView::export_all(&cfg)?;
    otaq_view::view_models::diff_page::CommentsView::export_all(&cfg)?;
    crate::api::DiffParams::export_all(&cfg)?;
    crate::api::DiffFileParams::export_all(&cfg)?;
    crate::api::DiffViewedParams::export_all(&cfg)?;
    crate::api::DiffModeParams::export_all(&cfg)?;
    crate::api::DiffCommentAdd::export_all(&cfg)?;
    crate::api::DiffCommentText::export_all(&cfg)?;
    crate::api::DiffCommentId::export_all(&cfg)?;
    // Review (T-0226): Approve ▾ / Local merge and Send back, their results and requests.
    otaq_view::view_models::review_page::ApproveView::export_all(&cfg)?;
    otaq_view::view_models::review_page::MergeResult::export_all(&cfg)?;
    otaq_view::view_models::review_page::SendBackView::export_all(&cfg)?;
    crate::api::ReviewParams::export_all(&cfg)?;
    crate::api::ReviewAvailable::export_all(&cfg)?;
    crate::api::LocalMergeParams::export_all(&cfg)?;
    crate::api::SendBackParams::export_all(&cfg)?;
    // Recorded decisions in the review (T-0287); also writes the item, its mark, status, labels, groups and child.
    otaq_view::view_models::decision_item::DecisionsSection::export_all(&cfg)?;
    otaq_view::view_models::decision_item::Gallery::export_all(&cfg)?;
    // Change… and Add a fix child (T-0288): the dialog's view and its requests.
    otaq_view::view_models::decision_change::ChangeView::export_all(&cfg)?;
    otaq_view::view_models::decision_change::Gallery::export_all(&cfg)?;
    crate::api::DecisionChangeViewParams::export_all(&cfg)?;
    crate::api::DecisionChangeParams::export_all(&cfg)?;
    // The Terminal tab (T-0224); the load also writes the view, its strips, hints and recovery.
    crate::api::TerminalTabLoad::export_all(&cfg)?;
    crate::api::TerminalTabParams::export_all(&cfg)?;
    crate::api::TerminalTabDeliverParams::export_all(&cfg)?;
    // The inbox and ⌘J (T-0230); the load also writes the view, groups, rows, answers and buttons.
    crate::api::InboxLoad::export_all(&cfg)?;
    crate::api::InboxAnswerParams::export_all(&cfg)?;
    crate::api::InboxActParams::export_all(&cfg)?;
    crate::api::InboxNext::export_all(&cfg)?;
    crate::api::InboxRunBarParams::export_all(&cfg)?;
    otaq_view::view_models::inbox_page::RunBarView::export_all(&cfg)?;
    otaq_view::view_models::inbox_page::ResumeAllDialog::export_all(&cfg)?;
    otaq_view::view_models::inbox_page::LeadGallery::export_all(&cfg)?;
    // Open PR / Push to PR (T-0227): the answer also writes the dialog's view, its stages and the Push intro.
    crate::api::PrAnswer::export_all(&cfg)?;
    otaq_view::view_models::pr_page::OpenBox::export_all(&cfg)?;
    crate::api::PrOpenParams::export_all(&cfg)?;
    crate::api::PrStartParams::export_all(&cfg)?;
    crate::api::PrParams::export_all(&cfg)?;
    crate::api::PrEditParams::export_all(&cfg)?;
    crate::api::PrCheckParams::export_all(&cfg)?;
    crate::api::PrConfirmParams::export_all(&cfg)?;
    // Recovery and Cleanup (T-0235): the after-restart sheet, Start from scratch, the Cleanup overlay; then the
    // requests' own types.
    otaq_view::view_models::after_restart::SummaryPage::export_all(&cfg)?;
    otaq_view::view_models::recovery::ScratchConfirm::export_all(&cfg)?;
    otaq_view::view_models::cleanup::Page::export_all(&cfg)?;
    otaq_view::view_models::cleanup::Gallery::export_all(&cfg)?;
    crate::api::AfterRestartParams::export_all(&cfg)?;
    crate::api::RecoveryResumeParams::export_all(&cfg)?;
    crate::api::CleanupParams::export_all(&cfg)?;
    crate::api::CleanupToggleParams::export_all(&cfg)?;
    crate::api::CleanupGroupParams::export_all(&cfg)?;
    crate::api::CleanupWarmClearParams::export_all(&cfg)?;
    // Run, the run-command dialog, the processes and the Shell tab (T-0232).
    otaq_view::view_models::run_page::TabView::export_all(&cfg)?;
    otaq_view::view_models::run_page::ProcessesView::export_all(&cfg)?;
    otaq_view::view_models::run_page::ShellView::export_all(&cfg)?;
    crate::api::RunStarted::export_all(&cfg)?;
    crate::api::RunCommandOpened::export_all(&cfg)?;
    crate::api::RunParams::export_all(&cfg)?;
    crate::api::RunOpenParams::export_all(&cfg)?;
    crate::api::RunCommandOpenParams::export_all(&cfg)?;
    crate::api::RunCommandParams::export_all(&cfg)?;
    crate::api::RunCommandSuggestionParams::export_all(&cfg)?;
    crate::api::RunCommandSaveParams::export_all(&cfg)?;
    crate::api::ShellTabParams::export_all(&cfg)?;
    // Validation reports in History and Review, Create ticket from a note (T-0238); HistoryView writes the rest.
    otaq_view::view_models::validation_report::Review::export_all(&cfg)?;
    crate::api::CreateFromNoteParams::export_all(&cfg)?;
    // The "Epics in flight" strip (T-0268): also writes its tracks, stations, stops, chips and stop states.
    otaq_view::view_models::epic_strip::Strip::export_all(&cfg)?;
    // The Lead Status gallery (T-0289); `LeadStatusView` itself comes with the epic card and the track.
    otaq_view::view_models::lead_status::Gallery::export_all(&cfg)?;
    // The epic panel's Lead tab (T-0290): its view, body, changes and gallery.
    crate::api::LeadTabLoad::export_all(&cfg)?;
    crate::api::LeadTabParams::export_all(&cfg)?;
    otaq_view::view_models::lead_tab::Gallery::export_all(&cfg)?;
    // The folder trust gallery (T-0306): the add-project forms and the two banners.
    otaq_view::view_models::onboarding_page::TrustGallery::export_all(&cfg)?;
    // The tool avatars' gallery (T-0321); it writes `ToolAvatarView` and `AvatarTool` too.
    otaq_view::view_models::tool_avatar::Gallery::export_all(&cfg)?;
    // The board's gallery section (T-0324): the Review column, the drop verdicts, the empty board.
    otaq_view::view_models::board_page::Gallery::export_all(&cfg)?;
    // Activity's popover and gallery (T-0325); the header writes the button.
    otaq_view::view_models::activity::Popover::export_all(&cfg)?;
    otaq_view::view_models::activity::Gallery::export_all(&cfg)?;
    // The pane's gallery section (T-0330): the pane in each state.
    otaq_view::view_models::agent_pane::Gallery::export_all(&cfg)?;
    Ok(())
}

/// Each file in `dir` by name, with its text. Line endings are compared as `\n`, so a Windows checkout isn't stale.
fn files(dir: &Path) -> BTreeMap<String, String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return BTreeMap::new();
    };
    entries
        .flatten()
        .map(|e| {
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            (
                e.file_name().to_string_lossy().into_owned(),
                text.replace("\r\n", "\n"),
            )
        })
        .collect()
}

/// The files of `committed` that don't match `fresh`: changed, missing, or not generated at all.
fn stale_files(
    fresh: &BTreeMap<String, String>,
    committed: &BTreeMap<String, String>,
) -> Vec<String> {
    let changed = fresh
        .iter()
        .filter(|(name, text)| committed.get(*name) != Some(text))
        .map(|(name, _)| name.clone());
    let extra = committed
        .keys()
        .filter(|name| !fresh.contains_key(*name))
        .map(|name| format!("{name} (not generated)"));
    changed.chain(extra).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn committed() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(FOLDER)
    }

    #[test]
    fn generated_types_are_fresh() {
        let fresh = tempfile::tempdir().unwrap();
        export_all(fresh.path()).unwrap();
        let stale = stale_files(&files(fresh.path()), &files(&committed()));
        assert!(
            stale.is_empty(),
            "{FOLDER} is stale: {}. Run `cargo test -p {} regenerate_types -- --ignored` to regenerate.",
            stale.join(", "),
            env!("CARGO_PKG_NAME"),
        );
    }

    /// The one command that regenerates the folder (see the module docs).
    #[test]
    #[ignore = "writes into the source tree; run on purpose to regenerate"]
    fn regenerate_types() {
        let dir = committed();
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        export_all(&dir).unwrap();
    }

    #[test]
    fn stale_files_names_changed_missing_and_extra_files() {
        let map = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
            pairs
                .iter()
                .map(|(n, t)| (n.to_string(), t.to_string()))
                .collect()
        };
        let fresh = map(&[("A.ts", "a"), ("B.ts", "b"), ("C.ts", "c")]);
        let committed = map(&[("A.ts", "a"), ("B.ts", "old"), ("Hand.ts", "x")]);
        assert_eq!(
            stale_files(&fresh, &committed),
            ["B.ts", "C.ts", "Hand.ts (not generated)"]
        );
        assert!(stale_files(&fresh, &fresh).is_empty());
    }

    #[test]
    fn line_endings_dont_make_files_stale() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("A.ts"), "one\r\ntwo\r\n").unwrap();
        assert_eq!(files(dir.path())["A.ts"], "one\ntwo\n");
    }
}
