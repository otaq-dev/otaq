//! OS notifications in the running service (T-0117, ATT-11; d0016, d0042): once per blocking item, never for
//! Ready for Review or a focused ticket, and the setting applies at once. A recording notifier stands in for the
//! OS.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use otaq_core::attention::notify::{Notifier, OnClick, OsNotification};
use otaq_core::board::settings::GlobalSettings;
use otaq_core::caller::Caller;
use otaq_core::client::{Client, ClientConfig};
use otaq_core::domain::Status;
use otaq_core::protocol::{Event, FocusParams, Request};
use otaq_core::service::ServiceConfig;
use otaq_core::service::testing::TestService;

/// Records what is posted, and keeps each notification's click so a test can click it.
#[derive(Default)]
struct Recorder(Mutex<Vec<OsNotification>>, Mutex<Vec<OnClick>>);

impl Notifier for Recorder {
    fn post(&self, n: &OsNotification, on_click: OnClick) {
        self.0.lock().unwrap().push(n.clone());
        self.1.lock().unwrap().push(on_click);
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
        .pin("SHOP-2", &otaq_core::domain::Actor::User("me".into()))
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

/// A click on a notification brings a connected app forward on the ticket (T-0159): the service tells every
/// subscribed client to open it with its terminal focused.
#[tokio::test(flavor = "multi_thread")]
async fn a_click_opens_the_ticket_in_the_connected_app() {
    let recorder = Arc::new(Recorder::default());
    let notifier: Arc<dyn Notifier> = recorder.clone();
    let w = TestService::start_with(move |config| ServiceConfig { notifier, ..config }).await;
    w.add_project("shop-web").await;
    let t = w.create("SHOP", "Split user full name").await;
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
    until_titles(&recorder, 1).await;

    let app = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    let mut events = app.subscribe().await.unwrap();
    let click = recorder.1.lock().unwrap().pop().unwrap();
    click();
    let opened = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(Event::OpenFromNotification { key }) = events.recv().await {
                return key;
            }
        }
    })
    .await
    .expect("the app was told to open the ticket");
    assert_eq!(opened, Some(t.key.clone()));
    w.stop().await;
}
