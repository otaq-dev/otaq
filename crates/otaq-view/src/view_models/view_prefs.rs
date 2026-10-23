//! A project's view preferences as the page reads and changes them (SHB-05, BRD-01, BRD-02, BRD-04; T-0460): the
//! service keeps them in `ProjectSettings.view` ([`ViewPrefs`]); the page gets them with `board.view` ([`Prefs`]) and
//! changes one at a time with `board.set_view` ([`Change`], applied by [`apply`] and stored with
//! `project_view.set`). Nothing is kept in the page. The toolbar ([`Toolbar`], *24 · The two views*) is worded here:
//! Board · List, Lanes None · Epic on the Board, Group by on the List, Fold empty columns or Hide empty stages, All
//! tickets and New ticket; each option carries the change it sends.

use otaq_core::board::settings::{self as core, ViewPrefs};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;
use crate::key_labels::KeyLabels;
use crate::keymap::KeyAction;

/// Board or List (BRD-01). `BoardViewKind` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "BoardViewKind")]
pub enum ViewKind {
    Board,
    List,
}

/// The Board's lanes (EPL-01). `BoardLanes` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "BoardLanes")]
pub enum Lanes {
    None,
    Epic,
}

/// The List's grouping (LST-01). `ListGroupBy` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "ListGroupBy")]
pub enum GroupBy {
    Stage,
    Status,
    Epic,
    Agent,
}

/// When dependencies are drawn (DPV-02). `DependenciesMode` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "DependenciesMode")]
pub enum Dependencies {
    OnHover,
    Always,
}

impl From<core::BoardView> for ViewKind {
    fn from(v: core::BoardView) -> Self {
        match v {
            core::BoardView::Board => Self::Board,
            core::BoardView::List => Self::List,
        }
    }
}

impl From<ViewKind> for core::BoardView {
    fn from(v: ViewKind) -> Self {
        match v {
            ViewKind::Board => Self::Board,
            ViewKind::List => Self::List,
        }
    }
}

impl From<core::Lanes> for Lanes {
    fn from(v: core::Lanes) -> Self {
        match v {
            core::Lanes::None => Self::None,
            core::Lanes::Epic => Self::Epic,
        }
    }
}

impl From<Lanes> for core::Lanes {
    fn from(v: Lanes) -> Self {
        match v {
            Lanes::None => Self::None,
            Lanes::Epic => Self::Epic,
        }
    }
}

impl From<core::GroupBy> for GroupBy {
    fn from(v: core::GroupBy) -> Self {
        match v {
            core::GroupBy::Stage => Self::Stage,
            core::GroupBy::Status => Self::Status,
            core::GroupBy::Epic => Self::Epic,
            core::GroupBy::Agent => Self::Agent,
        }
    }
}

impl From<GroupBy> for core::GroupBy {
    fn from(v: GroupBy) -> Self {
        match v {
            GroupBy::Stage => Self::Stage,
            GroupBy::Status => Self::Status,
            GroupBy::Epic => Self::Epic,
            GroupBy::Agent => Self::Agent,
        }
    }
}

impl From<core::Dependencies> for Dependencies {
    fn from(v: core::Dependencies) -> Self {
        match v {
            core::Dependencies::OnHover => Self::OnHover,
            core::Dependencies::Always => Self::Always,
        }
    }
}

impl From<Dependencies> for core::Dependencies {
    fn from(v: Dependencies) -> Self {
        match v {
            Dependencies::OnHover => Self::OnHover,
            Dependencies::Always => Self::Always,
        }
    }
}

/// The project's view preferences as the page reads them (SHB-05). `ViewPrefs` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ViewPrefs")]
pub struct Prefs {
    pub view: ViewKind,
    pub lanes: Lanes,
    pub group_by: GroupBy,
    pub fold_empty_columns: bool,
    pub hide_empty_stages: bool,
    pub dependencies: Dependencies,
    /// Column ids folded from their ⋯ ("done" at first).
    pub folded_columns: Vec<String>,
    /// Epic keys whose lanes are folded.
    pub folded_lanes: Vec<String>,
    /// A human check column's sections folded by their label.
    pub folded_sections: Vec<String>,
    pub dev_unfolded: bool,
}

