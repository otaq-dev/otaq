//! The Claude Code adapter (ADP-01; d0088). It answers the names, detect, input and skills itself; its launch,
//! signals, usage and setup call the existing functions (`session::launch`, `signals::hooks::map_hook`,
//! `usage::transcript`) until the later M20 tickets move that code in here. Its one-time setup is folder trust
//! ([`trust`]).

pub mod trust;

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use self::trust::ClaudeConfig;
use super::{
    AgentTool, BoxFuture, Detection, FirstPrompt, InputQuirks, LaunchError, LaunchPlan,
    LaunchRequest, LaunchRole, SetupEntry, SetupOutcome, SetupReport, SignalContext, SkillDelivery,
    TokenReader, TokenSource, TokenStep, TokensUnknown, Tool, ToolEvent, ToolIdMode,
    TranscriptEntry, WindowsSource,
};
use crate::agent_tools::events::LimitWindow;
use crate::data_dir::DataDir;
use crate::naming;
use crate::recovery::transcript::{find_transcript, read_transcript};
use crate::session::CLEAR_LINE;
use crate::session::launch::{self, AgentCommand, LaunchContext};
use crate::session::statusline;
use crate::signals::background::BackgroundWork;
use crate::signals::hooks::{HELPER_PROMPT_PREFIX, HookEvent, Signal, TurnFacts, map_hook};
use crate::tools::{self, CheckEnv};
use crate::usage::limit::LimitKind;
use crate::usage::plan::RateLimits;
use crate::usage::transcript::{Conversation, Unknown};

/// The status line's 5-hour window, in minutes.
const FIVE_HOUR_MINUTES: u32 = 5 * 60;
/// The status line's weekly window, in minutes.
const SEVEN_DAY_MINUTES: u32 = 7 * 24 * 60;

/// Where Claude Code and Otaq's files are on this machine.
#[derive(Debug, Clone)]
pub struct ClaudeContext {
    /// The program, plugin folder and PATH for launches. `None` when Otaq's plugin wasn't found: every launch
    /// fails with that reason.
    pub launch: Option<LaunchContext>,
    /// The PATH searched for `claude`, and the user's Claude Code folder (saved conversations).
    pub check: CheckEnv,
    /// Otaq's data folder, where the status-line settings files go.
    pub data: DataDir,
    /// Claude Code's config file for folder trust; `None` when it can't be located.
    pub trust: Option<ClaudeConfig>,
}

/// The Claude Code adapter.
#[derive(Debug, Clone)]
pub struct ClaudeCode {
    context: ClaudeContext,
}

impl ClaudeCode {
    pub fn new(context: ClaudeContext) -> Self {
        Self { context }
    }

    fn claude_dir(&self) -> Option<&Path> {
        self.context.check.claude_config_dir.as_deref()
    }

