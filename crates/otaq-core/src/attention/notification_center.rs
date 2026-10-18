//! macOS's notification API (`UNUserNotificationCenter`) under `Otaq.app`'s bundle ID (d0090): the permission state,
//! the question, posting, and clicks. Every call takes an [`InBundle`], which only [`in_bundle`] makes: called from a
//! process outside a bundle the API raises an Objective-C exception that kills the process (tests, `cargo run`).
//! `otaq notify-check` (T-0370) uses all of it; the native notifier (T-0371) reuses it.

use std::ptr::NonNull;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use block2::{DynBlock, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_foundation::{NSBundle, NSDate, NSError, NSRunLoop, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent, UNNotification,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSettings, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

use super::bundle::{self, Bundle};

/// Proof that this process runs from `Otaq.app`: its real path is in the bundle ([`bundle::current`]) and macOS's
/// main bundle has the same ID.
#[derive(Debug)]
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
pub fn post(_: &InBundle, id: &str, title: &str, body: &str) -> Result<(), String> {
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(id),
        &content,
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

struct DelegateIvars {
    clicks: mpsc::Sender<String>,
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
            let id = response.notification().request().identifier().to_string();
            let _ = self.ivars().clicks.send(id);
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
    fn new(clicks: mpsc::Sender<String>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateIvars { clicks });
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
        let delegate = OtaqNotificationDelegate::new(tx);
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