impl From<&ViewPrefs> for Prefs {
    fn from(p: &ViewPrefs) -> Self {
        Self {
            view: p.view.into(),
            lanes: p.lanes.into(),
            group_by: p.group_by.into(),
            fold_empty_columns: p.fold_empty_columns,
            hide_empty_stages: p.hide_empty_stages,
            dependencies: p.dependencies.into(),
            folded_columns: p.folded_columns.clone(),
            folded_lanes: p.folded_lanes.clone(),
            folded_sections: p.folded_sections.clone(),
            dev_unfolded: p.dev_unfolded,
        }
    }
}

/// One change of the view preferences (`board.set_view`). `ViewChange` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "ViewChange")]
pub enum Change {
    View {
        view: ViewKind,
    },
    Lanes {
        lanes: Lanes,
    },
    GroupBy {
        group_by: GroupBy,
    },
    FoldEmptyColumns {
        on: bool,
    },
    HideEmptyStages {
        on: bool,
    },
    Dependencies {
        dependencies: Dependencies,
    },
    /// A column folded from its ⋯ (`true`) or unfolded by a click on its rail (`false`), by its id.
    FoldColumn {
        column: String,
        folded: bool,
    },
    /// An epic lane folded or unfolded by its head (EPL-01), by the epic's key.
    FoldLane {
        epic: String,
        folded: bool,
    },
    /// A section of a human check column folded or unfolded by its label, by its id.
    FoldSection {
        section: String,
        folded: bool,
    },
    /// An agent column's "4 more · working" fold opened (`true`) or back (`false`).
    DevUnfolded {
        on: bool,
    },
}

/// Puts `change` into `prefs`. Folding twice or unfolding what isn't folded changes nothing.
pub fn apply(prefs: &mut ViewPrefs, change: &Change) {
    fn set(list: &mut Vec<String>, id: &str, on: bool) {
        list.retain(|f| f != id);
        if on {
            list.push(id.to_owned());
        }
    }
    match change {
        Change::View { view } => prefs.view = (*view).into(),
        Change::Lanes { lanes } => prefs.lanes = (*lanes).into(),
        Change::GroupBy { group_by } => prefs.group_by = (*group_by).into(),
        Change::FoldEmptyColumns { on } => prefs.fold_empty_columns = *on,
        Change::HideEmptyStages { on } => prefs.hide_empty_stages = *on,
        Change::Dependencies { dependencies } => prefs.dependencies = (*dependencies).into(),
        Change::FoldColumn { column, folded } => set(&mut prefs.folded_columns, column, *folded),
        Change::FoldLane { epic, folded } => set(&mut prefs.folded_lanes, epic, *folded),
        Change::FoldSection { section, folded } => {
            set(&mut prefs.folded_sections, section, *folded);
        }
        Change::DevUnfolded { on } => prefs.dev_unfolded = *on,
    }
}

/// One choice in the toolbar: its words, whether it is the current one, and the change a click sends.
/// `ToolbarOption` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ToolbarOption")]
pub struct ToolbarOption {
    pub label: String,
    pub icon: Option<IconName>,
    /// The current one (a segment), or on (a switch).
    pub selected: bool,
    pub change: Change,
    /// "List view" over its key ("g l"), on hover.
    pub tooltip: Option<String>,
    pub keys: Option<String>,
}

/// A segmented switch: "Lanes" None · Epic. `ToolbarGroup` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ToolbarGroup")]
pub struct ToolbarGroup {
    /// The words before it ("Lanes", "Group by"); `null` for Board · List.
    pub label: Option<String>,
    pub options: Vec<ToolbarOption>,
}