    /// The prompt as typed: `/otaq:epic-lead <message>`, or the text.
    fn prompt_text(&self, prompt: &FirstPrompt) -> String {
        match prompt {
            FirstPrompt::OtaqSkill { name, message } => {
                let skill = self.skill_reference(name);
                match message.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
                    Some(message) => format!("{skill} {message}"),
                    None => skill,
                }
            }
            FirstPrompt::Text(text) => text.clone(),
        }
    }

    /// Otaq's status-line settings file for the session (USG-04). Without it the agent still starts; only the
    /// plan meter misses data.
    fn settings(&self, launch: &LaunchContext, request: &LaunchRequest) -> Option<PathBuf> {
        let data = &self.context.data;
        let path = match (request.role, &request.ticket) {
            (LaunchRole::ProjectAgent | LaunchRole::EpicLead, _) | (_, None) => {
                statusline::project_agent_settings_path(data, &request.project)
            }
            (_, Some(ticket)) => statusline::settings_path(data, ticket),
        };
        statusline::write_settings_at(
            path,
            &request.cwd,
            launch.claude_config_dir.as_deref(),
            &launch.bin_dir,
        )
        .inspect_err(|e| tracing::warn!(project = %request.project, error = %e, "can't write the status-line settings"))
        .ok()
    }

    fn command(
        &self,
        launch: &LaunchContext,
        request: &LaunchRequest,
        prompt: Option<String>,
    ) -> Result<AgentCommand, LaunchError> {
        let settings = self.settings(launch, request);
        let settings = settings.as_deref();
        let cwd = request.cwd.as_path();
        let needs_ticket = || {
            request
                .ticket
                .as_ref()
                .ok_or_else(|| LaunchError(format!("{:?} needs a ticket", request.role)))
        };
        let needs_epic = || {
            request
                .epic
                .as_ref()
                .ok_or_else(|| LaunchError("an epic lead needs its epic".to_owned()))
        };
        let command = match (&request.resume, request.role) {
            (Some(id), LaunchRole::ProjectAgent) => {
                let mut command = launch::resume_project_agent(launch, cwd, id, settings);
                command.args.extend(prompt);
                command
            }
            (Some(id), LaunchRole::EpicLead) => {
                let mut command =
                    launch::resume_epic_lead(launch, cwd, needs_epic()?, id, settings);
                command.args.extend(prompt);
                command
            }
            (Some(id), role) => {
                let mut command = launch::resume_agent_with(
                    launch,
                    needs_ticket()?,
                    cwd,
                    request.port,
                    id,
                    prompt,
                    settings,
                );
                if role == LaunchRole::Validator {
                    validator_env(&mut command);
                }
                command
            }
            (None, role) => {
                let prompt = prompt.ok_or_else(|| {
                    LaunchError("a new conversation needs a first prompt".to_owned())
                })?;
                match role {
                    LaunchRole::ProjectAgent => {
                        with_last_arg(launch::launch_project_agent(launch, cwd, settings), prompt)
                    }
                    LaunchRole::EpicLead => with_last_arg(
                        launch::launch_epic_lead(launch, cwd, needs_epic()?, None, settings),
                        prompt,
                    ),
                    LaunchRole::Validator => {
                        launch::launch_validator(launch, needs_ticket()?, cwd, &prompt, settings)
                    }
                    LaunchRole::TicketAgent | LaunchRole::Answer | LaunchRole::Delivery => {
                        launch::launch_with_prompt(
                            launch,
                            needs_ticket()?,
                            cwd,
                            request.port,
                            prompt,
                            settings,
                        )
                    }
                }
            }
        };
        Ok(command)
    }
}

/// `command` with its last argument (the first prompt) replaced.
fn with_last_arg(mut command: AgentCommand, prompt: String) -> AgentCommand {
    if let Some(last) = command.args.last_mut() {
        *last = prompt;
    }
    command
}

/// A validator's environment: no `PORT`, the validator role (as [`launch::launch_validator`]).
fn validator_env(command: &mut AgentCommand) {
    command.env.retain(|(name, _)| name != "PORT");
    command
        .env
        .push((naming::role_env_var(), naming::ROLE_VALIDATOR.to_owned()));
}

impl AgentTool for ClaudeCode {
    fn tool(&self) -> Tool {
        Tool::Claude
    }

