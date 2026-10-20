//! Launch and resume (ADP-09, SES-03; T-0354). A new conversation is created first by [`super::prepare`], then:
//!
//! ```text
//! OTAQ_TICKET=SHOP-18  PORT=<agent port>  PATH=<otaq bin dir>:$PATH
//! OPENCODE_CONFIG_CONTENT={"plugins":["<otaq plugin>/opencode"],"skills":["<otaq plugin>/skills"],
//!                          "permissions":[{"action":"shell","resource":"otaq *","effect":"allow"}],"model":"<m>"}
//! opencode --standalone --session <ses_…> --prompt "<first prompt>"
//! ```
//!
//! Always `--standalone`: the default mode attaches to the user's shared OpenCode service, which ignores the
//! launch's environment and config (hands-on check). Everything per launch goes in `OPENCODE_CONFIG_CONTENT`, which
//! adds to the user's own config: Otaq's plugin folder, its skills folder, the one rule pre-approving `otaq`
//! commands, and the model when one is set. Never `OPENCODE_CONFIG_DIR` (it replaces the user's config folder) and
//! never `--auto` (it approves every prompt; d0011). The user's own data folder is used (no `XDG_*` change), so
//! the session shows in their OpenCode session list. Resume is `--session <id>` with an optional prompt; the
//! session's stored model wins over the config there.

use serde_json::{Value, json};

use super::{COMMAND_NAME, DISPLAY_NAME, OpenCode, OpenCodeContext, prepare};
use crate::agent_tools::{
    AgentTool, FirstPrompt, LaunchContext, LaunchError, LaunchPlan, LaunchRequest, LaunchRole,
    ToolIdMode,
};
use crate::naming;

/// The variable OpenCode reads its extra config from (not in `--help`; checked on 2.0.17).
const CONFIG_CONTENT: &str = "OPENCODE_CONFIG_CONTENT";

/// The folder of Otaq's OpenCode plugin inside Otaq's plugin folder (`package.json` and `index.js`).
const PLUGIN_FOLDER: &str = "opencode";

/// The folder of Otaq's skills inside its plugin folder (one copy for every tool, ADP-05).
const SKILLS_FOLDER: &str = "skills";

pub(super) fn launch(
    adapter: &OpenCode,
    request: &LaunchRequest,
) -> Result<LaunchPlan, LaunchError> {
    let launch = adapter
        .context()
        .tool
        .launch
        .as_ref()
        .ok_or_else(plugin_missing)?;
    let model = request
        .model
        .as_deref()
        .map(|m| {
            prepare::model_ref(m).map(|r| (m, r)).ok_or_else(|| {
                LaunchError(format!(
                    "{DISPLAY_NAME} can't run the model {m}: it names no provider"
                ))
            })
        })
        .transpose()?;
    let path = crate::shell::prepend_path(Some(&launch.bin_dir), &launch.path)
        .to_string_lossy()
        .into_owned();
    let prompt = request
        .first_prompt
        .as_ref()
        .map(|p| prompt_text(adapter, p));
    let (id, tool_id) = match &request.resume {
        Some(id) => (id.clone(), ToolIdMode::ChosenUpFront(id.clone())),
        None => {
            if prompt.is_none() {
                return Err(LaunchError(
                    "a new conversation needs a first prompt".to_owned(),
                ));
            }
            let id = prepare::new_session_id(crate::store::now_ms());
            let step = prepare::create_session(
                &id,
                &session_title(request, request.title.as_deref()),
                model.as_ref().map(|(_, r)| r.clone()),
                &request.cwd,
                path.clone(),
            );
            (id.clone(), ToolIdMode::CreatedBeforeLaunch { id, step })
        }
    };
    let mut args = vec!["--standalone".to_owned(), "--session".to_owned(), id];
    if let Some(prompt) = prompt {
        args.extend(["--prompt".to_owned(), prompt]);
    }
    let mut env = otaq_env(request, path)?;
    env.push((
        CONFIG_CONTENT.to_owned(),
        config(launch, model.map(|(m, _)| m)).to_string(),
    ));
    Ok(LaunchPlan {
        program: COMMAND_NAME.to_owned(),
        args,
        cwd: request.cwd.clone(),
        env,
        tool_id,
    })
}

