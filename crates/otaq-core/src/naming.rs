//! The product's names, in one place (d0028, d0076).
//!
//! The product name spreads into the command, the plugin namespace (`/otaq:work-ticket`), branch names and
//! environment variables (`OTAQ_TICKET`). Every crate builds these strings from the constants below, so a
//! forced rename later means changing only this file. A test (`tests/suite/naming_literals.rs`) fails if another source
//! file spells one of them out.

/// The product name shown to people.
pub const PRODUCT_NAME: &str = "Otaq";

/// The command-line binary.
pub const COMMAND_NAME: &str = "otaq";

/// The Claude Code plugin namespace, as in `/otaq:work-ticket`.
pub const PLUGIN_NAMESPACE: &str = "otaq";

/// The palette key of the bundled Otaq themes, Otaq Dark and Otaq Light, as the design has it and as
/// d0069's settings stored both (d0070 stores `otaq-dark` and `otaq-light`).
pub const THEME_KEY: &str = "otaq";

/// The stored names of Otaq Dark and Otaq Light (d0070).
pub const THEME_DARK: &str = "otaq-dark";
pub const THEME_LIGHT: &str = "otaq-light";

/// The prefix of every environment variable Otaq reads or sets.
pub const ENV_PREFIX: &str = "OTAQ_";

/// An environment variable name: `env_var("HOME")` → `OTAQ_HOME`.
pub fn env_var(suffix: &str) -> String {
    format!("{ENV_PREFIX}{suffix}")
}

/// The variable that tells an agent session which ticket it works on: `OTAQ_TICKET`.
pub fn ticket_env_var() -> String {
    env_var("TICKET")
}

/// The variable that says what an agent session is for when it isn't the ticket's agent: `OTAQ_ROLE`
/// (`validator`, `project_agent`). Hooks and the CLI read it (d0063, d0068).
pub fn role_env_var() -> String {
    env_var("ROLE")
}

/// [`role_env_var`]'s value in a validator session.
pub const ROLE_VALIDATOR: &str = "validator";
/// [`role_env_var`]'s value in a Project agent session.
pub const ROLE_PROJECT_AGENT: &str = "project_agent";
/// [`role_env_var`]'s value in an epic lead's session (d0083): a Project agent session for one epic.
pub const ROLE_EPIC_LEAD: &str = "epic_lead";

/// The variable that names an epic lead's epic: `OTAQ_EPIC` (d0083).
pub fn epic_env_var() -> String {
    env_var("EPIC")
}

/// The ticket resource's database name, set where a project has ticket resources (d0087): `OTAQ_DB_NAME`.
pub fn db_name_env_var() -> String {
    env_var("DB_NAME")
}

/// The project's template database name (d0087): `OTAQ_DB_TEMPLATE`.
pub fn db_template_env_var() -> String {
    env_var("DB_TEMPLATE")
}

/// The ticket's own resource folder in the data folder (d0087): `OTAQ_RESOURCE_DIR`.
pub fn resource_dir_env_var() -> String {
    env_var("RESOURCE_DIR")
}

/// The data folder's folder of ticket resource folders: `res` (`res/<repo>/<KEY>`, d0087).
pub fn resource_dir_name() -> &'static str {
    "res"
}

/// The first part of every database name Otaq computes: `otaq` (`otaq_shop_shop_18`, d0087).
pub fn db_prefix() -> &'static str {
    COMMAND_NAME
}

/// A plugin skill as typed in Claude Code: `skill("work-ticket")` → `/otaq:work-ticket`. Only the Claude Code
/// adapter names skills to agents with it (ADP-05), and Settings stores Otaq's own workflow skills in this form;
/// everything else asks the session's adapter ([`crate::agent_tools::AgentTool::skill_reference`]).
pub fn skill(name: &str) -> String {
    format!("/{PLUGIN_NAMESPACE}:{name}")
}

/// The folder (and `name:`) of an imported workflow skill's copy in a worktree (WSK-05, d0092):
/// `custom_skill("commit")` → `otaq-custom-commit`.
pub fn custom_skill(step: &str) -> String {
    format!("{COMMAND_NAME}-custom-{step}")
}

/// The prefix of branches Otaq creates: `otaq/`.
pub fn branch_prefix() -> String {
    format!("{COMMAND_NAME}/")
}

/// The data folder's name in the user's home folder: `.otaq` (d0055).
pub fn data_dir_name() -> String {
    format!(".{COMMAND_NAME}")
}

/// The desktop app's binary, installed next to the command: `otaq-app`.
pub fn app_binary_name() -> String {
    format!("{COMMAND_NAME}-app{}", std::env::consts::EXE_SUFFIX)
}

/// The macOS bundle's ID (d0072), under which macOS shows its notifications (d0090).
pub const BUNDLE_ID: &str = "io.otaq.desktop";

/// The macOS bundle's folder name: `Otaq.app` (d0072).
pub fn bundle_folder_name() -> String {
    format!("{PRODUCT_NAME}.app")
}

/// The default flow's file (STG-03, d0108), shipped embedded; its contents are [`DEFAULT_FLOW_YAML`].
pub const DEFAULT_FLOW_FILE: &str = "otaq-default.yaml";
/// The contents of [`DEFAULT_FLOW_FILE`], read by [`crate::flow::default`].
pub const DEFAULT_FLOW_YAML: &str = include_str!("../flows/otaq-default.yaml");

/// Otaq's agents file (AGM-02, d0108), shipped embedded; its contents are [`AGENTS_YAML`].
pub const AGENTS_FILE: &str = "otaq-agents.yaml";
/// The contents of [`AGENTS_FILE`], read by [`crate::agents::shipped`].
pub const AGENTS_YAML: &str = include_str!("../flows/otaq-agents.yaml");

/// A branch Otaq names: `branch("SHOP-40")` → `otaq/SHOP-40`.
pub fn branch(name: &str) -> String {
    format!("{}{name}", branch_prefix())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants() {
        assert_eq!(PRODUCT_NAME, "Otaq");
        assert_eq!(COMMAND_NAME, "otaq");
        assert_eq!(PLUGIN_NAMESPACE, "otaq");
        assert_eq!(ENV_PREFIX, "OTAQ_");
    }

    #[test]
    fn env_var_adds_the_prefix() {
        assert_eq!(env_var("HOME"), "OTAQ_HOME");
    }

    #[test]
    fn ticket_env_var_name() {
        assert_eq!(ticket_env_var(), "OTAQ_TICKET");
        assert_eq!(role_env_var(), "OTAQ_ROLE");
    }

    #[test]
    fn skill_uses_the_plugin_namespace() {
        assert_eq!(skill("work-ticket"), "/otaq:work-ticket");
    }

    #[test]
    fn branch_names() {
        assert_eq!(branch_prefix(), "otaq/");
        assert_eq!(branch("SHOP-40"), "otaq/SHOP-40");
    }

    #[test]
    fn app_binary() {
        assert!(app_binary_name().starts_with("otaq-app"));
    }

    #[test]
    fn resource_names() {
        assert_eq!(db_name_env_var(), "OTAQ_DB_NAME");
        assert_eq!(db_template_env_var(), "OTAQ_DB_TEMPLATE");
        assert_eq!(resource_dir_env_var(), "OTAQ_RESOURCE_DIR");
        assert_eq!(resource_dir_name(), "res");
        assert_eq!(db_prefix(), "otaq");
    }

    #[test]
    fn data_dir_name_is_hidden() {
        assert_eq!(data_dir_name(), ".otaq");
    }
}
