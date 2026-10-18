//! OS notifications in the running service (T-0117, ATT-11; d0016, d0042, d0090): once per blocking item, never for
//! Ready for Review or a focused ticket, never while an app window is focused, at most one pop-up per quiet period,
//! and the setting applies at once. A recording notifier stands in for the OS and a fake clock for the time: nothing
//! is posted for real and nothing sleeps for 2 minutes.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use otaq_core::attention::notify::pace::{FakeClock, QUIET};
use otaq_core::attention::notify::{Notifier, OnClick, OsNotification, Target};
use otaq_core::board::settings::GlobalSettings;
use otaq_core::caller::Caller;
use otaq_core::client::{Client, ClientConfig};
use otaq_core::domain::Status;
use otaq_core::protocol::{AppFocusParams, Event, FocusParams, Request};
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

/// A service whose notifications go to a recorder, on a fake clock.
async fn world() -> (TestService, Arc<Recorder>, Arc<FakeClock>) {
    let recorder = Arc::new(Recorder::default());
    let clock = Arc::new(FakeClock::new());
    let (notifier, time): (Arc<dyn Notifier>, Arc<FakeClock>) = (recorder.clone(), clock.clone());
    let w = TestService::start_with(move |config| ServiceConfig {
        notifier,
        clock: time,
        ..config
    })
    .await;
    w.add_project("shop-web").await;
    (w, recorder, clock)
}

/// An agent session for a new Working ticket, and the caller its hooks come from.
async fn working(
    w: &TestService,
    title: &str,
) -> (otaq_core::board::tickets::Ticket, Caller, String) {
    let t = w.create("SHOP", title).await;
    w.force_status(&t, Status::Working).await;
    let shell = w.agent_shell(&t).await;
    let agent = Caller::Agent {
        ticket: t.key.clone(),
        ticket_id: t.id.clone(),
        session: shell.session.clone(),
    };
    (t, agent, shell.session.clone())
}

#[tokio::test(flavor = "multi_thread")]
async fn blocking_items_notify_once_and_never_while_focused() {
    let (w, recorder, clock) = world().await;
    let review = w.create("SHOP", "Checkout API").await;
    let (t, agent, session) = working(&w, "Split user full name").await;
    settle().await;

    w.signals
        .ask_human(&agent, &t.id, "Should fullName be split everywhere?")
        .await
        .unwrap();
    let titles = until_titles(&recorder, 1).await;
    assert_eq!(titles, ["SHOP-2 asks a question"]);
    {
        let posted = recorder.0.lock().unwrap();
        assert_eq!(posted[0].subtitle, "shop-web · Split user full name");
        assert_eq!(posted[0].body, "Should fullName be split everywhere?");
        assert_eq!(posted[0].thread.as_deref(), Some("SHOP"));
        assert!(posted[0].sound);
    }
    // Ready for Review never notifies; other events don't repeat the question.
    w.force_status(&review, Status::ReadyForReview).await;
    w.board
        .pin("SHOP-1", &otaq_core::domain::Actor::User("me".into()))
        .await
        .unwrap();
    w.inbox.refresh().await.unwrap();
    settle().await;
    assert_eq!(recorder.titles().len(), 1);

    // Past the quiet period. Answered in the terminal, then a new question while the terminal has focus: nothing
    // until it loses focus.
    clock.advance(QUIET);
    w.signals.enter(&t.id).await.unwrap();
    let app = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    app.request(Request::SessionFocus(FocusParams {
        session: session.clone(),
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
        session: session.clone(),
        focused: false,
    }))
    .await
    .unwrap();
    let titles = until_titles(&recorder, 2).await;
    assert_eq!(titles[1], "SHOP-2 asks a question");

    // Turned off in Settings: respected at once.
    clock.advance(QUIET);
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
/// subscribed client to open its target.
#[tokio::test(flavor = "multi_thread")]
async fn a_click_opens_the_ticket_in_the_connected_app() {
    let (w, recorder, _clock) = world().await;
    let (t, agent, _) = working(&w, "Split user full name").await;
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
            if let Ok(Event::OpenFromNotification { target }) = events.recv().await {
                return target;
            }
        }
    })
    .await
    .expect("the app was told to open the ticket");
    assert_eq!(opened, Target::Ticket { key: t.key.clone() });
    w.stop().await;
}

