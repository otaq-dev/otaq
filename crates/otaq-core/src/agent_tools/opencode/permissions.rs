//! OpenCode's permission warning (SET-02, d0011): new agents stop on permission prompts when the user's own config
//! asks before commands or edits. Read-only; Otaq never changes it.
//!
//! OpenCode 2.x keeps its rules in `permissions`, a list of `{action, resource, effect}` (`effect` `allow`, `ask`
//! or `deny`), in `config.json`, `opencode.json` and `opencode.jsonc` of its config folder
//! (`$XDG_CONFIG_HOME/opencode`, else `~/.config/opencode`), read in that order. For each use it takes the last rule
//! whose action matches (`*` matches any text), and its default agent allows everything, so with no rules it never
//! asks about commands (hands-on report, row 4). Commands are the `shell` action and file changes `edit` (read in the
//! OpenCode 2.0.17 binary). The warning shows when a rule asks for either and no later rule for every use of it
//! (`*`) allows or denies instead.

use std::path::Path;

use serde_json::Value;

use super::{DISPLAY_NAME, OpenCodeContext};

/// The config files of OpenCode's config folder, in the order it reads them.
const FILES: [&str; 3] = ["config.json", "opencode.json", "opencode.jsonc"];

/// The actions whose prompts stop an agent.
const PROMPTING: [&str; 2] = ["shell", "edit"];

pub(super) fn warning(context: &OpenCodeContext) -> Option<String> {
    let dir = context.config_dir.as_deref()?;
    asks(&rules(dir)).then(|| format!("Your {DISPLAY_NAME} settings ask for permission prompts"))
}

/// One rule as OpenCode writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Rule {
    action: String,
    resource: String,
    effect: String,
}

/// Every rule in `dir`'s config files, in order. A file that is missing or can't be read adds nothing.
pub(super) fn rules(dir: &Path) -> Vec<Rule> {
    FILES
        .iter()
        .filter_map(|name| std::fs::read_to_string(dir.join(name)).ok())
        .filter_map(|text| serde_json::from_str::<Value>(&without_comments(&text)).ok())
        .flat_map(|config| {
            config
                .get("permissions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|rule| {
            let text = |field: &str| rule.get(field).and_then(Value::as_str).map(str::to_owned);
            Some(Rule {
                action: text("action")?,
                resource: text("resource").unwrap_or_else(|| "*".to_owned()),
                effect: text("effect")?,
            })
        })
        .collect()
}

/// Whether some use of `shell` or `edit` asks: a matching `ask` rule that no later rule for every use of the action
/// replaces.
pub(super) fn asks(rules: &[Rule]) -> bool {
    PROMPTING.iter().any(|action| {
        let matching: Vec<&Rule> = rules
            .iter()
            .filter(|r| matches(&r.action, action))
            .collect();
        matching.iter().enumerate().any(|(i, rule)| {
            rule.effect == "ask"
                && !matching[i + 1..]
                    .iter()
                    .any(|later| later.resource == "*" && later.effect != "ask")
        })
    })
}

/// OpenCode's wildcard: `*` matches any text, everything else itself.
fn matches(pattern: &str, text: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or("");
    let Some(mut rest) = text.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        return rest.is_empty();
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.ends_with(last)
}

/// JSON with comments (`opencode.jsonc`): `//` and `/* */` comments and trailing commas removed, strings kept.
fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next()),
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let mut last = ' ';
                for c in chars.by_ref() {
                    if last == '*' && c == '/' {
                        break;
                    }
                    last = c;
                }
            }
            (',', _) => {
                let after: String = chars.clone().collect();
                let next = after.trim_start();
                if !(next.starts_with(']') || next.starts_with('}')) {
                    out.push(c);
                }
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_tools::test_support::context;

    fn rule(action: &str, resource: &str, effect: &str) -> Rule {
        Rule {
            action: action.into(),
            resource: resource.into(),
            effect: effect.into(),
        }
    }

    fn adapter_in(config: &Path) -> OpenCodeContext {
        let mut context = OpenCodeContext::new(context(config, None));
        context.config_dir = Some(config.to_path_buf());
        context
    }

    #[test]
    fn rules_that_ask() {
        assert!(!asks(&[]), "no rules: OpenCode doesn't ask");
        assert!(asks(&[rule("shell", "*", "ask")]));
        assert!(asks(&[rule("edit", "*", "ask")]));
        assert!(asks(&[rule("*", "*", "ask")]));
        assert!(
            asks(&[rule("shell", "git push *", "ask")]),
            "some commands ask"
        );
        assert!(!asks(&[
            rule("read", "*.env", "ask"),
            rule("webfetch", "*", "ask")
        ]));
        assert!(!asks(&[
            rule("shell", "*", "ask"),
            rule("shell", "*", "allow")
        ]));
        assert!(
            asks(&[rule("shell", "*", "allow"), rule("shell", "*", "ask")]),
            "the last one wins"
        );
        assert!(
            asks(&[rule("shell", "*", "ask"), rule("shell", "ls *", "allow")]),
            "only some commands allowed"
        );
        assert!(!asks(&[rule("*", "*", "ask"), rule("*", "*", "deny")]));
        assert!(matches("sh*", "shell"));
        assert!(!matches("shell", "shells"));
        assert!(matches("*", "edit"));
    }

    /// The user's files in a temporary config folder: comments and trailing commas in `opencode.jsonc`, a later
    /// file's rules after an earlier one's, a file that can't be parsed adding nothing.
    #[test]
    fn the_users_config_files() {
        let dir = tempfile::tempdir().unwrap();
        let context = adapter_in(dir.path());
        assert_eq!(context.config_dir.as_deref(), Some(dir.path()));
        assert_eq!(warning(&context), None, "no config");

        std::fs::write(
            dir.path().join("opencode.json"),
            r#"{"$schema": "https://opencode.ai/config.json", "model": "opencode/big-pickle",
                "permissions": [{"action": "shell", "resource": "*", "effect": "ask"}]}"#,
        )
        .unwrap();
        assert_eq!(
            warning(&context).as_deref(),
            Some("Your OpenCode settings ask for permission prompts")
        );

        std::fs::write(
            dir.path().join("opencode.jsonc"),
            "{\n  // Let it run commands.\n  \"permissions\": [\n    /* every command */\n    {\"action\": \"shell\", \"resource\": \"*\", \"effect\": \"allow\"},\n  ],\n  \"username\": \"a // b\",\n}\n",
        )
        .unwrap();
        assert_eq!(rules(dir.path()).len(), 2);
        assert_eq!(warning(&context), None, "the later file allows");

        std::fs::write(dir.path().join("config.json"), "{ not json").unwrap();
        assert_eq!(rules(dir.path()).len(), 2);
    }

    #[test]
    fn no_config_folder_no_warning() {
        let bin = tempfile::tempdir().unwrap();
        let context = OpenCodeContext::new(context(bin.path(), None));
        assert_eq!(context.config_dir, None);
        assert_eq!(warning(&context), None);
    }
}