/// The toolbar at the top of the board, in both views (*24 · The two views*). `BoardToolbar` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "BoardToolbar")]
pub struct Toolbar {
    /// Board · List (BRD-01).
    pub views: ToolbarGroup,
    /// The Board's Lanes None · Epic (EPL-01).
    pub lanes: Option<ToolbarGroup>,
    /// The List's Group by Stage · Status · Epic · Agent (LST-01).
    pub group_by: Option<ToolbarGroup>,
    /// Fold empty columns (Board) or Hide empty stages (List): a switch.
    pub switch: ToolbarOption,
    /// "All tickets": the board's filter (`/`).
    pub all_tickets: String,
    /// "New ticket" (⌘N).
    pub new_ticket: String,
    pub new_ticket_keys: Option<String>,
}

pub const ALL_TICKETS: &str = "All tickets";
pub const NEW_TICKET: &str = "New ticket";

/// The toolbar for `prefs`, with the user's keys in the tooltips.
pub fn toolbar(prefs: &ViewPrefs, keys: &KeyLabels) -> Toolbar {
    let view: ViewKind = prefs.view.into();
    let option =
        |label: &str, icon: Option<IconName>, selected: bool, change: Change| ToolbarOption {
            label: label.to_owned(),
            icon,
            selected,
            change,
            tooltip: None,
            keys: None,
        };
    let view_option =
        |kind: ViewKind, label: &str, icon: IconName, action: KeyAction| ToolbarOption {
            tooltip: Some(format!("{label} view")),
            keys: keys.label(action),
            ..option(label, Some(icon), view == kind, Change::View { view: kind })
        };
    let lanes: Lanes = prefs.lanes.into();
    let group_by: GroupBy = prefs.group_by.into();
    Toolbar {
        views: ToolbarGroup {
            label: None,
            options: vec![
                view_option(
                    ViewKind::Board,
                    "Board",
                    IconName::Columns3,
                    KeyAction::ShowBoard,
                ),
                view_option(ViewKind::List, "List", IconName::List, KeyAction::ShowList),
            ],
        },
        lanes: (view == ViewKind::Board).then(|| ToolbarGroup {
            label: Some("Lanes".to_owned()),
            options: [(Lanes::None, "None"), (Lanes::Epic, "Epic")]
                .into_iter()
                .map(|(l, label)| option(label, None, lanes == l, Change::Lanes { lanes: l }))
                .collect(),
        }),
        group_by: (view == ViewKind::List).then(|| ToolbarGroup {
            label: Some("Group by".to_owned()),
            options: [
                (GroupBy::Stage, "Stage"),
                (GroupBy::Status, "Status"),
                (GroupBy::Epic, "Epic"),
                (GroupBy::Agent, "Agent"),
            ]
            .into_iter()
            .map(|(g, label)| option(label, None, group_by == g, Change::GroupBy { group_by: g }))
            .collect(),
        }),
        switch: match view {
            ViewKind::Board => option(
                "Fold empty columns",
                Some(IconName::FoldHorizontal),
                prefs.fold_empty_columns,
                Change::FoldEmptyColumns {
                    on: !prefs.fold_empty_columns,
                },
            ),
            ViewKind::List => option(
                "Hide empty stages",
                Some(IconName::FoldHorizontal),
                prefs.hide_empty_stages,
                Change::HideEmptyStages {
                    on: !prefs.hide_empty_stages,
                },
            ),
        },
        all_tickets: ALL_TICKETS.to_owned(),
        new_ticket: NEW_TICKET.to_owned(),
        new_ticket_keys: keys.label(KeyAction::NewTicket),
    }
}

