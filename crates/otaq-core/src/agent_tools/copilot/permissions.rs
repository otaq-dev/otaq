//! Copilot's permission warning (SET-02, d0011; T-0353). Read-only; Otaq never changes the user's settings.
//!
//! Copilot asks before every write and command its settings don't allow; reads are approved (`copilot help config`,
//! 1.0.98: `defaultPermissionMode` in `$COPILOT_HOME/settings.json`, `"manual"` by default, `"assisted"` still
//! prompts for what its review doesn't approve, `"allow-all"` approves everything). So the warning shows unless the
//! user's own setting is `"allow-all"`, and also when the file can't be read: prompting is Copilot's default.

use std::path::Path;

use serde_json::Value;

use super::{CopilotContext, DISPLAY_NAME, trust};

/// Copilot's user settings file in its home folder.
const SETTINGS_FILE: &str = "settings.json";

/// The setting, and the one value with which new sessions don't stop on prompts.
const MODE_KEY: &str = "defaultPermissionMode";
const ALLOW_ALL: &str = "allow-all";

pub(super) fn warning(context: &CopilotContext) -> Option<String> {
    let allows_all = context
        .copilot_home
        .as_deref()
        .is_some_and(allows_everything);
    (!allows_all).then(|| format!("Your {DISPLAY_NAME} settings ask for permission prompts"))
}

/// Whether `home`'s settings file sets the permission mode that approves everything.
fn allows_everything(home: &Path) -> bool {
    std::fs::read_to_string(home.join(SETTINGS_FILE))
        .ok()
        .and_then(|text| trust::parse(&text))
        .is_some_and(|(_, settings)| {
            settings.get(MODE_KEY).and_then(Value::as_str) == Some(ALLOW_ALL)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_tools::AgentTool;
    use crate::agent_tools::copilot::Copilot;
    use crate::agent_tools::test_support::context;

    /// Prompting is the default: no file, another mode, a broken file, no home folder. Only `"allow-all"` hides it.
    #[test]
    fn the_warning_shows_unless_everything_is_allowed() {
        let home = tempfile::tempdir().unwrap();
        let adapter = Copilot::with_context(CopilotContext {
            tool: context(&home.path().join("bin"), Some(home.path())),
            copilot_home: Some(home.path().to_path_buf()),
        });
        let warning = Some("Your GitHub Copilot settings ask for permission prompts".to_owned());
        assert_eq!(adapter.permission_warning(), warning);
        let settings = home.path().join(SETTINGS_FILE);
        for text in [
            r#"{"defaultPermissionMode": "manual"}"#,
            r#"{"defaultPermissionMode": "assisted", "model": "auto"}"#,
            r#"{"defaultPermissionMode": "#,
        ] {
            std::fs::write(&settings, text).unwrap();
            assert_eq!(adapter.permission_warning(), warning, "{text}");
        }
        std::fs::write(
            &settings,
            "// a comment line\n{\"model\": \"auto\", \"defaultPermissionMode\": \"allow-all\"}",
        )
        .unwrap();
        assert_eq!(adapter.permission_warning(), None);

        let bin = tempfile::tempdir().unwrap();
        assert_eq!(
            Copilot::new(context(bin.path(), None)).permission_warning(),
            warning
        );
    }
}
