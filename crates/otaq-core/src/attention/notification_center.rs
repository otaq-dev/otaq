//! macOS's notification API (`UNUserNotificationCenter`) under `Otaq.app`'s bundle ID (d0090): the permission state,
//! the question, posting, and clicks. Every call takes an [`InBundle`], which only [`in_bundle`] makes: called from a
//! process outside a bundle the API raises an Objective-C exception that kills the process (tests, `cargo run`).
//! `otaq notify-check` (T-0370) uses all of it; the native notifier (T-0371) reuses it.

use std::ptr::NonNull;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use block2::{DynBlock, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_foundation::{NSBundle, NSDate, NSDictionary, NSError, NSRunLoop, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent, UNNotification,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSettings, UNNotificationSound, UNUserNotificationCenter,
    UNUserNotificationCenterDelegate,
};

use super::bundle::{self, Bundle};

/// Proof that this process runs from `Otaq.app`: its real path is in the bundle ([`bundle::current`]) and macOS's
/// main bundle has the same ID.
#[derive(Debug, Clone)]
pub struct InBundle {
    bundle: Bundle,
}

impl InBundle {
    pub fn bundle(&self) -> &Bundle {
        &self.bundle
    }
}

/// [`InBundle`] when this process runs from `Otaq.app` and macOS agrees.
pub fn in_bundle() -> Option<InBundle> {
    let bundle = bundle::current()?;
    (main_bundle_id().as_deref() == Some(bundle.id.as_str())).then_some(InBundle { bundle })
}

/// `NSBundle.mainBundle.bundleIdentifier`: the ID macOS gives this process, `None` outside a bundle. Safe anywhere.
pub fn main_bundle_id() -> Option<String> {
    NSBundle::mainBundle()
        .bundleIdentifier()
        .map(|id| id.to_string())
}

/// What macOS allows the bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    /// macOS hasn't asked the user yet.
    NotAsked,
    /// Allowed (also macOS's provisional and ephemeral grants).
    Allowed,
    /// The user said no, or turned Otaq off in System Settings.
    TurnedOff,
}

impl Permission {
    fn from_status(status: UNAuthorizationStatus) -> Self {
        match status {
            UNAuthorizationStatus::NotDetermined => Self::NotAsked,
            UNAuthorizationStatus::Denied => Self::TurnedOff,
            _ => Self::Allowed,
        }
    }
}

/// How long a reply from macOS may take before a call gives up (not the user's answer to the question).
const REPLY_TIMEOUT: Duration = Duration::from_secs(10);

fn center() -> Retained<UNUserNotificationCenter> {
    UNUserNotificationCenter::currentNotificationCenter()
}

fn error_text(error: *mut NSError) -> Option<String> {
    // SAFETY: macOS passes either null or a valid NSError for the duration of the completion handler.
    unsafe { error.as_ref() }.map(|e| e.localizedDescription().to_string())
}

/// The bundle's permission state; `None` when macOS doesn't answer in time.
pub fn permission(_: &InBundle) -> Option<Permission> {
    let (tx, rx) = mpsc::channel();
    let handler = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
        // SAFETY: macOS passes a valid settings object for the duration of the completion handler.
        let status = unsafe { settings.as_ref() }.authorizationStatus();
        let _ = tx.send(Permission::from_status(status));
    });
    center().getNotificationSettingsWithCompletionHandler(&handler);
    rx.recv_timeout(REPLY_TIMEOUT).ok()
}

/// Asks macOS for permission (alerts and sound). macOS shows its question only while the state is
/// [`Permission::NotAsked`]; this waits up to `wait` for the user's answer and returns whether it was granted.
pub fn request_permission(_: &InBundle, wait: Duration) -> Result<bool, String> {
    let (tx, rx) = mpsc::channel();
    let handler = RcBlock::new(move |granted: Bool, error: *mut NSError| {
        let _ = tx.send(match error_text(error) {
            Some(text) => Err(text),
            None => Ok(granted.as_bool()),
        });
    });
    center().requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &handler,
    );
    rx.recv_timeout(wait)
        .map_err(|_| "macOS didn't answer in time".to_owned())?
}

/// Posts one notification now, with `id` as its identifier (a click reports it back).
pub fn post(in_bundle: &InBundle, id: &str, title: &str, body: &str) -> Result<(), String> {
    post_content(
        in_bundle,
        &Content {
            id,
            title,
            body,
            ..Content::default()
        },
    )
}

/// A notification as macOS takes it (d0090): title, subtitle, body, the thread it is grouped in, the default sound or
/// none, and the target a click opens (in `userInfo`, under [`USER_INFO_TARGET`]).
#[derive(Debug, Default)]
pub struct Content<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub subtitle: &'a str,
    pub body: &'a str,
    pub thread: Option<&'a str>,
    pub sound: bool,
    pub target: Option<&'a str>,
}

/// The `userInfo` key of a notification's target.
pub const USER_INFO_TARGET: &str = "target";

