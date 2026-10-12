//! The bundled icon set: Lucide (ISC licence, `assets/icons/LICENSE-lucide.txt`), drawn at 14 px with a 1.5 stroke
//! (*01 · Foundations → Icons*). The SVGs come from lucide-static 1.54.0 with `stroke-width` set to 1.5; the GPUI
//! app draws them as masks in the current text colour. The names and the SVGs live here so every UI draws the same
//! set.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! icons {
    ($($variant:ident = $file:literal;)*) => {
        /// On the page, an icon is named by its Lucide name (`"bell"`); its TypeScript type lists them all.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
        pub enum IconName {
            $(#[serde(rename = $file)] $variant,)*
        }

        impl IconName {
            pub const ALL: &[IconName] = &[$(IconName::$variant),*];

            /// The Lucide name, as used in the design (`message-circle-question-mark`).
            pub fn lucide_name(self) -> &'static str {
                match self {
                    $(Self::$variant => $file,)*
                }
            }

            /// The asset path, as passed to `svg().path(...)`.
            pub fn path(self) -> &'static str {
                match self {
                    $(Self::$variant => concat!("icons/", $file, ".svg"),)*
                }
            }

            /// The SVG bytes, embedded at build time.
            pub fn bytes(self) -> &'static [u8] {
                match self {
                    $(Self::$variant => include_bytes!(concat!("../assets/icons/", $file, ".svg")),)*
                }
            }
        }
    };
}

icons! {
    AArrowDown = "a-arrow-down";
    AArrowUp = "a-arrow-up";
    ALargeSmall = "a-large-small";
    Activity = "activity";
    AlarmClock = "alarm-clock";
    Archive = "archive";
    ArrowDown = "arrow-down";
    ArrowDownToLine = "arrow-down-to-line";
    ArrowRight = "arrow-right";
    ArrowUp = "arrow-up";
    ArrowUpRight = "arrow-up-right";
    Ban = "ban";
    Bell = "bell";
    Bot = "bot";
    BrushCleaning = "brush-cleaning";
    CalendarClock = "calendar-clock";
    Check = "check";
    CheckCheck = "check-check";
    ChevronDown = "chevron-down";
    ChevronLeft = "chevron-left";
    ChevronRight = "chevron-right";
    ChevronUp = "chevron-up";
    ChevronsLeftRight = "chevrons-left-right";
    ChevronsUp = "chevrons-up";
    ChevronsUpDown = "chevrons-up-down";
    Circle = "circle";
    CircleAlert = "circle-alert";
    CircleCheck = "circle-check";
    CircleCheckBig = "circle-check-big";
    CircleDashed = "circle-dashed";
    CircleMinus = "circle-minus";
    CirclePause = "circle-pause";
    CirclePlay = "circle-play";
    CircleQuestionMark = "circle-question-mark";
    CircleSlash = "circle-slash";
    CircleStop = "circle-stop";
    CircleX = "circle-x";
    Coins = "coins";
    Columns2 = "columns-2";
    Copy = "copy";
    CornerDownLeft = "corner-down-left";
    CornerDownRight = "corner-down-right";
    Cpu = "cpu";
    Database = "database";
    Dot = "dot";
    Ellipsis = "ellipsis";
    Equal = "equal";
    ExternalLink = "external-link";
    Eye = "eye";
    EyeOff = "eye-off";
    File = "file";
    FileDiff = "file-diff";
    FileSymlink = "file-symlink";
    FileText = "file-text";
    Folder = "folder";
    FolderGit2 = "folder-git-2";
    FolderOpen = "folder-open";
    FolderPlus = "folder-plus";
    Gauge = "gauge";
    GitBranch = "git-branch";
    GitCommitHorizontal = "git-commit-horizontal";
    GitCompare = "git-compare";
    GitCompareArrows = "git-compare-arrows";
    GitMerge = "git-merge";
    GitPullRequest = "git-pull-request";
    Hand = "hand";
    Hash = "hash";
    History = "history";
    Hourglass = "hourglass";
    Image = "image";
    Inbox = "inbox";
    Info = "info";
    Keyboard = "keyboard";
    Layers = "layers";
    LayoutDashboard = "layout-dashboard";
    Link2 = "link-2";
    ListChecks = "list-checks";
    Loader = "loader";
    LoaderCircle = "loader-circle";
    Lock = "lock";
    Maximize2 = "maximize-2";
    MessageCircleQuestionMark = "message-circle-question-mark";
    MessageCircleReply = "message-circle-reply";
    MessageSquare = "message-square";
    MessageSquareDot = "message-square-dot";
    MessageSquareText = "message-square-text";
    MessagesSquare = "messages-square";
    Minimize2 = "minimize-2";
    Minus = "minus";
    MousePointerClick = "mouse-pointer-click";
    Palette = "palette";
    PanelRight = "panel-right";
    Pencil = "pencil";
    Pin = "pin";
    PinOff = "pin-off";
    Play = "play";
    Plug = "plug";
    Plus = "plus";
    RefreshCcw = "refresh-ccw";
    RefreshCw = "refresh-cw";
    RotateCcw = "rotate-ccw";
    RotateCw = "rotate-cw";
    ScanSearch = "scan-search";
    ScrollText = "scroll-text";
    Search = "search";
    Send = "send";
    Server = "server";
    ServerCog = "server-cog";
    ServerOff = "server-off";
    Settings = "settings";
    Shield = "shield";
    ShieldAlert = "shield-alert";
    ShieldCheck = "shield-check";
    ShieldEllipsis = "shield-ellipsis";
    ShieldOff = "shield-off";
    ShieldQuestionMark = "shield-question-mark";
    ShieldX = "shield-x";
    SkipForward = "skip-forward";
    Sparkles = "sparkles";
    Square = "square";
    SquareCheck = "square-check";
    SquareParking = "square-parking";
    SquarePlus = "square-plus";
    SquareTerminal = "square-terminal";
    Star = "star";
    Tag = "tag";
    Terminal = "terminal";
    Trash2 = "trash-2";
    TriangleAlert = "triangle-alert";
    Undo2 = "undo-2";
    Unlink = "unlink";
    Upload = "upload";
    User = "user";
    UserCheck = "user-check";
    Workflow = "workflow";
    Wrench = "wrench";
    X = "x";
    ZapOff = "zap-off";
}