/// Until the List arrives (LST-01, T-0461), the List view says so; the toolbar and the strip stay.
pub const LIST_PLACEHOLDER: &str =
    "The List view isn\u{2019}t built yet. Switch to Board to see these tickets.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_fold_and_unfold_once() {
        let mut prefs = ViewPrefs::default();
        let fold = |column: &str, folded| Change::FoldColumn {
            column: column.into(),
            folded,
        };
        apply(&mut prefs, &fold("grill", true));
        apply(&mut prefs, &fold("grill", true));
        assert_eq!(prefs.folded_columns, ["done", "grill"]);
        apply(&mut prefs, &fold("done", false));
        assert_eq!(prefs.folded_columns, ["grill"]);
        apply(
            &mut prefs,
            &Change::View {
                view: ViewKind::List,
            },
        );
        apply(
            &mut prefs,
            &Change::GroupBy {
                group_by: GroupBy::Status,
            },
        );
        apply(&mut prefs, &Change::Lanes { lanes: Lanes::Epic });
        apply(
            &mut prefs,
            &Change::Dependencies {
                dependencies: Dependencies::Always,
            },
        );
        apply(&mut prefs, &Change::FoldEmptyColumns { on: true });
        apply(&mut prefs, &Change::HideEmptyStages { on: true });
        apply(
            &mut prefs,
            &Change::FoldLane {
                epic: "SHOP-40".into(),
                folded: true,
            },
        );
        apply(
            &mut prefs,
            &Change::FoldSection {
                section: "ship".into(),
                folded: true,
            },
        );
        apply(&mut prefs, &Change::DevUnfolded { on: true });
        let page = Prefs::from(&prefs);
        assert_eq!(
            (page.view, page.group_by, page.lanes, page.dependencies),
            (
                ViewKind::List,
                GroupBy::Status,
                Lanes::Epic,
                Dependencies::Always
            )
        );
        assert!(page.fold_empty_columns && page.hide_empty_stages && page.dev_unfolded);
        assert_eq!(page.folded_lanes, ["SHOP-40"]);
        assert_eq!(page.folded_sections, ["ship"]);
        // On the wire: `{"kind": "fold_column", "column": "grill", "folded": true}`.
        assert_eq!(
            serde_json::to_value(fold("grill", true)).unwrap(),
            serde_json::json!({"kind": "fold_column", "column": "grill", "folded": true})
        );
    }

    #[test]
    fn the_toolbar_follows_the_view() {
        let keys = KeyLabels::default();
        let prefs = ViewPrefs::default();
        let board = toolbar(&prefs, &keys);
        let labels = |g: &ToolbarGroup| -> Vec<(String, bool)> {
            g.options
                .iter()
                .map(|o| (o.label.clone(), o.selected))
                .collect()
        };
        assert_eq!(
            labels(&board.views),
            [("Board".to_owned(), true), ("List".to_owned(), false)]
        );
        assert_eq!(board.views.options[1].tooltip.as_deref(), Some("List view"));
        assert_eq!(board.views.options[1].keys.as_deref(), Some("g l"));
        let lanes = board.lanes.as_ref().unwrap();
        assert_eq!(lanes.label.as_deref(), Some("Lanes"));
        assert_eq!(
            labels(lanes),
            [("None".to_owned(), true), ("Epic".to_owned(), false)]
        );
        assert_eq!(board.group_by, None);
        assert_eq!(
            (
                board.switch.label.as_str(),
                board.switch.selected,
                &board.switch.change
            ),
            (
                "Fold empty columns",
                false,
                &Change::FoldEmptyColumns { on: true }
            )
        );
        assert_eq!(
            (board.all_tickets.as_str(), board.new_ticket.as_str()),
            ("All tickets", "New ticket")
        );

        let list = toolbar(
            &ViewPrefs {
                view: core::BoardView::List,
                hide_empty_stages: true,
                ..ViewPrefs::default()
            },
            &keys,
        );
        assert_eq!(list.lanes, None);
        let group = list.group_by.as_ref().unwrap();
        assert_eq!(group.label.as_deref(), Some("Group by"));
        assert_eq!(
            labels(group),
            [
                ("Stage".to_owned(), true),
                ("Status".to_owned(), false),
                ("Epic".to_owned(), false),
                ("Agent".to_owned(), false)
            ]
        );
        assert_eq!(
            (
                list.switch.label.as_str(),
                list.switch.selected,
                &list.switch.change
            ),
            (
                "Hide empty stages",
                true,
                &Change::HideEmptyStages { on: false }
            )
        );
    }
}
