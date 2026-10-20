//! An agent's imported skills (AGM-01, AGM-04; WSK-02…WSK-04, d0092): **Import skill folder…** checks a folder, keeps
//! Otaq's own copy in `$OTAQ_HOME/skills/<id>/` and adds it to the agent's skills; **Re-import** copies the remembered
//! original again. Agents only ever use the copy; a missing or broken copy means the shipped skill it stands for.
//!
//! Until each agent's skills are copied into the worktree by one name (AGM-07, T-0408), the workflow steps of M31 run
//! the imported skill that stands for their shipped one ([`step_skills`]): Commit and Push the first Agent stage's
//! agent's, Open PR the Delivery stage's, Validate the first agent check's.

use std::path::{Path, PathBuf};

use super::{Agent, ImportedSkill, SkillRef, resolve};
use crate::board::settings::WorkflowStep;
use crate::board::{Board, BoardError, Refusal};
use crate::flow::Flow;
use crate::store::{new_id, now_ms};
use crate::workflow_skills::{ResolvedSkill, ResolvedSkills, folder};

/// Runs blocking file work off the service's async threads.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(work)
        .await
        .expect("skill file work panicked")
}

/// Copies `folder` into a new `skills/<id>/` (WSK-02): refused, with nothing changed, for a relative path, a folder
/// with no SKILL.md, or front matter that doesn't read.
async fn copy_in(board: &Board, from: &Path) -> Result<ImportedSkill, BoardError> {
    if !from.is_absolute() {
        return Err(BoardError::refused(
            Refusal::BadSkillFolder,
            format!("{} isn't an absolute path", from.display()),
        ));
    }
    let id = new_id();
    let (data, source, copy_id) = (board.data().clone(), from.to_owned(), id.clone());
    let copied = blocking(move || folder::import(&data, &source, &copy_id))
        .await
        .map_err(|folder::Refused(why)| BoardError::refused(Refusal::BadSkillFolder, why))?;
    Ok(ImportedSkill {
        id,
        name: copied.name,
        original: from.display().to_string(),
        imported_at: now_ms(),
        fingerprint: copied.fingerprint,
        warnings: copied.warnings,
        stands_for: None,
    })
}

async fn remove_copy(board: &Board, id: &str) {
    if let Ok(copy) = board.data().skill_path(id) {
        blocking(move || folder::remove(&copy)).await;
    }
}

/// **Import skill folder…** on `agent_id` (`agents.import_skill`): the folder checked and copied, then added at the
/// end of the agent's skills. The import's warnings are the record's.
pub async fn import(board: &Board, agent_id: &str, from: &Path) -> Result<Agent, BoardError> {
    let _changing = board.lock_skill_changes().await;
    let store = board.store();
    let agent = store
        .agent(agent_id)
        .await?
        .ok_or_else(|| BoardError::NotFound(format!("no agent {agent_id}")))?;
    let skill = copy_in(board, from).await?;
    let mut new = super::store::NewAgent {
        name: agent.name.clone(),
        instructions: agent.instructions.clone(),
        skills: agent.skills.clone(),
        imported: agent.imported.clone(),
        model: agent.model.clone(),
        effort: agent.effort.clone(),
        role: agent.role,
    };
    new.skills.push(SkillRef::Imported(skill.id.clone()));
    new.imported.push(skill.clone());
    if let Err(e) = store.update_agent(agent_id, &new, false).await {
        remove_copy(board, &skill.id).await;
        return Err(e.into());
    }
    store
        .agent(agent_id)
        .await?
        .ok_or_else(|| BoardError::NotFound(format!("no agent {agent_id}")))
}

