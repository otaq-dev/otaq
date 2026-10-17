//! The model picker of the Add and Edit agent dialogs (T-0340; AGT-02, d0086; *15 → Add agent · model picker open*
//! `k8WyHN` / `L1mzD`), built from Otaq's copy of the tool's own list (T-0335's `ModelList`). Only what the list has
//! can be picked and nothing can be typed (d0086): "Tool default" first, then **ALWAYS THE NEWEST** (the tool's own
//! aliases, each with what it resolves to today), then **EXACT MODELS**. A saved model the copy no longer has is
//! shown as saved (M23 adds "no longer listed").

use otaq_core::agent_tools::{ModelKind, ModelList};
use otaq_core::naming::PRODUCT_NAME;
use serde::Serialize;
use ts_rs::TS;

use super::named_agents::{TOOL_DEFAULT_LABEL, is_tool_default, model_label};

/// One choice. `value` is what the dialog sends as the agent's model: `None` for the tool default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ModelPickerItem")]
pub struct Item {
    pub value: Option<String>,
    /// "Tool default", "Opus · always the newest", `claude-opus-5-5`.
    pub label: String,
    /// The muted part on the right: "Otaq passes no model", "now claude-opus-5-5".
    pub detail: Option<String>,
    /// An exact model id is drawn in the mono font.
    pub mono: bool,
    pub selected: bool,
}

/// A group under its small heading ("ALWAYS THE NEWEST"); the tool default's group has none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ModelPickerGroup")]
pub struct Group {
    pub heading: Option<String>,
    pub items: Vec<Item>,
    /// The muted line when the group has no items: "Claude Code's list has only its aliases".
    pub empty: Option<String>,
}

/// What the closed field shows: the picked model and what it runs today.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ModelPickerValue")]
pub struct Value {
    pub label: String,
    pub detail: Option<String>,
    pub mono: bool,
}

/// The whole picker: the field's value, the open list's groups in order, and the hint under the field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ModelPickerView")]
pub struct Picker {
    pub value: Value,
    pub groups: Vec<Group>,
    pub hint: String,
}

pub const NEWEST_HEADING: &str = "ALWAYS THE NEWEST";
pub const EXACT_HEADING: &str = "EXACT MODELS";

/// "now claude-opus-5-5".
fn now(resolves_to: Option<&str>) -> Option<String> {
    resolves_to.map(|id| format!("now {id}"))
}

/// What passing nothing means, beside "Tool default".
pub fn passes_nothing() -> String {
    format!("{PRODUCT_NAME} passes no model")
}

