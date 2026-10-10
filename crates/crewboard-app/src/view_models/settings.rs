//! Settings (SET-01, PRJ-03, PRJ-04; d0003, d0023, d0040, d0055; *15 · Settings*): one window with a section
//! nav; the global page (Agent instructions, Data, Appearance, Tools) and one page per project (name, key,
//! repository, base branch, agent instructions override, setup and run commands, Open URL, env files, Remove
//! project). Everything is stored in the app's database, never in a repository (d0023). Values the service
//! flags (a missing env file, a URL that isn't one) show in red with its reason; the nav marks the project.
//! Sections for later features are added by their tickets (limits T-0082 is in, workflow skills T-0139, usage
//! T-0126, notifications T-0117, PR provider T-0141, "let finished tickets answer" T-0153).

use crewboard_core::board::settings::{
    Appearance, GlobalSettings, ProjectSettings, SettingProblem, TERMINAL_FONT_SIZES,
};
use crewboard_core::board::snapshot::ProjectInfo;
use crewboard_core::data_dir::DataDir;

use crate::shell::tilde;
use crate::theme::ThemeChoice;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page {
    Global,
    Project(String),
}

/// The global page's sections, in nav order.
pub const GLOBAL_SECTIONS: [&str; 5] = [
    "Agent limits",
    "Agent instructions",
    "Data",
    "Appearance",
    "Tools",
];

/// One project's page data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPage {
    pub info: ProjectInfo,
    pub settings: ProjectSettings,
    pub problems: Vec<SettingProblem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsModel {
    pub global: GlobalSettings,
    pub global_problems: Vec<SettingProblem>,
    pub projects: Vec<ProjectPage>,
}

impl SettingsModel {
    pub fn project(&self, key: &str) -> Option<&ProjectPage> {
        self.projects.iter().find(|p| p.info.key == key)
    }

    /// Replaces (or adds) a project's page after loading or saving it.
    pub fn set_project(&mut self, page: ProjectPage) {
        match self
            .projects
            .iter_mut()
            .find(|p| p.info.key == page.info.key)
        {
            Some(slot) => *slot = page,
            None => self.projects.push(page),
        }
    }

    /// Whether the nav marks a project (it has a flagged value).
    pub fn needs_attention(&self, key: &str) -> bool {
        self.project(key).is_some_and(|p| !p.problems.is_empty())
    }
}

/// The message for a field, if the service flagged it.
pub fn problem_for(problems: &[SettingProblem], field: &str) -> Option<String> {
    problems
        .iter()
        .find(|p| p.field == field)
        .map(|p| p.message.clone())
}

/// The message for one env file, if it was flagged.
pub fn env_file_problem(problems: &[SettingProblem], file: &str) -> Option<String> {
    problems
        .iter()
        .filter(|p| p.field == "env_files")
        .find(|p| p.message.starts_with(&format!("{file} ")))
        .map(|p| p.message.clone())
}

/// What an empty project override says: "Empty — uses the global default: “…”".
pub fn instructions_placeholder(global: &GlobalSettings) -> String {
    if global.agent_instructions.trim().is_empty() {
        "Empty — no instructions are set globally either.".into()
    } else {
        format!(
            "Empty — uses the global default: “{}”",
            global.agent_instructions.trim()
        )
    }
}

pub fn theme_choice(appearance: Appearance) -> ThemeChoice {
    match appearance {
        Appearance::Light => ThemeChoice::Light,
        Appearance::Dark => ThemeChoice::Dark,
        Appearance::System => ThemeChoice::System,
    }
}

/// The database file and logs folder as shown in Data (d0055), with `~` for the home folder.
pub fn data_paths(data: &DataDir) -> (String, String) {
    (
        tilde(&data.database_path().display().to_string()),
        tilde(&format!("{}/", data.logs_dir().display())),
    )
}

/// Reads the terminal font size field.
pub fn parse_font_size(text: &str) -> Result<u32, String> {
    let range = TERMINAL_FONT_SIZES;
    match text.trim().trim_end_matches("px").trim().parse::<u32>() {
        Ok(n) if range.contains(&n) => Ok(n),
        _ => Err(format!(
            "Use a size from {} to {}",
            range.start(),
            range.end()
        )),
    }
}

