//! The bundled icon set: Lucide (ISC licence, `assets/icons/LICENSE-lucide.txt`), drawn at 14 px with a 1.5 stroke
//! (*01 · Foundations → Icons*). The SVGs come from lucide-static 1.54.0 with `stroke-width` set to 1.5; GPUI draws
//! them as masks in the current text colour.

use gpui::{Pixels, Styled, Svg, svg};

macro_rules! icons {
    ($($variant:ident = $file:literal;)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum IconName {
            $($variant,)*
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
    Activity = "activity";
    AlarmClock = "alarm-clock";
    Archive = "archive";
    ArrowRight = "arrow-right";
    ArrowUpRight = "arrow-up-right";
    Ban = "ban";
    Bot = "bot";
    BrushCleaning = "brush-cleaning";
    CalendarClock = "calendar-clock";
    Check = "check";
    ChevronDown = "chevron-down";
    ChevronRight = "chevron-right";
    ChevronsLeftRight = "chevrons-left-right";
    CircleAlert = "circle-alert";
    CircleCheck = "circle-check";
    CircleDashed = "circle-dashed";
    CircleMinus = "circle-minus";
    CirclePause = "circle-pause";
    CircleSlash = "circle-slash";
    CircleX = "circle-x";
    Coins = "coins";
    Copy = "copy";
    Cpu = "cpu";
    Ellipsis = "ellipsis";
    ExternalLink = "external-link";
    Eye = "eye";
    EyeOff = "eye-off";
    File = "file";
    FileDiff = "file-diff";
    FileText = "file-text";
    FolderGit2 = "folder-git-2";
    FolderOpen = "folder-open";
    FolderPlus = "folder-plus";
    Gauge = "gauge";
    GitBranch = "git-branch";
    GitCommitHorizontal = "git-commit-horizontal";
    GitCompareArrows = "git-compare-arrows";
    GitMerge = "git-merge";
    GitPullRequest = "git-pull-request";
    GripVertical = "grip-vertical";
    Hand = "hand";
    History = "history";
    Hourglass = "hourglass";
    Inbox = "inbox";
    Info = "info";
    Layers = "layers";
    LayoutDashboard = "layout-dashboard";
    Link2 = "link-2";
    ListChecks = "list-checks";
    LoaderCircle = "loader-circle";
    Lock = "lock";
    MessageCircleQuestionMark = "message-circle-question-mark";
    MessageCircleReply = "message-circle-reply";
    MessageSquareDot = "message-square-dot";
    MessageSquareText = "message-square-text";
    MessagesSquare = "messages-square";
    Minus = "minus";
    PanelRight = "panel-right";
    Pencil = "pencil";
    Pin = "pin";
    PinOff = "pin-off";
    Play = "play";
    Plug = "plug";
    Plus = "plus";
    RefreshCw = "refresh-cw";
    RotateCcw = "rotate-ccw";
    RotateCw = "rotate-cw";
    ScanSearch = "scan-search";
    Search = "search";
    Server = "server";
    ServerCog = "server-cog";
    ServerOff = "server-off";
    Settings = "settings";
    ShieldAlert = "shield-alert";
    ShieldCheck = "shield-check";
    Sparkles = "sparkles";
    Square = "square";
    SquareParking = "square-parking";
    SquareTerminal = "square-terminal";
    Tag = "tag";
    Terminal = "terminal";
    Trash2 = "trash-2";
    TriangleAlert = "triangle-alert";
    Undo2 = "undo-2";
    Upload = "upload";
    X = "x";
    ZapOff = "zap-off";
}

impl IconName {
    pub fn from_path(path: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|i| i.path() == path)
    }
}

/// An icon element; give it a colour with `.text_color(...)`.
pub fn icon(name: IconName, size: Pixels) -> Svg {
    svg().path(name.path()).size(size).flex_none()
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
            assert_eq!(IconName::from_path(icon.path()), Some(*icon));
        }
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