/// T-0371 (d0090): while an app window is focused nothing is posted, and those items never notify later; a closed
/// connection stops counting; `otaq attach` (a terminal's focus only) doesn't count.
#[tokio::test(flavor = "multi_thread")]
async fn nothing_is_posted_while_an_app_window_is_focused() {
    let (w, recorder, clock) = world().await;
    let (one, first, _) = working(&w, "Add a search box").await;
    let (two, second, _) = working(&w, "Fix the date format").await;
    settle().await;

    let app = Client::connect(ClientConfig::new(&w.data)).await.unwrap();
    app.request(Request::AppFocus(AppFocusParams { focused: true }))
        .await
        .unwrap();
    assert!(w.inbox.looking());
    w.signals
        .ask_human(&first, &one.id, "Fuzzy search?")
        .await
        .unwrap();
    w.inbox.refresh().await.unwrap();
    settle().await;
    assert!(recorder.titles().is_empty(), "looking at Otaq");

    // Switching away doesn't bring it up, not even after the quiet period.
    app.request(Request::AppFocus(AppFocusParams { focused: false }))
        .await
        .unwrap();
    assert!(!w.inbox.looking());
    clock.advance(QUIET);
    w.inbox.refresh().await.unwrap();
    settle().await;
    assert!(recorder.titles().is_empty(), "never later");

    // Focused again, then the window's connection closes: it doesn't count any more.
    app.request(Request::AppFocus(AppFocusParams { focused: true }))
        .await
        .unwrap();
    drop(app);
    tokio::time::timeout(Duration::from_secs(5), async {
        while w.inbox.looking() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("a closed connection stops looking");
    w.signals
        .ask_human(&second, &two.id, "Which date format?")
        .await
        .unwrap();
    assert_eq!(until_titles(&recorder, 1).await, ["SHOP-2 asks a question"]);
    w.stop().await;
}

/// T-0371 (d0090): the first pop-up goes out at once; the next item is held and goes out by itself when the quiet
/// period ends (the inbox wakes for it); one held while the service stops is never posted.
#[tokio::test(flavor = "multi_thread")]
async fn the_quiet_period_holds_and_the_service_posts_at_its_end() {
    let (w, recorder, clock) = world().await;
    let (one, first, _) = working(&w, "Add a search box").await;
    let (two, second, _) = working(&w, "Fix the date format").await;
    let (three, third, _) = working(&w, "Sort notes by date").await;
    settle().await;

    w.signals
        .ask_human(&first, &one.id, "Fuzzy search?")
        .await
        .unwrap();
    assert_eq!(until_titles(&recorder, 1).await, ["SHOP-1 asks a question"]);
    clock.advance(Duration::from_secs(30));
    w.signals
        .ask_human(&second, &two.id, "Which date format?")
        .await
        .unwrap();
    w.inbox.refresh().await.unwrap();
    settle().await;
    assert_eq!(recorder.titles().len(), 1, "held");

    // The period ends: the service posts the held item without any other change.
    clock.advance(QUIET - Duration::from_secs(30));
    assert_eq!(
        until_titles(&recorder, 2).await,
        ["SHOP-1 asks a question", "SHOP-2 asks a question"]
    );

    // Held in the new period, then the service stops: nothing is posted afterwards.
    w.signals
        .ask_human(&third, &three.id, "Newest first?")
        .await
        .unwrap();
    w.inbox.refresh().await.unwrap();
    settle().await;
    assert_eq!(recorder.titles().len(), 2, "held");
    w.stop().await;
    clock.advance(QUIET);
    settle().await;
    assert_eq!(recorder.titles().len(), 2, "no timer posts after the stop");
}