/// The Remove project confirmation (PRJ-04): what goes, what is never touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveText {
    pub title: String,
    pub deleted: Vec<String>,
    pub kept: Vec<String>,
    /// "1 agent is running. Removing stops it first." (agents arrive in M3).
    pub agents: Option<String>,
}

pub fn remove_text(info: &ProjectInfo, tickets: usize) -> RemoveText {
    let product = crewboard_core::naming::PRODUCT_NAME;
    RemoveText {
        title: format!("Remove {} from {product}?", info.name),
        deleted: vec![
            format!(
                "{tickets} ticket{}, their runs and comments",
                if tickets == 1 { "" } else { "s" }
            ),
            "Project settings (commands, env files)".into(),
            format!("{product} worktrees and branches — listed in Cleanup first"),
        ],
        kept: vec![
            format!("The repository at {}", tilde(&info.repo_path)),
            "Your own branches, commits and remotes".into(),
            "Open pull requests".into(),
        ],
        agents: (info.agents_working > 0).then(|| {
            format!(
                "{} agent{} running. Removing stops {} first.",
                info.agents_working,
                if info.agents_working == 1 {
                    " is"
                } else {
                    "s are"
                },
                if info.agents_working == 1 {
                    "it"
                } else {
                    "them"
                }
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crewboard_core::naming::PRODUCT_NAME;

    fn info(working: u32) -> ProjectInfo {
        ProjectInfo {
            key: "BILL".into(),
            name: "billing-api".into(),
            repo_path: "/code/billing-api".into(),
            base_branch: "develop".into(),
            agents_working: working,
            needs_you: 0,
        }
    }

    #[test]
    fn flagged_values_show_per_field_and_mark_the_nav() {
        let problems = vec![
            SettingProblem {
                field: "env_files".into(),
                message: ".env.local doesn't exist in the repository".into(),
            },
            SettingProblem {
                field: "open_url".into(),
                message: "localhost isn't a URL".into(),
            },
        ];
        assert_eq!(
            env_file_problem(&problems, ".env.local").as_deref(),
            Some(".env.local doesn't exist in the repository")
        );
        assert_eq!(env_file_problem(&problems, "appsettings.json"), None);
        assert_eq!(
            problem_for(&problems, "open_url").as_deref(),
            Some("localhost isn't a URL")
        );
        let mut model = SettingsModel::default();
        model.set_project(ProjectPage {
            info: info(0),
            settings: ProjectSettings::default(),
            problems,
        });
        assert!(model.needs_attention("BILL"));
        model.set_project(ProjectPage {
            info: info(0),
            settings: ProjectSettings::default(),
            problems: Vec::new(),
        });
        assert_eq!(model.projects.len(), 1, "replaced, not added");
        assert!(!model.needs_attention("BILL"));
    }

    #[test]
    fn load_and_save_helpers() {
        let mut global = GlobalSettings::default();
        assert!(instructions_placeholder(&global).contains("no instructions"));
        global.agent_instructions = "Ask before coding.".into();
        assert_eq!(
            instructions_placeholder(&global),
            "Empty — uses the global default: “Ask before coding.”"
        );
        assert_eq!(theme_choice(Appearance::Dark), ThemeChoice::Dark);
        assert_eq!(parse_font_size("15 px"), Ok(15));
        assert!(parse_font_size("99").is_err());
        assert!(parse_font_size("big").is_err());
        let data = DataDir::new("/tmp/home/.x");
        let (db, logs) = data_paths(&data);
        assert!(db.ends_with(".db"));
        assert!(logs.ends_with("logs/"));
    }

    #[test]
    fn remove_confirmation_says_what_goes_and_what_stays() {
        let t = remove_text(&info(0), 2);
        assert_eq!(t.title, format!("Remove billing-api from {PRODUCT_NAME}?"));
        assert_eq!(t.deleted[0], "2 tickets, their runs and comments");
        assert_eq!(t.kept[0], "The repository at /code/billing-api");
        assert_eq!(t.agents, None);
        assert_eq!(
            remove_text(&info(1), 1).agents.as_deref(),
            Some("1 agent is running. Removing stops it first.")
        );
    }
}