/// Launches can be built once Otaq's plugin folder (its OpenCode plugin and skills) was found.
pub(super) fn unavailable(context: &OpenCodeContext) -> Option<LaunchError> {
    context.tool.launch.is_none().then(plugin_missing)
}

/// "opencode --session <id>".
pub(crate) fn resume_hint(tool_id: &str) -> String {
    format!("{COMMAND_NAME} --session {tool_id}")
}

/// One of Otaq's skills named for the agent: the skill's name. OpenCode lists every skill of the launch's
/// `skills` folder to the model by id and name (`<available_skills>`) and its skill tool loads one by name; it has no
/// syntax of its own for running a skill from typed text (the TUI's `@<skill>` is an attachment made by its picker,
/// not parsed from text; checked on 2.0.17).
pub(super) fn skill_reference(skill: &str) -> String {
    skill.to_owned()
}

/// Why no launch can be built: the plugin folder wasn't found.
fn plugin_missing() -> LaunchError {
    LaunchError(format!(
        "{}'s plugin wasn't found next to the {} command",
        naming::PRODUCT_NAME,
        naming::COMMAND_NAME
    ))
}

/// The session's title in the user's OpenCode session list: "Otaq · SHOP-18 · Add coupon codes" with the ticket's
/// title, else "Otaq · SHOP-18"; "Otaq · SHOP-31" for an epic's lead, "Otaq · SHOP" for a Project agent.
pub(super) fn session_title(request: &LaunchRequest, ticket_title: Option<&str>) -> String {
    let what = request
        .ticket
        .as_ref()
        .or(request.epic.as_ref())
        .map_or_else(|| request.project.clone(), ToString::to_string);
    match ticket_title.map(str::trim).filter(|t| !t.is_empty()) {
        Some(title) => format!("{} · {what} · {title}", naming::PRODUCT_NAME),
        None => format!("{} · {what}", naming::PRODUCT_NAME),
    }
}

/// The prompt as sent: an Otaq skill by name (with the epic lead's message after it), a workflow skill named
/// for the working folder, or the text.
fn prompt_text(adapter: &OpenCode, prompt: &FirstPrompt) -> String {
    match prompt {
        FirstPrompt::OtaqSkill { name, message } => {
            let skill = skill_reference(name);
            match message.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
                Some(message) => format!("{skill} {message}"),
                None => skill,
            }
        }
        FirstPrompt::WorkflowSkill { step, imported } => {
            adapter.workflow_skill(*step, *imported).prompt()
        }
        FirstPrompt::Text(text) => text.clone(),
    }
}

/// Otaq's own variables for the role, as every tool gets them: the ticket and its port (a validator has no port),
/// the role, an epic lead's epic, and PATH with the `otaq` folder first.
fn otaq_env(request: &LaunchRequest, path: String) -> Result<Vec<(String, String)>, LaunchError> {
    let ticket = || {
        request
            .ticket
            .as_ref()
            .map(ToString::to_string)
            .ok_or_else(|| LaunchError(format!("{:?} needs a ticket", request.role)))
    };
    let mut env = Vec::new();
    match request.role {
        LaunchRole::TicketAgent | LaunchRole::Answer | LaunchRole::Delivery => {
            env.push((naming::ticket_env_var(), ticket()?));
            env.push(("PORT".to_owned(), request.port.to_string()));
        }
        LaunchRole::Validator => {
            env.push((naming::ticket_env_var(), ticket()?));
            env.push((naming::role_env_var(), naming::ROLE_VALIDATOR.to_owned()));
        }
        LaunchRole::ProjectAgent => {
            env.push((
                naming::role_env_var(),
                naming::ROLE_PROJECT_AGENT.to_owned(),
            ));
        }
        LaunchRole::EpicLead => {
            let epic = request
                .epic
                .as_ref()
                .ok_or_else(|| LaunchError("an epic lead needs its epic".to_owned()))?;
            env.push((naming::role_env_var(), naming::ROLE_EPIC_LEAD.to_owned()));
            env.push((naming::epic_env_var(), epic.to_string()));
        }
    }
    env.push(("PATH".to_owned(), path));
    Ok(env)
}