    fn display_name(&self) -> &'static str {
        "Claude Code"
    }

    fn short_name(&self) -> &'static str {
        "Claude"
    }

    fn command_name(&self) -> &'static str {
        tools::Tool::Claude.command()
    }

    fn detect(&self) -> BoxFuture<'_, Detection> {
        Box::pin(async move {
            let status = tools::check_one(tools::Tool::Claude, &self.context.check).await;
            Detection {
                installed: status.found,
                signed_in: status.logged_in,
                version: status.version,
            }
        })
    }

    fn launch(&self, request: &LaunchRequest) -> Result<LaunchPlan, LaunchError> {
        let Some(launch) = &self.context.launch else {
            return Err(LaunchError(format!(
                "{}'s Claude Code plugin wasn't found next to the {} command",
                naming::PRODUCT_NAME,
                naming::COMMAND_NAME
            )));
        };
        let prompt = request.first_prompt.as_ref().map(|p| self.prompt_text(p));
        let has_prompt = prompt.is_some();
        let mut command = self.command(launch, request, prompt)?;
        if let Some(model) = &request.model {
            // Before the prompt, which is always last.
            let at = command.args.len() - usize::from(has_prompt);
            command
                .args
                .splice(at..at, ["--model".to_owned(), model.clone()]);
        }
        Ok(LaunchPlan {
            program: command.program,
            args: command.args,
            cwd: command.cwd,
            env: command.env,
            tool_id: ToolIdMode::ChosenUpFront(command.claude_session_id),
        })
    }

    fn can_resume(&self, tool_id: &str, cwd: &Path) -> bool {
        self.claude_dir()
            .and_then(|dir| find_transcript(dir, cwd, tool_id))
            .is_some()
    }

    fn resume_hint(&self, tool_id: &str) -> String {
        format!("{} --resume {tool_id}", self.command_name())
    }

    /// Folder trust in Claude Code's config file ([`trust`]); `None` when the file couldn't be located.
    fn record_project(&self, repo: &Path) -> Option<SetupOutcome> {
        Some(self.context.trust.as_ref()?.record_trust(repo))
    }

    fn set_up_tool(&self) -> BoxFuture<'_, SetupReport> {
        Box::pin(std::future::ready(SetupReport::default()))
    }

    fn setup_entries(&self) -> Vec<SetupEntry> {
        vec![SetupEntry {
            text: "trust for your projects".to_owned(),
            removed_on_disconnect: false,
        }]
    }

    /// Claude Code has no Otaq-only entries: its hooks, pre-approval and skills come with every launch, and
    /// folder trust stays when a tool is disconnected (d0086).
    fn remove_setup(&self) -> SetupReport {
        SetupReport::default()
    }

    fn map_hook(
        &self,
        event: &str,
        payload: &Value,
        _context: SignalContext<'_>,
    ) -> Vec<ToolEvent> {
        let Ok(hook) = event.parse::<HookEvent>() else {
            tracing::debug!(event, %payload, "unrecognised Claude Code hook");
            return vec![ToolEvent::Unrecognised {
                event: event.to_owned(),
            }];
        };
        // Otaq's own rule (a submit this turn) is applied in `signals/`; here every turn end is reported.
        let event = match map_hook(hook, payload, TurnFacts::default()) {
            Signal::NeedsYou { prompt } => match prompt.strip_prefix(HELPER_PROMPT_PREFIX) {
                Some(prompt) => ToolEvent::NeedsYou {
                    prompt: prompt.to_owned(),
                    helper_agent: true,
                },
                None => ToolEvent::NeedsYou {
                    prompt,
                    helper_agent: false,
                },
            },
            Signal::WorkingInBackground { work } => turn_ended(work, payload),
            Signal::Idle { .. } | Signal::TurnEnded => {
                turn_ended(BackgroundWork::default(), payload)
            }
            Signal::Working => ToolEvent::NewTurn,
            Signal::LimitReached { hit } => ToolEvent::LimitReached {
                hit,
                resets_at: None,
            },
            Signal::Failed { error, message } => ToolEvent::Failed { error, message },
            Signal::Ignored => return Vec::new(),
        };
        vec![event]
    }

    fn watched_file(&self, _tool_id: &str, _cwd: &Path) -> Option<PathBuf> {
        None
    }

    fn map_watched_lines(&self, _lines: &str, _context: SignalContext<'_>) -> Vec<ToolEvent> {
        Vec::new()
    }

    fn token_source(&self, tool_id: &str, cwd: &Path) -> TokenSource {
        TokenSource::Reader(Box::new(Transcript {
            claude_dir: self.claude_dir().map(Path::to_path_buf),
            cwd: cwd.to_path_buf(),
            tool_id: tool_id.to_owned(),
            path: None,
            conversation: Conversation::default(),
        }))
    }

    fn last_entries(
        &self,
        tool_id: &str,
        cwd: &Path,
        limit: usize,
    ) -> Option<Vec<TranscriptEntry>> {
        find_transcript(self.claude_dir()?, cwd, tool_id)
            .and_then(|path| read_transcript(&path, limit))
    }

    fn windows_source(&self) -> WindowsSource {
        WindowsSource::Report
    }

    fn windows_from_report(&self, report: &Value) -> Option<Vec<LimitWindow>> {
        let limits = RateLimits::from_status_line(report)?;
        let window = |key: &str, minutes: u32, w: Option<crate::usage::plan::Window>| {
            w.map(|w| LimitWindow {
                key: key.to_owned(),
                length_minutes: Some(minutes),
                used_percent: w.used_percentage,
                resets_at: Some(w.resets_at),
                credits: None,
            })
        };
        Some(
            [
                window("five_hour", FIVE_HOUR_MINUTES, limits.five_hour),
                window("seven_day", SEVEN_DAY_MINUTES, limits.seven_day),
            ]
            .into_iter()
            .flatten()
            .collect(),
        )
    }

    fn limit_window_key(&self, kind: LimitKind) -> Option<&'static str> {
        match kind {
            LimitKind::Session => Some("five_hour"),
            LimitKind::Weekly => Some("seven_day"),
            LimitKind::Other => None,
        }
    }

    fn input(&self) -> InputQuirks {
        InputQuirks {
            clear_line: CLEAR_LINE,
            enter_delay: Duration::ZERO,
            multi_line_clearable: true,
        }
    }

    fn skill_reference(&self, skill: &str) -> String {
        naming::skill(skill)
    }

    fn skill_delivery(&self) -> SkillDelivery {
        SkillDelivery::PluginFolderAtLaunch
    }
}

