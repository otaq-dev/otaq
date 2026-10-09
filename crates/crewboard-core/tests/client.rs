//! The shared client: start-if-missing, reconnect with state changes, fail-fast requests while disconnected,
//! and resubscription. The service runs in-process on a temporary data folder; the "launcher" binds it.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crewboard_core::client::{Client, ClientConfig, ClientError, ConnectionState, Launcher};
use crewboard_core::data_dir::DataDir;
use crewboard_core::protocol::{Event, Reply, Request};
use crewboard_core::service::{EventBus, Service, ServiceConfig, ServiceError, Stopper};

struct Running {
    bus: EventBus,
    stopper: Stopper,
    task: tokio::task::JoinHandle<()>,
}

impl Running {
    async fn stop(self) {
        self.stopper.stop();
        self.task.await.unwrap();
    }
}

async fn bind(data: &DataDir) -> Result<Running, ServiceError> {
    let service = Service::bind(ServiceConfig::new(data.clone())).await?;
    let (bus, stopper) = (service.events(), service.stopper());
    let task = tokio::spawn(service.serve());
    Ok(Running { bus, stopper, task })
}

/// Starts an in-process service, like `crewboard daemon start` would; "already running" counts as success.
struct InProcess {
    data: DataDir,
    launches: AtomicUsize,
    started: Mutex<Vec<Running>>,
}

impl Launcher for InProcess {
    fn launch(&self) -> Pin<Box<dyn Future<Output = io::Result<()>> + Send + '_>> {
        Box::pin(async move {
            self.launches.fetch_add(1, Ordering::SeqCst);
            match bind(&self.data).await {
                Ok(running) => self.started.lock().unwrap().push(running),
                Err(ServiceError::AlreadyRunning { .. }) => {}
                Err(e) => return Err(io::Error::other(e.to_string())),
            }
            Ok(())
        })
    }
}

fn setup() -> (tempfile::TempDir, DataDir, ClientConfig) {
    let tmp = tempfile::tempdir().unwrap();
    let data = DataDir::new(tmp.path().join("home"));
    let mut config = ClientConfig::new(&data);
    config.backoff_min = Duration::from_millis(10);
    config.backoff_max = Duration::from_millis(50);
    (tmp, data, config)
}

async fn wait_for_state(client: &Client, want: impl Fn(&ConnectionState) -> bool) {
    let mut state = client.state();
    tokio::time::timeout(Duration::from_secs(5), state.wait_for(|s| want(s)))
        .await
        .expect("state change in time")
        .unwrap();
}

#[tokio::test]
async fn starts_the_service_if_missing_and_never_a_second_one() {
    let (_tmp, data, config) = setup();
    let launcher = InProcess {
        data: data.clone(),
        launches: AtomicUsize::new(0),
        started: Mutex::new(Vec::new()),
    };
    let client = Client::connect_or_start(config.clone(), &launcher)
        .await
        .unwrap();
    assert!(client.is_connected());
    assert_eq!(launcher.launches.load(Ordering::SeqCst), 1);
    assert_eq!(client.request(Request::Ping).await.unwrap(), Reply::Pong);

    // With the service up, connecting again doesn't launch anything.
    let second = Client::connect_or_start(config.clone(), &launcher)
        .await
        .unwrap();
    assert_eq!(launcher.launches.load(Ordering::SeqCst), 1);
    drop(second);

    // Even a launch while one runs doesn't start a second service: the lock refuses it.
    launcher.launch().await.unwrap();
    assert_eq!(launcher.started.lock().unwrap().len(), 1);

    drop(client);
    let running = launcher.started.lock().unwrap().pop().unwrap();
    running.stop().await;
}

#[tokio::test]
async fn connect_without_a_service_fails() {
    let (_tmp, _data, config) = setup();
    assert!(matches!(
        Client::connect(config).await,
        Err(ClientError::NotRunning(_))
    ));
}

#[tokio::test]
async fn reconnects_after_the_service_restarts() {
    let (_tmp, data, config) = setup();
    let first = bind(&data).await.unwrap();
    let client = Client::connect(config).await.unwrap();
    let ConnectionState::Connected(info) = client.state().borrow().clone() else {
        panic!()
    };
    assert_eq!(info.status.pid, std::process::id());

    first.stop().await;
    wait_for_state(&client, |s| {
        matches!(s, ConnectionState::Reconnecting { .. })
    })
    .await;

    // While disconnected, requests fail at once.
    let started = Instant::now();
    assert!(matches!(
        client.request(Request::Ping).await,
        Err(ClientError::Disconnected)
    ));
    assert!(started.elapsed() < Duration::from_millis(100));

    // Attempts keep counting while the service is away.
    wait_for_state(
        &client,
        |s| matches!(s, ConnectionState::Reconnecting { attempt } if *attempt >= 3),
    )
    .await;

    let second = bind(&data).await.unwrap();
    wait_for_state(&client, |s| matches!(s, ConnectionState::Connected(_))).await;
    assert_eq!(client.request(Request::Ping).await.unwrap(), Reply::Pong);

    drop(client);
    second.stop().await;
}

#[tokio::test]
async fn subscriptions_survive_a_reconnect() {
    let (_tmp, data, config) = setup();
    let first = bind(&data).await.unwrap();
    let client = Client::connect(config).await.unwrap();
    let mut events = client.subscribe().await.unwrap();
    assert_eq!(first.bus.subscriber_count(), 1);
    first.bus.publish(Event::Test { seq: 1 });
    assert_eq!(recv(&mut events).await, Event::Test { seq: 1 });

    first.stop().await;
    wait_for_state(&client, |s| {
        matches!(s, ConnectionState::Reconnecting { .. })
    })
    .await;
    let second = bind(&data).await.unwrap();
    wait_for_state(&client, |s| matches!(s, ConnectionState::Connected(_))).await;

    // The client subscribed again on its own.
    assert_eq!(second.bus.subscriber_count(), 1);
    second.bus.publish(Event::Test { seq: 2 });
    assert_eq!(recv(&mut events).await, Event::Test { seq: 2 });

    drop(client);
    second.stop().await;
}

async fn recv(events: &mut tokio::sync::broadcast::Receiver<Event>) -> Event {
    tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("event in time")
        .unwrap()
}

#[tokio::test]
async fn dropping_the_client_leaves_the_service_running() {
    let (_tmp, data, config) = setup();
    let running = bind(&data).await.unwrap();
    let client = Client::connect(config.clone()).await.unwrap();
    drop(client);
    let again = Client::connect(config).await.unwrap();
    assert_eq!(again.request(Request::Ping).await.unwrap(), Reply::Pong);
    drop(again);
    running.stop().await;
}