/// `OPENCODE_CONFIG_CONTENT`: Otaq's plugin, its skills, the one `otaq *` shell rule, and `model` when set.
fn config(launch: &LaunchContext, model: Option<&str>) -> Value {
    let mut config = json!({
        "plugins": [launch.plugin_dir.join(PLUGIN_FOLDER).display().to_string()],
        "skills": [launch.plugin_dir.join(SKILLS_FOLDER).display().to_string()],
        "permissions": [{
            "action": "shell",
            "resource": format!("{} *", naming::COMMAND_NAME),
            "effect": "allow",
        }],
    });
    if let Some(model) = model {
        config["model"] = json!(model);
    }
    config
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::agent_tools::claude::ClaudeLaunch;
    use crate::agent_tools::test_support::{context, follow_validate, only_the_prompt_differs};
    use crate::agent_tools::{PrepareStep, WORK_TICKET_SKILL};
    use crate::board::settings::WorkflowStep;

    fn adapter() -> OpenCode {
        let mut tools = context(Path::new("/nowhere"), None);
        tools.launch = Some(LaunchContext {
            bin_dir: "/opt/cb/bin".into(),
            plugin_dir: "/opt/cb/plugin".into(),
            path: std::env::join_paths(["/usr/bin", "/bin"]).unwrap(),
            claude: ClaudeLaunch {
                program: vec!["claude".into()],
                config_dir: None,
            },
        });
        OpenCode::new(tools)
    }

    fn path() -> String {
        std::env::join_paths(["/opt/cb/bin", "/usr/bin", "/bin"])
            .unwrap()
            .into_string()
            .unwrap()
    }

    fn request(role: LaunchRole, model: Option<&str>) -> LaunchRequest {
        LaunchRequest {
            role,
            cwd: "/wt/shop-web/SHOP-50".into(),
            project: "SHOP".into(),
            ticket: Some("SHOP-50".parse().unwrap()),
            epic: None,
            port: 4012,
            first_prompt: Some(FirstPrompt::OtaqSkill {
                name: WORK_TICKET_SKILL.into(),
                message: None,
            }),
            resume: None,
            model: model.map(Into::into),
            title: None,
            effort: None,
        }
    }

    fn env<'a>(plan: &'a LaunchPlan, name: &str) -> Option<&'a str> {
        plan.env
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn config(plan: &LaunchPlan) -> Value {
        serde_json::from_str(env(plan, CONFIG_CONTENT).expect("config")).unwrap()
    }

    /// ADP-09, ADP-04: the session is made first (its id, title and model), then OpenCode opens it with the first
    /// prompt; the config carries the plugin, the skills, the one `otaq *` rule and the model.
    #[test]
    fn a_new_conversation_is_made_before_launch() {
        let plan = launch(
            &adapter(),
            &request(LaunchRole::TicketAgent, Some("opencode/big-pickle")),
        )
        .unwrap();
        let ToolIdMode::CreatedBeforeLaunch { id, step } = &plan.tool_id else {
            panic!("{:?}", plan.tool_id)
        };
        assert!(id.starts_with("ses_") && id.len() == 30, "{id}");
        assert!(id[4..16].bytes().all(|b| b.is_ascii_hexdigit()), "{id}");
        assert!(id[16..].bytes().all(|b| b.is_ascii_alphanumeric()), "{id}");
        let body: Value = serde_json::from_str(step.args.last().unwrap()).unwrap();
        assert_eq!(
            body,
            json!({
                "id": id,
                "title": format!("{} · SHOP-50", naming::PRODUCT_NAME),
                "model": {"providerID": "opencode", "id": "big-pickle"}
            })
        );
        assert_eq!(
            step,
            &PrepareStep {
                program: "opencode".into(),
                args: vec![
                    "api".into(),
                    "--standalone".into(),
                    "POST".into(),
                    "/api/session".into(),
                    "-d".into(),
                    step.args.last().unwrap().clone(),
                ],
                cwd: "/wt/shop-web/SHOP-50".into(),
                env: vec![("PATH".into(), path())],
            }
        );
        assert_eq!(plan.program, "opencode");
        assert_eq!(
            plan.args,
            [
                "--standalone",
                "--session",
                id.as_str(),
                "--prompt",
                "work-ticket"
            ]
        );
        assert_eq!(plan.cwd, Path::new("/wt/shop-web/SHOP-50"));
        assert_eq!(
            config(&plan),
            json!({
                "plugins": ["/opt/cb/plugin/opencode"],
                "skills": ["/opt/cb/plugin/skills"],
                "permissions": [{
                    "action": "shell",
                    "resource": format!("{} *", naming::COMMAND_NAME),
                    "effect": "allow"
                }],
                "model": "opencode/big-pickle"
            })
        );
        assert_eq!(env(&plan, &naming::ticket_env_var()), Some("SHOP-50"));
        assert_eq!(env(&plan, "PORT"), Some("4012"));
        assert_eq!(env(&plan, "PATH"), Some(path().as_str()));
        assert_eq!(plan.env.len(), 4);
    }

    /// No model: no `model` key and no model in the session made first (OpenCode's own default).
    #[test]
    fn no_model_no_key() {
        let plan = launch(&adapter(), &request(LaunchRole::TicketAgent, None)).unwrap();
        assert!(config(&plan).get("model").is_none());
        let ToolIdMode::CreatedBeforeLaunch { step, .. } = &plan.tool_id else {
            panic!()
        };
        let body: Value = serde_json::from_str(step.args.last().unwrap()).unwrap();
        assert!(body.get("model").is_none());
        // A variant goes with the session's model; a name without a provider is refused.
        let plan = launch(
            &adapter(),
            &request(
                LaunchRole::TicketAgent,
                Some("anthropic/claude-sonnet-5-5#high"),
            ),
        )
        .unwrap();
        let ToolIdMode::CreatedBeforeLaunch { step, .. } = &plan.tool_id else {
            panic!()
        };
        let body: Value = serde_json::from_str(step.args.last().unwrap()).unwrap();
        assert_eq!(
            body["model"],
            json!({"providerID": "anthropic", "id": "claude-sonnet-5-5", "variant": "high"})
        );
        assert!(
            launch(
                &adapter(),
                &request(LaunchRole::TicketAgent, Some("big-pickle"))
            )
            .is_err()
        );
    }

    /// Resume opens the stored session, with a prompt when there is one; nothing is made first.
    #[test]
    fn resume() {
        let mut resumed = request(LaunchRole::TicketAgent, Some("opencode/big-pickle"));
        resumed.resume = Some("ses_eb4534bbfffee5hwyyyzWkBMXI".into());
        resumed.first_prompt = Some(FirstPrompt::Text("address-review".into()));
        let plan = launch(&adapter(), &resumed).unwrap();
        assert_eq!(
            plan.tool_id,
            ToolIdMode::ChosenUpFront("ses_eb4534bbfffee5hwyyyzWkBMXI".into())
        );
        assert_eq!(
            plan.args,
            [
                "--standalone",
                "--session",
                "ses_eb4534bbfffee5hwyyyzWkBMXI",
                "--prompt",
                "address-review"
            ]
        );
        resumed.first_prompt = None;
        let plan = launch(&adapter(), &resumed).unwrap();
        assert_eq!(
            plan.args,
            [
                "--standalone",
                "--session",
                "ses_eb4534bbfffee5hwyyyzWkBMXI"
            ]
        );
        assert_eq!(
            resume_hint("ses_eb4534bb"),
            "opencode --session ses_eb4534bb"
        );
    }

    /// Each role's own variables, as Claude Code's launches set them.
    #[test]
    fn roles() {
        let validator = launch(&adapter(), &request(LaunchRole::Validator, None)).unwrap();
        assert_eq!(env(&validator, "PORT"), None);
        assert_eq!(
            env(&validator, &naming::role_env_var()),
            Some(naming::ROLE_VALIDATOR)
        );
        let mut project = request(LaunchRole::ProjectAgent, None);
        project.ticket = None;
        project.cwd = "/repos/shop-web".into();
        let plan = launch(&adapter(), &project).unwrap();
        assert_eq!(env(&plan, &naming::ticket_env_var()), None);
        assert_eq!(
            env(&plan, &naming::role_env_var()),
            Some(naming::ROLE_PROJECT_AGENT)
        );
        let ToolIdMode::CreatedBeforeLaunch { step, .. } = &plan.tool_id else {
            panic!()
        };
        assert!(step.args.last().unwrap().contains(" · SHOP\""));
        let mut lead = project.clone();
        lead.role = LaunchRole::EpicLead;
        lead.epic = Some("SHOP-31".parse().unwrap());
        lead.first_prompt = Some(FirstPrompt::OtaqSkill {
            name: "epic-lead".into(),
            message: Some("Start with the cart".into()),
        });
        let plan = launch(&adapter(), &lead).unwrap();
        assert_eq!(env(&plan, &naming::epic_env_var()), Some("SHOP-31"));
        assert_eq!(plan.args.last().unwrap(), "epic-lead Start with the cart");
        lead.epic = None;
        assert!(launch(&adapter(), &lead).is_err());
    }

    /// d0011: never a permission or bypass flag, never a config folder that replaces the user's.
    #[test]
    fn never_auto_or_a_config_folder() {
        let plan = launch(&adapter(), &request(LaunchRole::TicketAgent, None)).unwrap();
        assert!(
            !plan
                .args
                .iter()
                .any(|a| a == "--auto" || a.contains("yolo"))
        );
        assert!(
            !plan
                .env
                .iter()
                .any(|(n, _)| n == "OPENCODE_CONFIG_DIR" || n.starts_with("XDG_"))
        );
    }

    /// Without Otaq's plugin folder no launch can be built.
    #[test]
    fn without_the_plugin_folder() {
        let adapter = OpenCode::new(context(Path::new("/nowhere"), None));
        let refusal = unavailable(adapter.context()).unwrap();
        assert!(refusal.0.contains("plugin wasn't found"), "{refusal}");
        assert_eq!(
            launch(&adapter, &request(LaunchRole::TicketAgent, None)),
            Err(refusal)
        );
        assert_eq!(unavailable(self::adapter().context()), None);
    }

    /// The title with and without the ticket's own title.
    #[test]
    fn session_titles() {
        let request = request(LaunchRole::TicketAgent, None);
        let name = naming::PRODUCT_NAME;
        assert_eq!(session_title(&request, None), format!("{name} · SHOP-50"));
        assert_eq!(
            session_title(&request, Some("Order totals rounding")),
            format!("{name} · SHOP-50 · Order totals rounding")
        );
    }

    /// Two ids made at once differ, and a later one sorts before an earlier one (OpenCode's own order).
    #[test]
    fn session_ids_sort_newest_first() {
        let a = prepare::new_session_id(1_792_270_972_289);
        let b = prepare::new_session_id(1_792_270_972_289);
        let later = prepare::new_session_id(1_792_270_999_000);
        assert_ne!(a, b);
        assert!(later < a && later < b);
        assert_eq!(&a[..10], "ses_eb4534");
    }

    /// A workflow skill as a first prompt (WSK-06, d0092): Otaq's own by its name, an imported one by its worktree
    /// folder; the launch and its config are otherwise the same, with no `external_directory` rule (d0011).
    #[test]
    fn a_workflow_skill_first_prompt() {
        let mut req = request(LaunchRole::Validator, None);
        req.first_prompt = Some(FirstPrompt::WorkflowSkill {
            step: WorkflowStep::Validate,
            imported: false,
        });
        let built_in = launch(&adapter(), &req).unwrap();
        assert_eq!(built_in.args.last().unwrap(), "validate");
        req.first_prompt = Some(FirstPrompt::WorkflowSkill {
            step: WorkflowStep::Validate,
            imported: true,
        });
        let imported = launch(&adapter(), &req).unwrap();
        assert_eq!(imported.args.last(), Some(&follow_validate()));
        only_the_prompt_differs(&built_in, &imported);
        assert!(!config(&imported).to_string().contains("external_directory"));
    }
}