/// Posts `content` now; waits up to [`REPLY_TIMEOUT`] for macOS to take it.
pub fn post_content(_: &InBundle, content: &Content<'_>) -> Result<(), String> {
    let notification = UNMutableNotificationContent::new();
    notification.setTitle(&NSString::from_str(content.title));
    if !content.subtitle.is_empty() {
        notification.setSubtitle(&NSString::from_str(content.subtitle));
    }
    notification.setBody(&NSString::from_str(content.body));
    if let Some(thread) = content.thread {
        notification.setThreadIdentifier(&NSString::from_str(thread));
    }
    if content.sound {
        notification.setSound(Some(&UNNotificationSound::defaultSound()));
    }
    if let Some(target) = content.target {
        let key = NSString::from_str(USER_INFO_TARGET);
        let value = NSString::from_str(target);
        let info: Retained<NSDictionary<NSString, AnyObject>> =
            NSDictionary::from_slices(&[&*key], &[value.as_ref()]);
        // SAFETY: the keys and values are NSStrings, so the dictionary is one of objects, and it holds only
        // property-list values, as `userInfo` requires.
        unsafe {
            let info: Retained<NSDictionary> = Retained::cast_unchecked(info);
            notification.setUserInfo(&info);
        }
    }
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(content.id),
        &notification,
        None,
    );
    let (tx, rx) = mpsc::channel();
    let handler = RcBlock::new(move |error: *mut NSError| {
        let _ = tx.send(error_text(error));
    });
    center().addNotificationRequest_withCompletionHandler(&request, Some(&handler));
    match rx.recv_timeout(REPLY_TIMEOUT) {
        Ok(None) => Ok(()),
        Ok(Some(error)) => Err(error),
        Err(_) => Err("macOS didn't answer in time".to_owned()),
    }
}

/// A click on one of the bundle's notifications: its identifier and the target it carries, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clicked {
    pub id: String,
    pub target: Option<String>,
}

struct DelegateIvars {
    on_click: Box<dyn Fn(Clicked)>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and the class doesn't implement Drop.
    #[unsafe(super(NSObject))]
    #[ivars = DelegateIvars]
    struct OtaqNotificationDelegate;

    unsafe impl NSObjectProtocol for OtaqNotificationDelegate {}

    unsafe impl UNUserNotificationCenterDelegate for OtaqNotificationDelegate {
        /// A click on one of this bundle's notifications, delivered to this process.
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion: &DynBlock<dyn Fn()>,
        ) {
            let request = response.notification().request();
            let key = NSString::from_str(USER_INFO_TARGET);
            let target = request
                .content()
                .userInfo()
                .objectForKey(&key)
                .and_then(|value| value.downcast_ref::<NSString>().map(ToString::to_string));
            (self.ivars().on_click)(Clicked {
                id: request.identifier().to_string(),
                target,
            });
            completion.call(());
        }

        /// A notification while this process counts as in front: shown as usual.
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            completion
                .call((UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::List,));
        }
    }
);

impl OtaqNotificationDelegate {
    fn new(on_click: Box<dyn Fn(Clicked)>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateIvars { on_click });
        // SAFETY: NSObject's init on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}

/// This process as the notification center's delegate, so clicks on the bundle's notifications that macOS delivers
/// to it arrive here. macOS keeps only a weak reference: keep the listener alive while waiting.
pub struct ClickListener {
    delegate: Retained<OtaqNotificationDelegate>,
    clicks: mpsc::Receiver<String>,
}

impl ClickListener {
    pub fn new(_: &InBundle) -> Self {
        let (tx, clicks) = mpsc::channel();
        let delegate = OtaqNotificationDelegate::new(Box::new(move |clicked: Clicked| {
            let _ = tx.send(clicked.id);
        }));
        center().setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        Self { delegate, clicks }
    }

    /// The identifier of the next clicked notification, waiting up to `timeout` while running this thread's run
    /// loop (macOS may deliver the click through the main queue). Call it on the main thread.
    pub fn wait(&self, timeout: Duration) -> Option<String> {
        let deadline = Instant::now() + timeout;
        let run_loop = NSRunLoop::currentRunLoop();
        while Instant::now() < deadline {
            if let Ok(id) = self.clicks.try_recv() {
                return Some(id);
            }
            run_loop.runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.2));
            // A run loop without sources returns at once: don't spin.
            std::thread::sleep(Duration::from_millis(50));
        }
        self.clicks.try_recv().ok()
    }
}

impl Drop for ClickListener {
    fn drop(&mut self) {
        let center = center();
        let ours = center.delegate().is_some_and(|current| {
            std::ptr::eq(
                (&*current as *const ProtocolObject<dyn UNUserNotificationCenterDelegate>)
                    .cast::<()>(),
                (&*self.delegate as *const OtaqNotificationDelegate).cast::<()>(),
            )
        });
        if ours {
            center.setDelegate(None);
        }
    }
}

/// Makes this process the notification center's delegate for its whole life (the service, d0090): every click macOS
/// delivers to it goes to `on_click`, also for notifications an earlier run posted. macOS keeps only a weak reference,
/// so the delegate is never released.
pub fn on_clicks(_: &InBundle, on_click: impl Fn(Clicked) + 'static) {
    let delegate = OtaqNotificationDelegate::new(Box::new(on_click));
    center().setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    std::mem::forget(delegate);
}

/// Runs `work` on a thread of its own while this (the main) thread runs its run loop, until `work` ends; returns its
/// result. macOS may deliver a notification click through the main queue, which only a running main run loop serves:
/// the service runs this way inside the bundle.
pub fn with_main_run_loop<R: Send + 'static>(work: impl FnOnce() -> R + Send + 'static) -> R {
    let worker = std::thread::Builder::new()
        .name("service".into())
        .spawn(work)
        .expect("a thread for the service");
    let run_loop = NSRunLoop::currentRunLoop();
    while !worker.is_finished() {
        let started = Instant::now();
        run_loop.runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.5));
        // A run loop without sources returns at once: don't spin.
        if started.elapsed() < Duration::from_millis(10) {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    match worker.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
