//! `keymap.*`: the §21.3 keyboard map for the page's key dispatcher (T-0215, d0058). The page asks once at start; the
//! map is fixed for the life of the app, so no event refetches it.

use otaq_core::naming;
use otaq_view::keymap::{self, Binding, Modifiers, Platform, TerminalBinding};
use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(_app: &App, method: &str, _params: Value) -> Result<Value, RequestError> {
    match method {
        "keymap.map" => reply(&Keymap::new(Platform::current(), key_log())),
        _ => Err(unknown(method)),
    }
}

/// The map on this platform: every shortcut with its action, and which modifiers take a key from a focused terminal
/// to the app ([`keymap::route`]'s rule, as data the page applies to each key).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct Keymap {
    pub platform: Platform,
    pub bindings: Vec<Binding>,
    pub terminal_app_modifiers: Vec<Modifiers>,
    /// A focused terminal's own keys (copy, paste, font size), which the terminal handles itself (T-0216).
    pub terminal_keys: Vec<TerminalBinding>,
    /// Whether the page logs every key it gets and where it went, for checking the map by hand. Set by starting the
    /// app with `OTAQ_KEY_LOG=1`.
    pub log: bool,
}

impl Keymap {
    pub fn new(platform: Platform, log: bool) -> Self {
        Self {
            platform,
            bindings: keymap::bindings(platform),
            terminal_app_modifiers: keymap::terminal_app_modifiers(platform).to_vec(),
            terminal_keys: keymap::terminal_bindings(platform),
            log,
        }
    }
}

fn key_log() -> bool {
    std::env::var(naming::env_var("KEY_LOG")).is_ok_and(|v| !v.is_empty() && v != "0")
}

#[cfg(test)]
mod tests {
    use otaq_view::keymap::KeyAction;
    use serde_json::json;

    use super::*;
    use crate::api::request;
    use crate::service::testing::Harness;

    #[tokio::test(flavor = "multi_thread")]
    async fn the_table_answers_keymap_map_for_this_platform() {
        let h = Harness::new(None);
        let map = request(&h.app, "keymap.map", Value::Null).await.unwrap();
        assert_eq!(
            map,
            serde_json::to_value(Keymap::new(Platform::current(), key_log())).unwrap()
        );
        assert!(request(&h.app, "keymap.nothing", Value::Null).await.is_err());
        h.service.stop_all().await;
    }

    #[test]
    fn the_page_gets_the_platform_bindings_and_the_terminal_rule() {
        let map = serde_json::to_value(Keymap::new(Platform::WinLinux, false)).unwrap();
        assert_eq!(map["platform"], "win_linux");
        assert_eq!(map["log"], false);
        assert_eq!(
            map["terminal_app_modifiers"][0],
            json!({ "control": true, "alt": false, "shift": true, "platform": false, "function": false })
        );
        let search = map["bindings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["in_terminal"] == true && b["action"] == json!({ "action": "search" }))
            .unwrap();
        assert_eq!(search["text"], "Ctrl+Shift+K");
        assert_eq!(search["keystroke"]["key"], "k");
        assert_eq!(search["keystroke"]["code"], "KeyK");
        assert_eq!(map["terminal_keys"][0]["key"], "copy");
        assert_eq!(map["terminal_keys"][0]["keystroke"]["code"], "KeyC");

        let mac = Keymap::new(Platform::Mac, true);
        assert!(mac.log);
        assert_eq!(
            mac.bindings
                .iter()
                .find(|b| b.action == KeyAction::Project(3))
                .map(|b| b.text.as_str()),
            Some("⌘3")
        );
    }
}