/// **Re-import** (`agents.reimport_skill`, WSK-04): the import again from the remembered original, into a fresh copy
/// that replaces the old one for every agent holding it. "Original not found" when its folder is gone (the copy
/// stays).
pub async fn reimport(board: &Board, agent_id: &str, skill_id: &str) -> Result<Agent, BoardError> {
    let _changing = board.lock_skill_changes().await;
    let store = board.store();
    let agent = store
        .agent(agent_id)
        .await?
        .ok_or_else(|| BoardError::NotFound(format!("no agent {agent_id}")))?;
    let Some(old) = agent.imported_skill(skill_id).cloned() else {
        return Err(BoardError::refused(
            Refusal::NotImported,
            format!("{} has no such imported skill", agent.name),
        ));
    };
    let original = PathBuf::from(&old.original);
    if !original.is_dir() {
        return Err(BoardError::refused(
            Refusal::OriginalNotFound,
            format!("Original not found: {}", old.original),
        ));
    }
    let fresh = copy_in(board, &original).await?;
    let done = sqlx::query(
        "UPDATE agent_skills SET imported_id = ?, name = ?, original = ?, imported_at = ?, fingerprint = ?,
                                 warnings_json = ?
         WHERE imported_id = ?",
    )
    .bind(&fresh.id)
    .bind(&fresh.name)
    .bind(&fresh.original)
    .bind(fresh.imported_at)
    .bind(&fresh.fingerprint)
    .bind(serde_json::to_string(&fresh.warnings).expect("warnings serialize"))
    .bind(skill_id)
    .execute(store.pool())
    .await;
    if let Err(e) = done {
        remove_copy(board, &fresh.id).await;
        return Err(crate::store::StoreError::from(e).into());
    }
    remove_copy(board, skill_id).await;
    store
        .agent(agent_id)
        .await?
        .ok_or_else(|| BoardError::NotFound(format!("no agent {agent_id}")))
}

/// Deletes every folder under `skills/` that no agent holds (at service start): copies left by a crash between a
/// copy and its record, by a deleted agent or by the move-over's dropped project skills.
pub async fn remove_unreferenced(board: &Board) -> Result<usize, BoardError> {
    let _changing = board.lock_skill_changes().await;
    let held: std::collections::HashSet<String> = board
        .store()
        .imported_skill_ids()
        .await?
        .into_iter()
        .collect();
    let dir = board.data().skills_dir();
    Ok(blocking(move || {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return 0;
        };
        let mut removed = 0;
        for entry in entries.flatten() {
            if !held.contains(&*entry.file_name().to_string_lossy()) {
                folder::remove(&entry.path());
                removed += 1;
            }
        }
        removed
    })
    .await)
}

/// The skill `agent` runs for `step`: its imported skill standing for the step's shipped one, when Otaq's copy is
/// there and reads; else the built-in (WSK-04: a missing or broken copy means the built-in).
fn step_skill(board: &Board, agent: Option<&Agent>, step: WorkflowStep) -> ResolvedSkill {
    agent
        .and_then(|agent| {
            agent.skills.iter().find_map(|skill| match skill {
                SkillRef::Imported(id) => agent
                    .imported_skill(id)
                    .filter(|s| s.stands_for.as_deref() == Some(step.own_skill())),
                SkillRef::Shipped(_) => None,
            })
        })
        .and_then(|skill| {
            let folder = board.data().skill_path(&skill.id).ok()?;
            folder::copy_problem(&folder)
                .is_none()
                .then(|| ResolvedSkill::Imported {
                    name: skill.name.clone(),
                    folder,
                })
        })
        .unwrap_or(ResolvedSkill::BuiltIn(step))
}

/// The skill each workflow step of `project` runs ([`crate::board::settings::EffectiveSettings::workflow_skills`]),
/// from the agents its flow's stages name. The flow is the default file's (or a test's): which agents the stages
/// name doesn't depend on the project's Validation settings.
pub async fn step_skills(board: &Board, project: &str) -> Result<ResolvedSkills, BoardError> {
    let flow = board
        .test_flow(project)
        .unwrap_or_else(crate::flow::default_flow);
    let agent_of = async |stage: Option<&crate::flow::Stage>| -> Result<Option<Agent>, BoardError> {
        Ok(match stage.and_then(|s| s.agent.as_deref()) {
            Some(name) => resolve::named(board.store(), name).await?,
            None => None,
        })
    };
    let developer = agent_of(flow.first_agent_stage()).await?;
    let deliverer = agent_of(delivery_stage(&flow)).await?;
    let validator = agent_of(flow.stages.iter().find(|s| s.is_agent_check())).await?;
    Ok(ResolvedSkills {
        commit: step_skill(board, developer.as_ref(), WorkflowStep::Commit),
        push: step_skill(board, developer.as_ref(), WorkflowStep::Push),
        open_pr: step_skill(
            board,
            deliverer.as_ref().or(developer.as_ref()),
            WorkflowStep::OpenPr,
        ),
        validate: step_skill(board, validator.as_ref(), WorkflowStep::Validate),
    })
}

fn delivery_stage(flow: &Flow) -> Option<&crate::flow::Stage> {
    flow.stages
        .iter()
        .find(|s| s.is_delivery() && s.agent.is_some())
}
