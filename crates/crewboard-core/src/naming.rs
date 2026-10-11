//! The product's names, in one place (d0028).
//!
//! The product name spreads into the command, the plugin namespace (`/crewboard:work-ticket`), branch names and
//! environment variables (`CREWBOARD_TICKET`). Every crate builds these strings from the constants below, so a
//! forced rename later means changing only this file. A test (`tests/naming_literals.rs`) fails if another source
//! file spells one of them out.

/// The product name shown to people.
pub const PRODUCT_NAME: &str = "Crewboard";

/// The command-line binary.
pub const COMMAND_NAME: &str = "crewboard";

/// The Claude Code plugin namespace, as in `/crewboard:work-ticket`.
pub const PLUGIN_NAMESPACE: &str = "crewboard";

/// The palette key of the bundled Crewboard themes, Crewboard Dark and Crewboard Light, as the design has it and as
/// d0069's settings stored both (d0070 stores `crewboard-dark` and `crewboard-light`).
pub const THEME_KEY: &str = "crewboard";

/// The stored names of Crewboard Dark and Crewboard Light (d0070).
pub const THEME_DARK: &str = "crewboard-dark";
pub const THEME_LIGHT: &str = "crewboard-light";

/// The prefix of every environment variable Crewboard reads or sets.
pub const ENV_PREFIX: &str = "CREWBOARD_";

/// An environment variable name: `env_var("HOME")` → `CREWBOARD_HOME`.
pub fn env_var(suffix: &str) -> String {
    format!("{ENV_PREFIX}{suffix}")
}

/// The variable that tells an agent session which ticket it works on: `CREWBOARD_TICKET`.
pub fn ticket_env_var() -> String {
    env_var("TICKET")
}

/// The variable that says what a `claude` session is for when it isn't the ticket's agent: `CREWBOARD_ROLE`
/// (`validator`, `project_agent`). Hooks and the CLI read it (d0063, d0068).
pub fn role_env_var() -> String {
    env_var("ROLE")
}

/// [`role_env_var`]'s value in a validator session.
pub const ROLE_VALIDATOR: &str = "validator";
/// [`role_env_var`]'s value in a Project agent session.
pub const ROLE_PROJECT_AGENT: &str = "project_agent";

/// A plugin skill as typed in Claude Code: `skill("work-ticket")` → `/crewboard:work-ticket`.
pub fn skill(name: &str) -> String {
    format!("/{PLUGIN_NAMESPACE}:{name}")
}

/// The prefix of branches Crewboard creates: `crewboard/`.
pub fn branch_prefix() -> String {
    format!("{COMMAND_NAME}/")
}

/// The data folder's name in the user's home folder: `.crewboard` (d0055).
pub fn data_dir_name() -> String {
    format!(".{COMMAND_NAME}")
}

/// The desktop app's binary, installed next to the command: `crewboard-app`.
pub fn app_binary_name() -> String {
    format!("{COMMAND_NAME}-app{}", std::env::consts::EXE_SUFFIX)
}

/// A branch Crewboard names: `branch("SHOP-40")` → `crewboard/SHOP-40`.
pub fn branch(name: &str) -> String {
    format!("{}{name}", branch_prefix())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants() {
        assert_eq!(PRODUCT_NAME, "Crewboard");
        assert_eq!(COMMAND_NAME, "crewboard");
        assert_eq!(PLUGIN_NAMESPACE, "crewboard");
        assert_eq!(ENV_PREFIX, "CREWBOARD_");
    }

    #[test]
    fn env_var_adds_the_prefix() {
        assert_eq!(env_var("HOME"), "CREWBOARD_HOME");
    }

    #[test]
    fn ticket_env_var_name() {
        assert_eq!(ticket_env_var(), "CREWBOARD_TICKET");
        assert_eq!(role_env_var(), "CREWBOARD_ROLE");
    }

    #[test]
    fn skill_uses_the_plugin_namespace() {
        assert_eq!(skill("work-ticket"), "/crewboard:work-ticket");
    }

    #[test]
    fn branch_names() {
        assert_eq!(branch_prefix(), "crewboard/");
        assert_eq!(branch("SHOP-40"), "crewboard/SHOP-40");
    }

    #[test]
    fn app_binary() {
        assert!(app_binary_name().starts_with("crewboard-app"));
    }

    #[test]
    fn data_dir_name_is_hidden() {
        assert_eq!(data_dir_name(), ".crewboard");
    }
}