impl IconName {
    pub fn from_path(path: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|i| i.path() == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_bundled_with_a_thin_stroke() {
        assert!(IconName::ALL.len() >= 70);
        for icon in IconName::ALL {
            let text = std::str::from_utf8(icon.bytes()).unwrap();
            assert!(text.contains("<svg"), "{icon:?}");
            assert!(text.contains(r#"stroke-width="1.5""#), "{icon:?}");
            let shapes = [
                "<path",
                "<circle",
                "<rect",
                "<line",
                "<polyline",
                "<polygon",
                "<ellipse",
            ];
            assert!(
                shapes.iter().any(|s| text.contains(s)),
                "{icon:?} draws nothing"
            );
            assert_eq!(IconName::from_path(icon.path()), Some(*icon));
        }
    }

    /// Every Lucide icon `crewboard.pen` uses (read from the design on 2026-10-11, after design update M), except `github`: Lucide dropped
    /// it and the app shows `git-pull-request` instead (Q31). An icon added to the design shows up here as missing
    /// until it is bundled.
    const DESIGN_ICONS: &[&str] = &[
        "a-arrow-down",
        "a-arrow-up",
        "a-large-small",
        "activity",
        "alarm-clock",
        "archive",
        "arrow-down",
        "arrow-down-to-line",
        "arrow-right",
        "arrow-up",
        "arrow-up-right",
        "ban",
        "bell",
        "bot",
        "brush-cleaning",
        "calendar-clock",
        "check",
        "check-check",
        "chevron-down",
        "chevron-right",
        "chevron-up",
        "chevrons-left-right",
        "chevrons-up",
        "chevrons-up-down",
        "circle",
        "circle-alert",
        "circle-check",
        "circle-check-big",
        "circle-dashed",
        "circle-minus",
        "circle-pause",
        "circle-play",
        "circle-question-mark",
        "circle-slash",
        "circle-stop",
        "circle-x",
        "coins",
        "copy",
        "corner-down-left",
        "corner-down-right",
        "cpu",
        "database",
        "dot",
        "ellipsis",
        "equal",
        "external-link",
        "eye",
        "eye-off",
        "file",
        "file-diff",
        "file-symlink",
        "file-text",
        "folder",
        "folder-git-2",
        "folder-open",
        "folder-plus",
        "gauge",
        "git-branch",
        "git-commit-horizontal",
        "git-compare",
        "git-compare-arrows",
        "git-merge",
        "git-pull-request",
        "hand",
        "hash",
        "history",
        "hourglass",
        "image",
        "inbox",
        "info",
        "keyboard",
        "layers",
        "layout-dashboard",
        "link-2",
        "list-checks",
        "loader",
        "loader-circle",
        "lock",
        "maximize-2",
        "message-circle-question-mark",
        "message-circle-reply",
        "message-square",
        "message-square-dot",
        "message-square-text",
        "messages-square",
        "minimize-2",
        "minus",
        "mouse-pointer-click",
        "palette",
        "panel-right",
        "pencil",
        "pin",
        "pin-off",
        "play",
        "plug",
        "plus",
        "refresh-ccw",
        "refresh-cw",
        "rotate-ccw",
        "rotate-cw",
        "scan-search",
        "scroll-text",
        "search",
        "send",
        "server",
        "server-cog",
        "server-off",
        "settings",
        "shield",
        "shield-alert",
        "shield-check",
        "shield-ellipsis",
        "shield-off",
        "shield-question-mark",
        "shield-x",
        "skip-forward",
        "sparkles",
        "square",
        "square-check",
        "square-parking",
        "square-plus",
        "square-terminal",
        "star",
        "tag",
        "terminal",
        "trash-2",
        "triangle-alert",
        "undo-2",
        "unlink",
        "upload",
        "user",
        "user-check",
        "workflow",
        "wrench",
        "x",
        "zap-off",
    ];

    #[test]
    fn every_design_icon_is_bundled() {
        let missing: Vec<_> = DESIGN_ICONS
            .iter()
            .filter(|name| !IconName::ALL.iter().any(|i| i.lucide_name() == **name))
            .collect();
        assert!(missing.is_empty(), "design icons not bundled: {missing:?}");
    }

    #[test]
    fn names_match_lucide() {
        assert_eq!(
            IconName::MessageCircleQuestionMark.lucide_name(),
            "message-circle-question-mark"
        );
        assert_eq!(IconName::Hand.path(), "icons/hand.svg");
    }
}
