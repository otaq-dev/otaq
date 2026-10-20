//! The first start of M32 (AGM-02; d0108, user decisions 2026-10-20, no preview until M33): Otaq applies its agents
//! file (Planner, Epic lead) and the default flow's `agents:` (Developer, Validator) directly. On an install that
//! already has data, the same step moves M31's settings over, once, in one transaction:
//!
//! - the default model ← the app's Developer default's model, else the first named agent's tool with its default
//!   model (none at all on a fresh install: the first connected tool sets it, [`ensure_default_model`]);
//! - each role default's model → the matching agent (Developer, Validator, Epic lead, Planner);
//! - the global Agent instructions → Developer's instructions; the Project agent instructions → Planner's;
//! - the global imported Commit, Push and Open PR skills → Developer's skills in place of the shipped ones (Commit
//!   replaces `commit`; Push and Open PR are added), Validate → Validator's (in place of `validate`);
//! - every named agent → an agent with its id, name and model and Developer's instructions and skills (a name that
//!   is taken or not allowed any more gets a number); a named agent's fallback → nothing (fallback is per tool);
//! - **dropped**: per-ticket Developer / Validator / Epic lead rows, per-project role defaults, per-project workflow
//!   skills and Project agent instructions, recorded ([`Dropped`]) for the notice on Settings → Agents;
//! - **kept**: each project's Agent instructions, as its notes (`project_notes`), and its Validation settings.
//!
//! Then `named_agents` and `ticket_agent_rows` go. The record is the marker: a later start finds it and changes
//! nothing. A fresh install goes the same way with nothing to carry and nothing to drop.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{Sqlite, Transaction};

use super::store::NewAgent;
use super::{AgentModel, CameFrom, ImportedSkill, ModelRef, SkillRef, checked_name, shipped};
use crate::agent_tools::Tool;
use crate::domain::{ProjectKey, TicketKey};
use crate::flow;
use crate::store::settings::{AGENT_FLOW_NAMES, DEFAULT_MODEL, MOVED_OVER};
use crate::store::{Store, StoreError, now_ms};

/// What the move-over dropped (AGM-02: "Otaq says once what it dropped, naming the projects"), shown on Settings →
/// Agents until dismissed. Stored in `settings` (`agents.moved_over`); its presence means the move-over ran.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dropped {
    /// When it ran, in Unix milliseconds.
    pub at: i64,
    /// The projects that had their own role defaults, workflow skills or Project agent instructions, by key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projects: Vec<DroppedProject>,
    /// The tickets' rows, by key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tickets: Vec<DroppedRow>,
    /// What was carried over to agents ("Moved over: Strong, Light, your Agent instructions (now Developer's)…").
    #[serde(default)]
    pub moved: Moved,
    /// The user dismissed the notice.
    #[serde(default)]
    pub dismissed: bool,
}

impl Dropped {
    /// Nothing was dropped (a fresh install, or an install without overrides and rows).
    pub fn is_empty(&self) -> bool {
        self.projects.is_empty() && self.tickets.is_empty()
    }

    /// Nothing to tell: nothing moved over and nothing dropped (a fresh install).
    pub fn says_nothing(&self) -> bool {
        self.is_empty() && self.moved == Moved::default()
    }
}

/// What the move-over carried to agents (AGM-02), for the notice and the agents' came-from lines while it shows.
/// Agents by id, so a rename since reads right.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moved {
    /// The agents made from M31's named agents, in their order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub named: Vec<String>,
    /// The agent that got the global Agent instructions (Developer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_instructions: Option<String>,
    /// The agent that got the Project agent instructions (Planner).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_agent_instructions: Option<String>,
    /// The global imported workflow skills, each with the agent that got it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<MovedSkill>,
}

/// A global imported workflow skill now on an agent: "your Validate skill (now Validator's)".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovedSkill {
    /// The step as people read it ("Validate").
    pub step: String,
    pub agent: String,
}