/// The picker for `tool` (its display name) with `picked` (the dialog's current model, `None` = tool default).
/// `refused`: the entry is marked for this very model (the hint then says the account can't use it, `WFHlh`).
pub fn picker(tool: &str, list: Option<&ModelList>, picked: Option<&str>, refused: bool) -> Picker {
    let entries = list.map(|l| l.entries.as_slice()).unwrap_or_default();
    let picked_default = is_tool_default(picked, list);
    let selected = |id: &str| !picked_default && picked == Some(id);
    let item = |e: &otaq_core::agent_tools::models::ModelEntry| Item {
        value: Some(e.id.clone()),
        label: model_label(Some(&e.id), list),
        detail: match e.kind {
            ModelKind::Follow => now(e.resolves_to.as_deref()),
            _ => None,
        },
        mono: e.kind == ModelKind::Exact,
        selected: selected(&e.id),
    };
    let of = |kind| entries.iter().filter(move |e| e.kind == kind);
    let default = Group {
        heading: None,
        items: vec![Item {
            value: None,
            label: TOOL_DEFAULT_LABEL.to_owned(),
            detail: Some(passes_nothing()),
            mono: false,
            selected: picked_default,
        }],
        empty: None,
    };
    let newest = Group {
        heading: Some(NEWEST_HEADING.to_owned()),
        items: of(ModelKind::Follow).map(item).collect(),
        empty: None,
    };
    let exact: Vec<Item> = of(ModelKind::Exact).map(item).collect();
    let exact = Group {
        heading: Some(EXACT_HEADING.to_owned()),
        empty: exact
            .is_empty()
            .then(|| format!("{tool}'s list has only its aliases")),
        items: exact,
    };
    let value = if picked_default {
        Value {
            label: TOOL_DEFAULT_LABEL.to_owned(),
            detail: Some(passes_nothing()),
            mono: false,
        }
    } else {
        let id = picked.unwrap_or_default();
        let entry = list.and_then(|l| l.entry(id));
        Value {
            label: model_label(Some(id), list),
            detail: entry
                .filter(|e| e.kind == ModelKind::Follow)
                .and_then(|e| now(e.resolves_to.as_deref())),
            // A saved model the copy no longer has is shown as saved: its id.
            mono: entry.is_none_or(|e| e.kind == ModelKind::Exact),
        }
    };
    let hint = if refused {
        format!("From {tool}'s own list. The list offered it, but this account can't use it.")
    } else {
        format!(
            "From {tool}'s own list. A follow choice is resolved when each session starts, and the session keeps the exact model it ran on."
        )
    };
    Picker {
        value,
        groups: vec![default, newest, exact],
        hint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::named_agents::fixtures;

    /// A group's heading and its items' label and detail.
    type Shape = (Option<String>, Vec<(String, Option<String>)>);

    fn labels(p: &Picker) -> Vec<Shape> {
        p.groups
            .iter()
            .map(|g| {
                (
                    g.heading.clone(),
                    g.items
                        .iter()
                        .map(|i| (i.label.clone(), i.detail.clone()))
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn the_groups_come_in_order_with_what_each_follow_choice_runs_today() {
        let list = fixtures::claude_list();
        let p = picker("Claude Code", Some(&list), Some("fable"), false);
        let s = |t: &str| t.to_owned();
        assert_eq!(
            labels(&p),
            vec![
                (
                    None,
                    vec![(
                        s("Tool default"),
                        Some(format!(
                            "{} passes no model",
                            otaq_core::naming::PRODUCT_NAME
                        ))
                    )]
                ),
                (
                    Some(s("ALWAYS THE NEWEST")),
                    vec![
                        (
                            s("Opus · always the newest"),
                            Some(s("now claude-opus-5-5"))
                        ),
                        (
                            s("Sonnet · always the newest"),
                            Some(s("now claude-sonnet-5-5"))
                        ),
                        (
                            s("Haiku · always the newest"),
                            Some(s("now claude-haiku-5-5"))
                        ),
                        (
                            s("Fable · always the newest"),
                            Some(s("now claude-fable-5-1"))
                        ),
                    ]
                ),
                (Some(s("EXACT MODELS")), vec![]),
            ]
        );
        assert_eq!(
            p.groups[2].empty.as_deref(),
            Some("Claude Code's list has only its aliases")
        );
        assert_eq!(
            p.hint,
            "From Claude Code's own list. A follow choice is resolved when each session starts, and the session keeps the exact model it ran on."
        );
        let picked: Vec<&str> = p
            .groups
            .iter()
            .flat_map(|g| &g.items)
            .filter(|i| i.selected)
            .map(|i| i.label.as_str())
            .collect();
        assert_eq!(picked, ["Fable · always the newest"]);
        assert_eq!(p.value.label, "Fable · always the newest");
        assert_eq!(p.value.detail.as_deref(), Some("now claude-fable-5-1"));
        assert_eq!(p.groups[1].items[0].value.as_deref(), Some("opus"));
    }

    #[test]
    fn the_tool_default_is_picked_for_no_model_and_for_the_tools_own_default() {
        let list = fixtures::claude_list();
        for picked in [None, Some("default")] {
            let p = picker("Claude Code", Some(&list), picked, false);
            assert!(p.groups[0].items[0].selected);
            assert_eq!(
                p.groups[0].items[0].value, None,
                "the tool default passes nothing"
            );
            assert_eq!(p.value.label, "Tool default");
            assert_eq!(
                p.value.detail.as_deref(),
                Some(format!("{} passes no model", otaq_core::naming::PRODUCT_NAME).as_str())
            );
        }
    }

    #[test]
    fn exact_models_list_in_mono_and_a_saved_model_the_list_lost_shows_as_saved() {
        let mut list = fixtures::claude_list();
        list.entries.push(fixtures::entry(
            "claude-opus-5-5",
            "Opus 5.5",
            ModelKind::Exact,
            None,
        ));
        let p = picker("Claude Code", Some(&list), Some("claude-opus-5-5"), false);
        assert_eq!(p.groups[2].empty, None);
        let exact = &p.groups[2].items[0];
        assert_eq!(
            (exact.label.as_str(), exact.mono, exact.selected),
            ("claude-opus-5-5", true, true)
        );

        let p = picker("Claude Code", Some(&list), Some("claude-opus-4-0"), false);
        assert_eq!(p.value.label, "claude-opus-4-0");
        assert!(p.value.mono);
        assert!(p.groups.iter().flat_map(|g| &g.items).all(|i| !i.selected));
    }

    #[test]
    fn a_refused_model_says_the_account_cant_use_it() {
        let list = fixtures::claude_list();
        let p = picker("Claude Code", Some(&list), Some("fable"), true);
        assert_eq!(
            p.hint,
            "From Claude Code's own list. The list offered it, but this account can't use it."
        );
    }
}