/// A `Stop`'s turn end, with the whole last message.
fn turn_ended(background: BackgroundWork, payload: &Value) -> ToolEvent {
    ToolEvent::TurnEnded {
        background,
        last_message: payload
            .get("last_assistant_message")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .map(str::to_owned),
    }
}

/// One Claude conversation's token counts, from its transcript and its helper agents' files.
struct Transcript {
    claude_dir: Option<PathBuf>,
    cwd: PathBuf,
    tool_id: String,
    /// The transcript, once found.
    path: Option<PathBuf>,
    conversation: Conversation,
}

impl TokenReader for Transcript {
    fn read(&mut self) -> Result<bool, TokensUnknown> {
        let Some(claude_dir) = &self.claude_dir else {
            return Err(TokensUnknown::Missing);
        };
        if self.path.as_ref().is_none_or(|p| !p.is_file()) {
            self.path = find_transcript(claude_dir, &self.cwd, &self.tool_id);
        }
        let Some(path) = &self.path else {
            return Err(TokensUnknown::Missing);
        };
        self.conversation.read(path).map_err(|e| match e {
            Unknown::Missing => TokensUnknown::Missing,
            Unknown::Unreadable(e) => TokensUnknown::Unreadable(e),
            Unknown::Unrecognised => TokensUnknown::Unrecognised,
        })
    }