/// One project's dropped overrides: "pocket-api: its own Developer (Strong) and Commit skill".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DroppedProject {
    pub key: String,
    pub name: String,
    /// Its own role defaults: the role as people read it ("Developer") and the named agent's name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<DroppedRole>,
    /// Its own imported workflow skills: the step ("Commit") and the skill's `name:`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<DroppedSkill>,
    /// It had its own Project agent instructions.
    #[serde(default)]
    pub project_agent_instructions: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DroppedRole {
    pub role: String,
    pub agent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DroppedSkill {
    pub step: String,
    pub name: String,
}

/// One ticket's dropped row: "NOTE-3's Developer row (Strong, set by you)".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DroppedRow {
    pub key: TicketKey,
    /// "Developer", "Validator", "Epic lead".
    pub role: String,
    /// The named agent's name; `None` when it had been deleted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// Set by the user; else picked by `picked_by` for `reason`.
    pub set_by_user: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picked_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// M31's settings and tables the move-over reads (the names they had then).
mod m31 {
    /// The app-wide role defaults (`settings`) and a project's overrides (its settings' `agents`).
    pub const ROLE_DEFAULTS: &str = "agents.defaults";
    pub const PROJECT_ROLE_DEFAULTS: &str = "agents";
    pub const AUTO_FALLBACK: &str = "agents.auto_fallback";
    pub const AGENT_INSTRUCTIONS: &str = "agent_instructions";
    pub const PROJECT_AGENT_INSTRUCTIONS: &str = "project_agent_instructions";
    pub const WORKFLOW_SKILLS: &str = "workflow_skills";
    /// The four steps: settings field, label, the shipped skill the step's built-in is.
    pub const STEPS: [(&str, &str, &str); 4] = [
        ("commit", "Commit", "commit"),
        ("push", "Push", "push"),
        ("open_pr", "Open PR", "open-pr"),
        ("validate", "Validate", "validate"),
    ];
    /// The role defaults' fields and the rows' roles, with the label.
    pub const ROLES: [(&str, &str, &str); 4] = [
        ("developer", "developer", "Developer"),
        ("validator", "validator", "Validator"),
        ("epic_lead", "lead", "Epic lead"),
        ("project_agent", "project_agent", "Project agent"),
    ];
}

/// A named agent of M31.
#[derive(Debug, Clone, sqlx::FromRow)]
struct NamedAgent {
    id: String,
    name: String,
    tool: String,
    model: Option<String>,
}

/// An imported workflow skill as M31's settings stored it.
#[derive(Debug, Clone, Deserialize)]
struct StoredImport {
    id: String,
    name: String,
    original: String,
    imported_at: i64,
    fingerprint: String,
    #[serde(default)]
    warnings: Vec<String>,
}

/// Runs the move-over unless it ran already (AGM-02). Whether it ran now.
pub async fn run(store: &Store) -> Result<bool, StoreError> {
    if store.setting(MOVED_OVER).await?.is_some() {
        return Ok(false);
    }
    let mut tx = store.begin_write().await?;
    let dropped = move_over(&mut tx).await?;
    tx.commit().await?;
    tracing::info!(
        projects = dropped.projects.len(),
        tickets = dropped.tickets.len(),
        "agents applied (AGM-02)"
    );
    Ok(true)
}

async fn table_exists(tx: &mut Transaction<'_, Sqlite>, name: &str) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?)",
    )
    .bind(name)
    .fetch_one(&mut **tx)
    .await?)
}

