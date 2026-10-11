//! First run (UI-08, UI-11, SET-02; *14 · First run & onboarding*): Welcome, then the Theme step ("Pick a look",
//! d0069), then the tools checklist from `tools.check`. `claude` (and `git`) missing blocks Continue; gh and az are optional. The permission-mode
//! warning shows only when the check says "prompting", and stays away once dismissed. Nothing on these screens
//! installs a tool or changes a setting (d0021, d0011): the commands are only shown for copying.
//!
//! After Tools comes 4 · Project (the add-project form, T-0034); once a project is added the onboarding steps
//! disappear and the empty board is step 5 · Done.

use crewboard_core::naming;
use crewboard_core::tools::{PermissionVerdict, Tool, ToolStatus, ToolsReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Welcome,
    Theme,
    Tools,
    Project,
}

/// The step indicator in the title bar.
pub const STEPS: [&str; 5] = ["Welcome", "Theme", "Tools", "Project", "Done"];

impl Step {
    /// The step's place in [`STEPS`] (0-based; the indicator shows it plus one).
    pub fn index(self) -> usize {
        match self {
            Self::Welcome => 0,
            Self::Theme => 1,
            Self::Tools => 2,
            Self::Project => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowIcon {
    /// Found: green check.
    Found,
    /// A required tool is missing: red cross, red row.
    MissingRequired,
    /// An optional tool is missing: grey minus.
    MissingOptional,
}

/// One row of the checklist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRow {
    pub tool: Tool,
    pub name: &'static str,
    pub required: bool,
    pub icon: RowIcon,
    /// "2.46.0", "2.62.0 · logged in", "not found on PATH", "not found — needed for GitHub PRs".
    pub status: String,
    /// Why a missing required tool matters.
    pub explanation: Option<String>,
    /// A command to copy (install, add the extension, or log in).
    pub command: Option<String>,
}

/// The Continue button on the tools step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinueState {
    pub enabled: bool,
    /// Shown next to the disabled button: "Install claude to continue".
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OnboardingModel {
    pub step: Step,
    pub report: Option<ToolsReport>,
    pub checking: bool,
    /// The last `tools.check` failed (service unreachable).
    pub error: Option<String>,
    pub permission_dismissed: bool,
}

impl OnboardingModel {
    pub fn new(permission_dismissed: bool) -> Self {
        Self {
            step: Step::Welcome,
            report: None,
            checking: false,
            error: None,
            permission_dismissed,
        }
    }

    /// "Choose a theme" on Welcome: the Theme step.
    pub fn choose_theme(&mut self) {
        self.step = Step::Theme;
    }

    /// "Check my tools" on the Theme step: go to the tools step and run the check.
    pub fn start_check(&mut self) {
        self.step = Step::Tools;
        self.checking = true;
        self.error = None;
    }

    pub fn finish_check(&mut self, result: Result<ToolsReport, String>) {
        self.checking = false;
        match result {
            Ok(report) => {
                self.report = Some(report);
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub fn rows(&self) -> Vec<ToolRow> {
        self.report
            .as_ref()
            .map(|r| r.tools.iter().map(tool_row).collect())
            .unwrap_or_default()
    }

    pub fn continue_state(&self) -> ContinueState {
        let Some(report) = &self.report else {
            return ContinueState {
                enabled: false,
                reason: None,
            };
        };
        let missing: Vec<&str> = report
            .tools
            .iter()
            .filter(|t| t.required && !t.found)
            .map(|t| t.tool.command())
            .collect();
        if missing.is_empty() {
            ContinueState {
                enabled: !self.checking,
                reason: None,
            }
        } else {
            ContinueState {
                enabled: false,
                reason: Some(format!("Install {} to continue", missing.join(" and "))),
            }
        }
    }

    pub fn show_permission_warning(&self) -> bool {
        !self.permission_dismissed
            && self
                .report
                .as_ref()
                .is_some_and(|r| r.permission.verdict == PermissionVerdict::Prompting)
    }
}

fn tool_row(t: &ToolStatus) -> ToolRow {
    let name = match t.tool {
        Tool::Az => "az + azure-devops",
        other => other.command(),
    };
    if !t.found {
        let (status, explanation) = match t.tool {
            Tool::Git => (
                "not found on PATH".to_owned(),
                Some(format!(
                    "{} runs your own git for worktrees, branches and merges.",
                    naming::PRODUCT_NAME
                )),
            ),
            Tool::Claude => (
                "not found on PATH".to_owned(),
                Some(format!(
                    "{} runs every agent as a real claude session — nothing can start without it.",
                    naming::PRODUCT_NAME
                )),
            ),
            Tool::Gh => ("not found — needed for GitHub PRs".to_owned(), None),
            Tool::Az => ("not found — needed for Azure DevOps PRs".to_owned(), None),
        };
        return ToolRow {
            tool: t.tool,
            name,
            required: t.required,
            icon: if t.required {
                RowIcon::MissingRequired
            } else {
                RowIcon::MissingOptional
            },
            status,
            explanation,
            command: t.install_command.clone(),
        };
    }
    let mut parts = vec![t.version.clone().unwrap_or_else(|| "found".to_owned())];
    if let Some(installed) = t.extension_installed {
        parts.push(
            if installed {
                "extension installed"
            } else {
                "extension missing"
            }
            .to_owned(),
        );
    }
    if let Some(logged_in) = t.logged_in {
        parts.push(
            if logged_in {
                "logged in"
            } else {
                "not logged in"
            }
            .to_owned(),
        );
    }
    ToolRow {
        tool: t.tool,
        name,
        required: t.required,
        icon: RowIcon::Found,
        status: parts.join(" · "),
        explanation: None,
        command: t
            .install_command
            .clone()
            .or_else(|| t.login_command.clone()),
    }
}

#[cfg(test)]
mod tests {
    use crewboard_core::tools::{PermissionCheck, PermissionVerdict};

    use super::*;

    fn status(tool: Tool, found: bool) -> ToolStatus {
        ToolStatus {
            tool,
            required: tool.required(),
            found,
            version: None,
            logged_in: None,
            extension_installed: None,
            install_command: (!found).then(|| tool.install_command().to_owned()),
            login_command: None,
        }
    }

    fn report(tools: Vec<ToolStatus>, verdict: PermissionVerdict) -> ToolsReport {
        ToolsReport {
            tools,
            permission: PermissionCheck {
                verdict,
                mode: None,
                source: None,
            },
        }
    }

    fn everything_installed() -> Vec<ToolStatus> {
        vec![
            ToolStatus {
                version: Some("2.46.0".into()),
                ..status(Tool::Git, true)
            },
            ToolStatus {
                version: Some("Claude Code 2.1.4".into()),
                ..status(Tool::Claude, true)
            },
            ToolStatus {
                version: Some("2.62.0".into()),
                logged_in: Some(true),
                ..status(Tool::Gh, true)
            },
            ToolStatus {
                version: Some("2.67.0".into()),
                logged_in: Some(true),
                extension_installed: Some(true),
                ..status(Tool::Az, true)
            },
        ]
    }

    fn checked(tools: Vec<ToolStatus>, verdict: PermissionVerdict) -> OnboardingModel {
        let mut m = OnboardingModel::new(false);
        m.start_check();
        m.finish_check(Ok(report(tools, verdict)));
        m
    }

    #[test]
    fn welcome_then_theme_then_tools() {
        let mut m = OnboardingModel::new(false);
        assert_eq!(m.step, Step::Welcome);
        assert!(m.rows().is_empty());
        assert!(!m.continue_state().enabled);
        m.choose_theme();
        assert_eq!(m.step, Step::Theme);
        assert!(!m.checking, "the check starts with Check my tools");
        m.start_check();
        assert_eq!(m.step, Step::Tools);
        assert!(m.checking);
    }

    #[test]
    fn the_indicator_has_five_steps_with_theme_second() {
        assert_eq!(STEPS, ["Welcome", "Theme", "Tools", "Project", "Done"]);
        for (step, n) in [
            (Step::Welcome, 1),
            (Step::Theme, 2),
            (Step::Tools, 3),
            (Step::Project, 4),
        ] {
            assert_eq!(step.index() + 1, n, "{step:?}");
            assert_eq!(STEPS[step.index()], format!("{step:?}"));
        }
    }

    #[test]
    fn everything_installed_matches_the_frame() {
        let m = checked(everything_installed(), PermissionVerdict::NotPrompting);
        let statuses: Vec<(&str, String, RowIcon)> = m
            .rows()
            .into_iter()
            .map(|r| (r.name, r.status, r.icon))
            .collect();
        assert_eq!(
            statuses,
            [
                ("git", "2.46.0".to_owned(), RowIcon::Found),
                ("claude", "Claude Code 2.1.4".to_owned(), RowIcon::Found),
                ("gh", "2.62.0 · logged in".to_owned(), RowIcon::Found),
                (
                    "az + azure-devops",
                    "2.67.0 · extension installed · logged in".to_owned(),
                    RowIcon::Found
                ),
            ]
        );
        assert!(m.rows().iter().all(|r| r.command.is_none()));
        assert_eq!(
            m.continue_state(),
            ContinueState {
                enabled: true,
                reason: None
            }
        );
        assert!(!m.show_permission_warning());
    }

    #[test]
    fn claude_missing_blocks_continue() {
        let mut tools = everything_installed();
        tools[1] = status(Tool::Claude, false);
        let m = checked(tools, PermissionVerdict::Unknown);
        let claude = &m.rows()[1];
        assert_eq!(claude.icon, RowIcon::MissingRequired);
        assert_eq!(claude.status, "not found on PATH");
        assert!(
            claude
                .explanation
                .as_deref()
                .unwrap()
                .contains("real claude session")
        );
        assert_eq!(
            claude.command.as_deref(),
            Some(Tool::Claude.install_command())
        );
        let cont = m.continue_state();
        assert!(!cont.enabled);
        assert_eq!(cont.reason.as_deref(), Some("Install claude to continue"));
    }

    #[test]
    fn gh_and_az_missing_do_not_block() {
        let mut tools = everything_installed();
        tools[2] = status(Tool::Gh, false);
        tools[3] = status(Tool::Az, false);
        let m = checked(tools, PermissionVerdict::Prompting);
        let rows = m.rows();
        assert_eq!(rows[2].status, "not found — needed for GitHub PRs");
        assert_eq!(rows[2].icon, RowIcon::MissingOptional);
        // The command of the OS the app runs on ("brew install gh", "winget install GitHub.cli", …).
        assert_eq!(rows[2].command.as_deref(), Some(Tool::Gh.install_command()));
        assert_eq!(rows[3].status, "not found — needed for Azure DevOps PRs");
        assert!(m.continue_state().enabled);
        assert!(
            m.show_permission_warning(),
            "this step can show the warning"
        );
    }

    #[test]
    fn logged_out_and_missing_extension_offer_the_fix() {
        let mut tools = everything_installed();
        tools[2].logged_in = Some(false);
        tools[2].login_command = Some("gh auth login".into());
        tools[3].extension_installed = Some(false);
        tools[3].install_command = Some("az extension add --name azure-devops".into());
        let m = checked(tools, PermissionVerdict::NotPrompting);
        let rows = m.rows();
        assert_eq!(rows[2].status, "2.62.0 · not logged in");
        assert_eq!(rows[2].command.as_deref(), Some("gh auth login"));
        assert_eq!(rows[3].status, "2.67.0 · extension missing · logged in");
        assert_eq!(
            rows[3].command.as_deref(),
            Some("az extension add --name azure-devops")
        );
    }

    #[test]
    fn permission_warning_only_when_prompting_and_not_dismissed() {
        for (verdict, shown) in [
            (PermissionVerdict::Prompting, true),
            (PermissionVerdict::NotPrompting, false),
            (PermissionVerdict::Unknown, false),
        ] {
            assert_eq!(
                checked(everything_installed(), verdict).show_permission_warning(),
                shown,
                "{verdict:?}"
            );
        }
        let mut m = checked(everything_installed(), PermissionVerdict::Prompting);
        m.permission_dismissed = true;
        assert!(!m.show_permission_warning());
    }

    #[test]
    fn a_failed_check_keeps_the_last_report_and_says_why() {
        let mut m = checked(everything_installed(), PermissionVerdict::Unknown);
        m.start_check();
        assert!(!m.continue_state().enabled, "not while re-checking");
        m.finish_check(Err("not connected".into()));
        assert_eq!(m.error.as_deref(), Some("not connected"));
        assert_eq!(m.rows().len(), 4);
        assert!(m.continue_state().enabled);
    }
}