    fn steps(&self) -> Vec<TokenStep> {
        self.conversation
            .messages()
            .map(|m| TokenStep {
                id: m.id.clone(),
                at: m.at,
                model: m.model.clone(),
                tokens: m.tokens,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use serde_json::json;

    use super::*;
    use crate::agent_tools::Adapters;
    use crate::domain::TicketKey;

    struct Fixture {
        _data: tempfile::TempDir,
        adapter: ClaudeCode,
    }

    fn launch_context() -> LaunchContext {
        LaunchContext {
            claude: vec!["claude".into()],
            bin_dir: "/opt/cb/bin".into(),
            plugin_dir: "/opt/cb/plugin".into(),
            path: std::env::join_paths(["/usr/bin", "/bin"]).unwrap(),
            claude_config_dir: None,
        }
    }

    fn fixture() -> Fixture {
        let data = tempfile::tempdir().unwrap();
        let adapter = ClaudeCode::new(ClaudeContext {
            launch: Some(launch_context()),
            check: CheckEnv {
                path: OsString::new(),
                claude_config_dir: None,
                managed_settings_dir: data.path().join("managed"),
            },
            data: DataDir::new(data.path()),
            trust: None,
        });
        Fixture {
            _data: data,
            adapter,
        }
    }

    fn key() -> TicketKey {
        "SHOP-18".parse().unwrap()
    }

    fn request(role: LaunchRole) -> LaunchRequest {
        LaunchRequest {
            role,
            cwd: "/wt/shop-web/SHOP-18".into(),
            project: "SHOP".into(),
            ticket: Some(key()),
            epic: None,
            port: 4012,
            first_prompt: Some(FirstPrompt::OtaqSkill {
                name: launch::WORK_TICKET_SKILL.into(),
                message: None,
            }),
            resume: None,
            model: None,
        }
    }

    fn chosen(plan: &LaunchPlan) -> &str {
        match &plan.tool_id {
            ToolIdMode::ChosenUpFront(id) => id,
            other => panic!("not chosen up front: {other:?}"),
        }
    }

    #[test]
    fn names_keep_todays_wording() {
        let f = fixture();
        assert_eq!(f.adapter.tool(), Tool::Claude);
        assert_eq!(f.adapter.display_name(), "Claude Code");
        assert_eq!(f.adapter.short_name(), "Claude");
        assert_eq!(f.adapter.command_name(), "claude");
        assert_eq!(
            f.adapter.resume_hint("3be94c10"),
            "claude --resume 3be94c10"
        );
    }

    #[test]
    fn input_quirks() {
        let quirks = fixture().adapter.input();
        assert_eq!(quirks.clear_line, b"\x05\x15");
        assert_eq!(quirks.enter_delay, Duration::ZERO);
        assert!(quirks.multi_line_clearable);
    }

    #[test]
    fn skills() {
        let f = fixture();
        let name = naming::PLUGIN_NAMESPACE;
        assert_eq!(
            f.adapter.skill_reference("commit"),
            format!("/{name}:commit")
        );
        assert_eq!(
            f.adapter.skill_delivery(),
            SkillDelivery::PluginFolderAtLaunch
        );
    }

    #[test]
    fn the_tool_id_is_chosen_up_front_a_fresh_uuid_v7_each_time() {
        let f = fixture();
        let first = f.adapter.launch(&request(LaunchRole::TicketAgent)).unwrap();
        let second = f.adapter.launch(&request(LaunchRole::TicketAgent)).unwrap();
        let (a, b) = (chosen(&first), chosen(&second));
        assert_ne!(a, b);
        for id in [a, b] {
            assert_eq!(uuid::Uuid::parse_str(id).unwrap().get_version_num(), 7);
        }
        let mut resume = request(LaunchRole::TicketAgent);
        resume.resume = Some("3be94c10-0000-7000-8000-000000000000".into());
        resume.first_prompt = None;
        let resumed = f.adapter.launch(&resume).unwrap();
        assert_eq!(chosen(&resumed), "3be94c10-0000-7000-8000-000000000000");
    }

    /// The ticket agent's launch is today's `launch_agent`, with the status-line file written into the data
    /// folder.
    #[test]
    fn a_ticket_agent_launches_as_today() {
        let f = fixture();
        let plan = f.adapter.launch(&request(LaunchRole::TicketAgent)).unwrap();
        let settings = statusline::settings_path(&f.adapter.context.data, &key());
        assert!(settings.is_file());
        let today = launch::launch_agent(
            &launch_context(),
            &key(),
            Path::new("/wt/shop-web/SHOP-18"),
            4012,
            Some(&settings),
        );
        assert_eq!(plan.program, today.program);
        assert_eq!(plan.cwd, today.cwd);
        assert_eq!(plan.env, today.env);
        let mut expected = today.args.clone();
        let id_at = expected.len() - 2;
        expected[id_at] = chosen(&plan).to_owned();
        assert_eq!(plan.args, expected);
    }

    #[test]
    fn a_model_goes_before_the_prompt() {
        let f = fixture();
        let mut req = request(LaunchRole::TicketAgent);
        req.model = Some("opus".into());
        let plan = f.adapter.launch(&req).unwrap();
        let n = plan.args.len();
        assert_eq!(
            plan.args[n - 3..],
            [
                "--model".to_owned(),
                "opus".to_owned(),
                naming::skill(launch::WORK_TICKET_SKILL)
            ]
        );
    }

    #[test]
    fn without_the_plugin_a_launch_fails_with_todays_reason() {
        let mut f = fixture();
        f.adapter.context.launch = None;
        let error = f
            .adapter
            .launch(&request(LaunchRole::TicketAgent))
            .unwrap_err();
        assert!(
            error.0.contains("Claude Code plugin wasn't found"),
            "{error}"
        );
    }

    #[test]
    fn hooks_become_otaqs_events() {
        let f = fixture();
        let map = |event: &str, payload: Value| {
            f.adapter
                .map_hook(event, &payload, SignalContext::default())
        };
        assert_eq!(
            map(
                "Notification",
                json!({"notification_type": "permission_prompt", "message": "Run command: ls", "agent_id": "a1"})
            ),
            [ToolEvent::NeedsYou {
                prompt: "Run command: ls".into(),
                helper_agent: true
            }]
        );
        assert_eq!(
            map("Notification", json!({"notification_type": "idle_prompt"})),
            []
        );
        assert_eq!(
            map(
                "Stop",
                json!({"last_assistant_message": "Done.\n- all green"})
            ),
            [ToolEvent::TurnEnded {
                background: BackgroundWork::default(),
                last_message: Some("Done.\n- all green".into())
            }]
        );
        assert_eq!(
            map("UserPromptSubmit", json!({"prompt": "hi"})),
            [ToolEvent::NewTurn]
        );
        assert_eq!(
            map("SessionStart", json!({})),
            [ToolEvent::Unrecognised {
                event: "SessionStart".into()
            }]
        );
    }

    #[test]
    fn status_line_windows() {
        let f = fixture();
        let windows = f
            .adapter
            .windows_from_report(&json!({"rate_limits": {
                "five_hour": {"used_percentage": 82.0, "resets_at": 1_760_000_000},
                "seven_day": {"used_percentage": 40.5, "resets_at": 1_760_500_000}
            }}))
            .unwrap();
        assert_eq!(
            windows,
            [
                LimitWindow {
                    key: "five_hour".into(),
                    length_minutes: Some(300),
                    used_percent: 82.0,
                    resets_at: Some(1_760_000_000_000),
                    credits: None
                },
                LimitWindow {
                    key: "seven_day".into(),
                    length_minutes: Some(10_080),
                    used_percent: 40.5,
                    resets_at: Some(1_760_500_000_000),
                    credits: None
                },
            ]
        );
        assert_eq!(f.adapter.windows_from_report(&json!({"model": {}})), None);
        assert_eq!(
            f.adapter.limit_window_key(LimitKind::Weekly),
            Some("seven_day")
        );
    }

    /// Claude Code has no Otaq-only entries: removal removes nothing (d0086).
    #[test]
    fn setup_entries_and_removal() {
        let f = fixture();
        assert_eq!(
            f.adapter.setup_entries(),
            [SetupEntry {
                text: "trust for your projects".into(),
                removed_on_disconnect: false
            }]
        );
        assert_eq!(f.adapter.remove_setup(), SetupReport::default());
        assert_eq!(f.adapter.record_project(Path::new("/repo")), None);
    }

    /// With a (temporary) config file, the adapter records the trust; removal leaves the file as it is.
    #[test]
    fn the_setup_records_the_trust_and_removal_touches_no_file() {
        let claude = tempfile::tempdir().unwrap();
        let config = ClaudeConfig::locate(Some(claude.path().into()), None).unwrap();
        let mut f = fixture();
        f.adapter.context.trust = Some(config.clone());
        let repo = Path::new("/Users/me/pocket-notes");

        assert_eq!(f.adapter.record_project(repo), Some(SetupOutcome::Recorded));
        assert_eq!(config.is_trusted(&trust::trust_key(repo)), Ok(true));
        assert_eq!(
            f.adapter.record_project(repo),
            Some(SetupOutcome::AlreadyTrusted)
        );

        let bytes = std::fs::read(&config.file).unwrap();
        let modified = std::fs::metadata(&config.file).unwrap().modified().unwrap();
        assert_eq!(f.adapter.remove_setup(), SetupReport::default());
        assert_eq!(std::fs::read(&config.file).unwrap(), bytes);
        assert_eq!(
            std::fs::metadata(&config.file).unwrap().modified().unwrap(),
            modified
        );
        assert_eq!(std::fs::read_dir(claude.path()).unwrap().count(), 1);
    }

    #[test]
    fn without_a_claude_folder_tokens_are_unknown() {
        let f = fixture();
        let TokenSource::Reader(mut reader) = f.adapter.token_source("id", Path::new("/wt")) else {
            panic!("Claude Code's tokens come from its transcript");
        };
        assert_eq!(reader.read(), Err(TokensUnknown::Missing));
        assert!(reader.steps().is_empty());
        assert!(!f.adapter.can_resume("id", Path::new("/wt")));
    }

    #[tokio::test]
    async fn detect_without_claude_on_path() {
        let detection = fixture().adapter.detect().await;
        assert_eq!(detection, Detection::default());
    }

    #[test]
    fn the_registry_returns_the_claude_adapter() {
        let adapters = Adapters::new(fixture().adapter);
        let copy = adapters.clone();
        assert_eq!(copy.get(Tool::Claude).tool(), Tool::Claude);
        assert_eq!(adapters.get(Tool::Claude).display_name(), "Claude Code");
    }
}