async fn setting_in(
    tx: &mut Transaction<'_, Sqlite>,
    key: &str,
) -> Result<Option<Value>, StoreError> {
    let json: Option<String> = sqlx::query_scalar("SELECT value_json FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(&mut **tx)
        .await?;
    Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
}

async fn put_setting_in(
    tx: &mut Transaction<'_, Sqlite>,
    key: &str,
    value_json: &str,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO settings (key, value_json, updated_at) VALUES (?, ?, ?)
         ON CONFLICT (key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value_json)
    .bind(now_ms())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// A text setting (`"Run just test"`), trimmed; empty when missing.
fn text_of(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_owned()
}

/// The imported skill a workflow skill value holds, if it holds one (a text is the built-in or a typed name of
/// before d0092, never used to run anything).
fn import_of(value: Option<&Value>) -> Option<StoredImport> {
    value
        .filter(|v| v.is_object())
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

fn imported(stored: StoredImport, stands_for: &str) -> ImportedSkill {
    ImportedSkill {
        id: stored.id,
        name: stored.name,
        original: stored.original,
        imported_at: stored.imported_at,
        fingerprint: stored.fingerprint,
        warnings: stored.warnings,
        stands_for: Some(stands_for.to_owned()),
    }
}

/// A shipped agent with `skill` in place of the shipped skill it stands for (or added when the agent doesn't have
/// that one).
fn put_skill(agent: &mut NewAgent, skill: ImportedSkill) {
    let stands = skill.stands_for.clone().unwrap_or_default();
    let new = SkillRef::Imported(skill.id.clone());
    match agent
        .skills
        .iter()
        .position(|s| *s == SkillRef::Shipped(stands.clone()))
    {
        Some(at) => agent.skills[at] = new,
        None => agent.skills.push(new),
    }
    agent.imported.push(skill);
}

async fn move_over(tx: &mut Transaction<'_, Sqlite>) -> Result<Dropped, StoreError> {
    let had_named = table_exists(tx, "named_agents").await?;
    let named: Vec<NamedAgent> = if had_named {
        sqlx::query_as("SELECT id, name, tool, model FROM named_agents ORDER BY created_at, id")
            .fetch_all(&mut **tx)
            .await?
    } else {
        Vec::new()
    };
    let model_of = |id: Option<&str>| -> Option<ModelRef> {
        let agent = named.iter().find(|a| Some(a.id.as_str()) == id)?;
        Some(ModelRef {
            tool: agent.tool.parse().ok()?,
            model: agent.model.clone(),
        })
    };
    let defaults = setting_in(tx, m31::ROLE_DEFAULTS)
        .await?
        .unwrap_or(Value::Null);
    let default_of = |field: &str| model_of(defaults.get(field).and_then(Value::as_str));
    let instructions = text_of(setting_in(tx, m31::AGENT_INSTRUCTIONS).await?.as_ref());
    let project_agent_instructions = text_of(
        setting_in(tx, m31::PROJECT_AGENT_INSTRUCTIONS)
            .await?
            .as_ref(),
    );
    let skills = setting_in(tx, m31::WORKFLOW_SKILLS)
        .await?
        .unwrap_or(Value::Null);

    // The shipped agents, with what M31's settings say.
    let as_model = |m: Option<ModelRef>| {
        m.map_or(AgentModel::Default, |m| AgentModel::Tool {
            tool: m.tool,
            model: m.model,
        })
    };
    let flow_agents = &flow::default_file().agents;
    let mut built: Vec<(NewAgent, CameFrom, Option<String>)> = Vec::new();
    // What went where, by the built agent's place: the notice names them once written (ids).
    let mut instructions_to = None;
    let mut planner_instructions_to = None;
    let mut skills_to: Vec<(String, usize)> = Vec::new();
    for agent in shipped::agents_file() {
        let mut new = shipped::new_agent(agent);
        match new.role {
            Some(super::Role::ProjectAgent) => {
                new.model = as_model(default_of("project_agent"));
                new.instructions = project_agent_instructions.clone();
                if !project_agent_instructions.is_empty() {
                    planner_instructions_to = Some(built.len());
                }
            }
            Some(super::Role::EpicLead) => new.model = as_model(default_of("epic_lead")),
            None => {}
        }
        built.push((new, CameFrom::AgentsFile, None));
    }
    let mut developer_index = None;
    for agent in flow_agents {
        let mut new = shipped::new_agent(agent);
        // The default flow's agents by the stage they run (Developer: Dev and Delivery; Validator: Validate).
        let runs = |id: &str| {
            flow::default_flow()
                .stage(id)
                .and_then(|s| s.agent.clone())
                .is_some_and(|a| a == agent.name)
        };
        if runs(flow::default::DEV) {
            new.model = as_model(default_of("developer"));
            new.instructions = instructions.clone();
            if !instructions.is_empty() {
                instructions_to = Some(built.len());
            }
            for (field, label, stands) in &m31::STEPS[..3] {
                if let Some(skill) = import_of(skills.get(field)) {
                    put_skill(&mut new, imported(skill, stands));
                    skills_to.push(((*label).to_owned(), built.len()));
                }
            }
            developer_index = Some(built.len());
        } else if runs(flow::default::VALIDATE) {
            new.model = as_model(default_of("validator"));
            let (field, label, stands) = m31::STEPS[3];
            if let Some(skill) = import_of(skills.get(field)) {
                put_skill(&mut new, imported(skill, stands));
                skills_to.push((label.to_owned(), built.len()));
            }
        }
        built.push((new, CameFrom::DefaultPreset, None));
    }

    // Every named agent: its id, name and model, Developer's instructions and skills.
    let developer = developer_index.map(|i| built[i].0.clone());
    for agent in &named {
        let Ok(tool) = agent.tool.parse::<Tool>() else {
            tracing::warn!(
                id = agent.id,
                tool = agent.tool,
                "a named agent of an unknown tool isn't moved over"
            );
            continue;
        };
        let name = free_name(&agent.name, built.iter().map(|(a, ..)| a.name.as_str()));
        let mut new = developer.clone().unwrap_or(NewAgent {
            name: String::new(),
            instructions: String::new(),
            skills: Vec::new(),
            imported: Vec::new(),
            model: AgentModel::Default,
            effort: None,
            role: None,
        });
        new.name = name;
        new.model = AgentModel::Tool {
            tool,
            model: agent.model.clone(),
        };
        new.role = None;
        built.push((new, CameFrom::You, Some(agent.id.clone())));
    }

    // Write them, and the names the default flow's stages run.
    let mut flow_names = HashMap::new();
    let mut ids = Vec::new();
    for (new, came_from, id) in &built {
        let id = Store::insert_agent_in(tx, new, came_from, id.as_deref()).await?;
        if *came_from == CameFrom::DefaultPreset {
            flow_names.insert(new.name.clone(), id.clone());
        }
        ids.push(id);
    }
    let moved = Moved {
        named: built
            .iter()
            .zip(&ids)
            .filter(|((.., from), _)| from.is_some())
            .map(|(_, id)| id.clone())
            .collect(),
        agent_instructions: instructions_to.map(|i| ids[i].clone()),
        project_agent_instructions: planner_instructions_to.map(|i| ids[i].clone()),
        skills: skills_to
            .into_iter()
            .map(|(step, i)| MovedSkill {
                step,
                agent: ids[i].clone(),
            })
            .collect(),
    };
    put_setting_in(
        tx,
        AGENT_FLOW_NAMES,
        &serde_json::to_string(&flow_names).expect("names serialize"),
    )
    .await?;
    let default_model = default_of("developer").or_else(|| {
        named.iter().find_map(|a| {
            Some(ModelRef {
                tool: a.tool.parse().ok()?,
                model: None,
            })
        })
    });
    if let Some(model) = default_model {
        put_setting_in(
            tx,
            DEFAULT_MODEL,
            &serde_json::to_string(&model).expect("a model serializes"),
        )
        .await?;
    }

    // What goes: the projects' overrides (their notes stay), the rows, the old settings and tables.
    let names: HashMap<&str, &str> = named
        .iter()
        .map(|a| (a.id.as_str(), a.name.as_str()))
        .collect();
    let mut dropped = Dropped {
        at: now_ms(),
        moved,
        ..Dropped::default()
    };
    let projects: Vec<(String, String, String, String)> =
        sqlx::query_as("SELECT id, key, name, settings_json FROM projects ORDER BY key")
            .fetch_all(&mut **tx)
            .await?;
    for (id, key, name, json) in projects {
        let mut settings: Map<String, Value> = serde_json::from_str(&json).unwrap_or_default();
        let mut project = DroppedProject {
            key,
            name,
            roles: Vec::new(),
            skills: Vec::new(),
            project_agent_instructions: false,
        };
        if let Some(own) = settings.remove(m31::PROJECT_ROLE_DEFAULTS) {
            for (field, _, label) in m31::ROLES {
                if let Some(agent) = own.get(field).and_then(Value::as_str) {
                    project.roles.push(DroppedRole {
                        role: label.to_owned(),
                        agent: names.get(agent).copied().unwrap_or(agent).to_owned(),
                    });
                }
            }
        }
        if let Some(own) = settings.remove(m31::WORKFLOW_SKILLS) {
            for (field, label, _) in m31::STEPS {
                if let Some(skill) = import_of(own.get(field)) {
                    project.skills.push(DroppedSkill {
                        step: label.to_owned(),
                        name: skill.name,
                    });
                }
            }
        }
        if let Some(own) = settings.remove(m31::PROJECT_AGENT_INSTRUCTIONS) {
            project.project_agent_instructions = !text_of(Some(&own)).is_empty();
        }
        if let Some(notes) = settings.remove(m31::AGENT_INSTRUCTIONS) {
            settings.insert(crate::board::settings::PROJECT_NOTES.to_owned(), notes);
        }
        sqlx::query("UPDATE projects SET settings_json = ? WHERE id = ?")
            .bind(Value::Object(settings).to_string())
            .bind(&id)
            .execute(&mut **tx)
            .await?;
        if !project.roles.is_empty()
            || !project.skills.is_empty()
            || project.project_agent_instructions
        {
            dropped.projects.push(project);
        }
    }
    if table_exists(tx, "ticket_agent_rows").await? {
        type Row = (
            String,
            i64,
            i64,
            String,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
        );
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT p.key, t.number, t.child_number, r.role, r.named_agent_id, r.set_by, r.picked_by, r.reason
             FROM ticket_agent_rows r JOIN tickets t ON t.id = r.ticket_id JOIN projects p ON p.id = t.project_id
             ORDER BY p.key, t.number, t.child_number, r.role",
        )
        .fetch_all(&mut **tx)
        .await?;
        for (project, number, child, role, agent, set_by, picked_by, reason) in rows {
            let Some(key) = ticket_key(&project, number, child) else {
                continue;
            };
            let label = m31::ROLES
                .iter()
                .find(|(_, stored, _)| *stored == role)
                .map_or(role.as_str(), |(.., label)| label);
            let user = set_by == "user";
            dropped.tickets.push(DroppedRow {
                key,
                role: label.to_owned(),
                agent: agent.map(|id| names.get(id.as_str()).copied().unwrap_or(&id).to_owned()),
                set_by_user: user,
                picked_by: picked_by.filter(|_| !user),
                reason: reason.filter(|_| !user),
            });
        }
        sqlx::query("DROP TABLE ticket_agent_rows")
            .execute(&mut **tx)
            .await?;
    }
    if had_named {
        sqlx::query("DROP TABLE named_agents")
            .execute(&mut **tx)
            .await?;
    }
    for key in [
        m31::ROLE_DEFAULTS,
        m31::AUTO_FALLBACK,
        m31::AGENT_INSTRUCTIONS,
        m31::PROJECT_AGENT_INSTRUCTIONS,
        m31::WORKFLOW_SKILLS,
    ] {
        sqlx::query("DELETE FROM settings WHERE key = ?")
            .bind(key)
            .execute(&mut **tx)
            .await?;
    }
    put_setting_in(
        tx,
        MOVED_OVER,
        &serde_json::to_string(&dropped).expect("the record serializes"),
    )
    .await?;
    Ok(dropped)
}

fn ticket_key(project: &str, number: i64, child: i64) -> Option<TicketKey> {
    let project = ProjectKey::new(project).ok()?;
    let number = u32::try_from(number).ok()?;
    Some(match u32::try_from(child).ok().filter(|c| *c > 0) {
        Some(child) => TicketKey::child(project, number, child),
        None => TicketKey::new(project, number),
    })
}

/// A named agent's name as an agent's: kept when the rules allow it, else made free ("Developer 2", `_x` → "x").
fn free_name<'a>(name: &str, taken: impl Iterator<Item = &'a str> + Clone) -> String {
    let base = name.trim().trim_start_matches('_').trim();
    let base = if base.is_empty() { "Agent" } else { base };
    if checked_name(base, taken.clone()).is_ok() {
        return base.to_owned();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|candidate| checked_name(candidate, taken.clone()).is_ok())
        .expect("a free name")
}

/// Sets the default model for new agents to the first connected tool with its default model when none is set (a
/// fresh install: AGM-02's "else the first starter entry's tool with no model"). `connected` in
/// [`Tool::ALL`]'s order. Whether it set one.
pub async fn ensure_default_model(store: &Store, connected: &[Tool]) -> Result<bool, StoreError> {
    let Some(&tool) = connected.first() else {
        return Ok(false);
    };
    if store.default_model().await?.is_some() {
        return Ok(false);
    }
    store
        .set_default_model(&ModelRef { tool, model: None })
        .await?;
    tracing::info!(%tool, "the default model for new agents is the first connected tool's");
    Ok(true)
}
