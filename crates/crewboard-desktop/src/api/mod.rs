//! The request table: the door's one request call lands here and is dispatched by the area before the dot of its
//! method name (`"service.status"` → `service`). Each area has its own `api/<area>.rs` with a
//! `pub async fn request(app, method, params)` that matches its full method names and ends with
//! `_ => Err(unknown(method))`, and one line in the [`areas!`] list below.

use serde::Serialize;
use serde_json::Value;

use crate::App;
pub use crate::service::RequestError;
#[cfg(test)]
pub use board::{
    DropParams as BoardDropParams, MenuParams as CardMenuParams, SearchParams,
    TicketParams as BoardTicketParams,
};
#[cfg(test)]
pub use diff::{
    AddParams as DiffCommentAdd, FileParams as DiffFileParams, IdParams as DiffCommentId,
    Load as DiffLoad, ModeParams as DiffModeParams, Params as DiffParams,
    TextParams as DiffCommentText, ViewedParams as DiffViewedParams,
};
#[cfg(test)]
pub use drawer::{
    Button as DrawerButton, Closed as DrawerClosed, NewParams as DrawerNewParams,
    Params as DrawerParams, ResumeParams as DrawerResumeParams, TabParams as DrawerTabParams,
    View as DrawerView,
};
#[cfg(test)]
pub use editor::{
    BranchesParams as EditorBranchesParams, CompareEntry as EditorCompareEntry,
    CompareParams as EditorCompareParams, LinkParams as EditorLinkParams, Linked as EditorLinked,
    PickerParams as EditorPickerParams, PickerRowView as EditorPickerRow,
    SaveOutcome as EditorSaved, SaveParams as EditorSaveParams, View as EditorView,
};
#[cfg(test)]
pub use inbox::{
    ActRequest as InboxActParams, AnswerRequest as InboxAnswerParams, Load as InboxLoad,
    Next as InboxNext, RunBarRequest as InboxRunBarParams,
};
#[cfg(test)]
pub use keymap::Keymap;
#[cfg(test)]
pub use notices::{DismissBanner, HoldToast, NotificationKey, ToastId};
pub use onboarding::dropped as folder_dropped;
#[cfg(test)]
pub use onboarding::{
    AddOutcome as AddProjectOutcome, FormParams as AddProjectParams, Start as OnboardingStart,
};
#[cfg(test)]
pub use panel::{
    ActionParams as PanelActionParams, AnswerParams as PanelAnswerParams,
    CardParams as PanelCardParams, CloseDialog as PanelCloseDialog, Load as PanelLoad,
    MenuParams as PanelMenuParams, Params as PanelParams, ParentParams as PanelParentParams,
    PriorityParams as PanelPriorityParams, SendToFix as PanelSendToFix, Sent as PanelSent,
    SubmitDialog as PanelSubmitDialog, SubmitParams as PanelSubmitParams,
};
#[cfg(test)]
pub use review::{
    Available as ReviewAvailable, MergeParams as LocalMergeParams, Params as ReviewParams,
    SendBackParams,
};
#[cfg(test)]
pub use sections::{
    AnswerMessage as AnswerMessageParams, CloseEpicDialogView, EpicParams, EpicView, HistoryView,
    MessagesView, MoveDialogView, MoveTo as MoveToEpicParams, Params as SectionParams, StackView,
};
#[cfg(test)]
pub use settings::{
    SettingsEdit, SettingsLink, SettingsOpen, SettingsProject, SettingsRevealParams,
};
#[cfg(test)]
pub use shell::{
    Broom as CleanupBroom, DigitParams, Header as HeaderView, Params as ShellParams,
    Quick as QuickSwitcherView, QuickParams, Rail as RailView, StepParams,
};
#[cfg(test)]
pub use terminal::{
    TerminalDetach, TerminalFocus, TerminalInput, TerminalLink, TerminalResize, TerminalSettings,
};
#[cfg(test)]
pub use terminal_tab::{
    DeliverParams as TerminalTabDeliverParams, Load as TerminalTabLoad, Params as TerminalTabParams,
};
#[cfg(test)]
pub use theme::{ActiveTheme, ThemeParams, ThemePick};

/// Declares each area's module and its arm of [`request`]: one line per area.
macro_rules! areas {
    ($($area:ident,)*) => {
        $(mod $area;)*

        pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
            match method.split_once('.').map_or(method, |(area, _)| area) {
                $(stringify!($area) => $area::request(app, method, params).await,)*
                _ => Err(unknown(method)),
            }
        }
    };
}

areas! {
    service,
    theme,
    window,
    keymap,
    terminal,
    gallery,
    shell,
    notices,
    panel,
    board,
    settings,
    drawer,
    onboarding,
    sections,
    editor,
    diff,
    terminal_tab,
    review,
    inbox,
}

/// The answer to a method no area knows.
pub fn unknown(method: &str) -> RequestError {
    RequestError::plain(format!("unknown request: {method}"))
}

/// A handler's answer as the page receives it.
pub fn reply(value: &impl Serialize) -> Result<Value, RequestError> {
    serde_json::to_value(value).map_err(|e| RequestError::plain(e.to_string()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::service::testing::{Harness, connected};

    #[tokio::test(flavor = "multi_thread")]
    async fn the_table_dispatches_by_method_name_to_the_real_service() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;

        let connection = request(&h.app, "service.connection", Value::Null)
            .await
            .unwrap();
        assert_eq!(connection["state"], "connected");
        assert_eq!(connection["compatibility"], "same");

        let status = request(&h.app, "service.status", Value::Null)
            .await
            .unwrap();
        assert_eq!(status["pid"], std::process::id());
        assert_eq!(status["build_id"], connection["build_id"]);

        for method in ["board.nothing", "service.nothing", "service", ""] {
            assert_eq!(
                request(&h.app, method, json!({})).await,
                Err(unknown(method)),
                "{method:?}"
            );
        }
        assert_eq!(
            unknown("board.nothing"),
            RequestError::plain("unknown request: board.nothing")
        );
        h.service.stop_all().await;
    }
}
