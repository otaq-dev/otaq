//! OS notifications in the running service (T-0117, ATT-11; d0016, d0042): once per blocking item, never for
//! Ready for Review or a focused ticket, and the setting applies at once. A recording notifier stands in for the
//! OS.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crewboard_core::attention::notify::{Notifier, OsNotification};
use crewboard_core::board::settings::GlobalSettings;
use crewboard_core::caller::Caller;
use crewboard_core::client::{Client, ClientConfig};
use crewboard_core::domain::Status;
use crewboard_core::protocol::{FocusParams, Request};
use crewboard_core::service::ServiceConfig;
use crewboard_core::service::testing::TestService;

#[derive(Default)]
struct Recorder(Mutex<Vec<OsNotification>>);

impl Notifier for Recorder {
    fn post(&self, n: &OsNotification) {
        self.0.lock().unwrap().push(n.clone());
    }
}

impl Recorder {
    fn titles(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|n| n.title.clone())
            .collect()
    }
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(300)).await;
}

async fn until_titles(recorder: &Recorder, n: usize) -> Vec<String> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let titles = recorder.titles();
            if titles.len() >= n {
                return titles;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{n} notifications never came: {:?}", recorder.titles()))
}

#[tokio::test(flavor = "multi_thread")]
async fn blocking_items_notify_once_and_never_while_focused() {
    let recorder = Arc::new(Recorder::default());
    let notifier: Arc<dyn Notifier> = recorder.clone();
    let w = TestService::start_with(move |config| ServiceConfig { notifier, ..config }).await;
    w.add_project("shop-web").await;
    let t = w.create("SHOP", "Split user full name").await;
    let review = w.create("SHOP", "Checkout API").await;
    w.force_status(&t, Status::Working).await;
    let shell = w.agent_shell(&t).await;
    let agent = Caller::Agent {
        ticket: t.key.clone(),
        ticket_id: t.id.clone(),
        session: shell.session.clone(),
    };
    settle().await;

    w.signals
        .ask_human(&agent, &t.id, "Should fullName be split everywhere?")
        .await
        .unwrap();
    let titles = until_titles(&recorder, 1).await;
    assert_eq!(titles, ["shop-web · SHOP-1 asks a question"]);
    assert_eq!(
        recorder.0.lock().unwrap()[0].body,
        "Should fullName be split everywhere?"
    );
    // Ready for Review never notifies; other events don't repeat the question.
    w.force_status(&review, Status::ReadyForReview).await;
    w.board
        .pin("SHOP-2", &crewboard_core::domain::Actor::User("me".into()))
        .await
        .unwrap();
    w.inbox.refresh().await.unwrap();
    settle().await;
    assert_eq!(recorder.titles().len(), 1);

    // Answered in the terminal, then a new question while the terminal has focus: nothing until it loses focus.
    w.signals.enter(&t.id).await.unwrap();
    let app = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    app.request(Request::SessionFocus(FocusParams {
        session: shell.session.clone(),
        focused: true,
    }))
    .await
    .unwrap();
    w.signals
        .ask_human(&agent, &t.id, "And the sender address?")
        .await
        .unwrap();
    w.inbox.refresh().await.unwrap();
    settle().await;
    assert_eq!(recorder.titles().len(), 1, "never for the focused ticket");
    app.request(Request::SessionFocus(FocusParams {
        session: shell.session.clone(),
        focused: false,
    }))
    .await
    .unwrap();
    let titles = until_titles(&recorder, 2).await;
    assert_eq!(titles[1], "shop-web · SHOP-1 asks a question");

    // Turned off in Settings: respected at once.
    let mut settings = w.board.global_settings().await.unwrap();
    settings.notifications = false;
    w.board.set_global_settings(&settings).await.unwrap();
    w.signals.enter(&t.id).await.unwrap();
    w.signals
        .ask_human(&agent, &t.id, "Third question?")
        .await
        .unwrap();
    w.inbox.refresh().await.unwrap();
    settle().await;
    assert_eq!(recorder.titles().len(), 2);
    let _: GlobalSettings = settings;
    w.stop().await;
}
