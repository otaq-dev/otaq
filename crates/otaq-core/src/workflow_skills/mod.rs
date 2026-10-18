//! Imported workflow skills (WSK-01…WSK-04, WSK-07; d0092): a user's Commit, Push, Open PR or Validate skill is a
//! folder imported in Settings, never a typed name. Otaq checks it, keeps its own copy in `$OTAQ_HOME/skills/<id>/`
//! (one per setting, under an id of Otaq's own), and remembers the original folder, the import time and a
//! fingerprint of the copy. Agents only ever use the copy; a missing or broken copy means the step's built-in.
//!
//! The setting itself is [`crate::board::settings::SkillSetting`]; the actions here are `workflow_skills.import`,
//! `.reimport`, `.reset` and `.status`. Nothing is written into a repository (d0023).

pub mod folder;
pub mod testing;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::board::settings::{GlobalSettings, ProjectSettings, SkillSetting, WorkflowStep};
use crate::board::{Board, BoardError, Refusal};
use crate::data_dir::DataDir;
use crate::naming;
use crate::store::{new_id, now_ms};

pub use folder::{copy_missing, copy_problem};

/// What a setting remembers of an imported skill (WSK-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedSkill {
    /// Otaq's own id: the copy is `skills/<id>/`.
    pub id: String,
    /// The `name:` of its SKILL.md, the skill's label in Settings.
    pub name: String,
    /// The folder it was imported from (absolute).
    pub original: String,
    /// Unix milliseconds.
    pub imported_at: i64,
    /// A hash of the copy as imported ([`folder::fingerprint`]): the copy was edited when it differs.
    pub fingerprint: String,
    /// What the last import warned about (WSK-02: files the SKILL.md mentions that aren't in the folder).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// The skill a step runs (WSK-07), its copy checked: the built-in, or an imported skill with Otaq's copy's folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedSkill {
    BuiltIn(WorkflowStep),
    Imported { name: String, folder: PathBuf },
}

impl ResolvedSkill {
    pub fn is_built_in(&self) -> bool {
        matches!(self, Self::BuiltIn(_))
    }

    /// How Settings and the Open PR flow name it: the built-in as Otaq's own skill (`/otaq:open-pr`), an imported
    /// one by its `name:`.
    pub fn label(&self) -> String {
        match self {
            Self::BuiltIn(step) => step.own_default(),
            Self::Imported { name, .. } => name.clone(),
        }
    }

    /// The value the agent tools' naming takes (until the worktree copy of WSK-05 replaces it): the built-in as
    /// Otaq's own skill (`/otaq:commit`), an imported one by its copy's SKILL.md path, which the tools follow as
    /// a file.
    pub fn stored(&self) -> String {
        match self {
            Self::BuiltIn(step) => step.own_default(),
            Self::Imported { folder, .. } => folder.join(folder::SKILL_FILE).display().to_string(),
        }
    }
}

/// The skill each step runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedSkills {
    pub commit: ResolvedSkill,
    pub push: ResolvedSkill,
    pub open_pr: ResolvedSkill,
    pub validate: ResolvedSkill,
}

impl ResolvedSkills {
    pub fn get(&self, step: WorkflowStep) -> &ResolvedSkill {
        match step {
            WorkflowStep::Commit => &self.commit,
            WorkflowStep::Push => &self.push,
            WorkflowStep::OpenPr => &self.open_pr,
            WorkflowStep::Validate => &self.validate,
        }
    }
}

/// What a setting holds, for the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillKind {
    BuiltIn,
    UseGlobal,
    Imported,
    /// A typed name from before d0092: "Not imported" (WSK-07).
    Legacy,
}

/// One setting as Settings shows it (`workflow_skills.status`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillStatus {
    pub step: WorkflowStep,
    pub kind: SkillKind,
    /// The imported skill's `name:`, or the legacy value as typed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The folder it was imported from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<String>,
    /// Whether that folder still exists (Re-import is unavailable without it, WSK-04).
    #[serde(default)]
    pub original_exists: bool,
    /// Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_at: Option<i64>,
    /// Otaq's copy (Show Otaq's copy).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<PathBuf>,
    /// The copy is present and its SKILL.md reads; otherwise the step uses the built-in (shown in red).
    #[serde(default)]
    pub copy_ok: bool,
    /// Why the copy can't be used, when it can't.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy_problem: Option<String>,
    /// The copy changed since it was imported (its fingerprint): going back asks first (WSK-04).
    #[serde(default)]
    pub edited: bool,
    /// The last import's warnings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl SkillStatus {
    /// Reads the copy and the original (blocking: the fingerprint hashes the copy's files).
    fn of(step: WorkflowStep, setting: &SkillSetting, data: &DataDir) -> Self {
        let mut status = Self {
            step,
            kind: SkillKind::BuiltIn,
            name: None,
            original: None,
            original_exists: false,
            imported_at: None,
            copy: None,
            copy_ok: false,
            copy_problem: None,
            edited: false,
            warnings: Vec::new(),
        };
        match setting {
            SkillSetting::BuiltIn => {}
            SkillSetting::UseGlobal => status.kind = SkillKind::UseGlobal,
            SkillSetting::Legacy(name) => {
                status.kind = SkillKind::Legacy;
                status.name = Some(name.clone());
            }
            SkillSetting::Imported(skill) => {
                let copy = data.skill_path(&skill.id).ok();
                let problem = match &copy {
                    Some(copy) => copy_problem(copy),
                    None => Some(copy_missing()),
                };
                status.edited = copy.as_deref().is_some_and(|copy| {
                    copy.is_dir()
                        && folder::fingerprint(copy).is_ok_and(|now| now != skill.fingerprint)
                });
                status.kind = SkillKind::Imported;
                status.name = Some(skill.name.clone());
                status.original_exists = Path::new(&skill.original).is_dir();
                status.original = Some(skill.original.clone());
                status.imported_at = Some(skill.imported_at);
                status.copy = copy;
                status.copy_ok = problem.is_none();
                status.copy_problem = problem;
                status.warnings.clone_from(&skill.warnings);
            }
        }
        status
    }
}

/// Runs blocking file work off the service's async threads.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(work)
        .await
        .expect("workflow skill file work panicked")
}

impl Board {
    /// The four settings of the global page (`project` `None`) or of a project, in [`WorkflowStep::ALL`]'s order.
    pub async fn workflow_skills_status(
        &self,
        project: Option<&str>,
    ) -> Result<Vec<SkillStatus>, BoardError> {
        let settings: Vec<(WorkflowStep, SkillSetting)> = match project {
            None => {
                let s = self.global_settings().await?.workflow_skills;
                WorkflowStep::ALL
                    .into_iter()
                    .map(|step| (step, s.get(step).clone()))
                    .collect()
            }
            Some(key) => {
                let s = self.project_settings(key).await?.workflow_skills;
                WorkflowStep::ALL
                    .into_iter()
                    .map(|step| (step, s.get(step).clone()))
                    .collect()
            }
        };
        let data = self.data().clone();
        Ok(blocking(move || {
            settings
                .iter()
                .map(|(step, setting)| SkillStatus::of(*step, setting, &data))
                .collect()
        })
        .await)
    }

    /// Imports `folder` as the skill of `step` (WSK-01, WSK-02; Import a different folder too): checked, copied
    /// whole into a new `skills/<id>/`, and recorded in the setting, whose earlier copy is deleted once the new one
    /// is in place. Refused, with nothing changed, for a relative path, a folder with no SKILL.md, or front matter
    /// that doesn't read; files the SKILL.md mentions that are missing are the status's warnings.
    pub async fn import_workflow_skill(
        &self,
        project: Option<&str>,
        step: WorkflowStep,
        folder: &Path,
    ) -> Result<SkillStatus, BoardError> {
        let _changing = self.lock_skill_changes().await;
        self.import_locked(project, step, folder).await
    }

    /// Re-import (WSK-04): the import again from the remembered original. Refused when the setting has no imported
    /// skill, or "original not found" when its folder is gone (the copy stays). Asking first is the app's.
    pub async fn reimport_workflow_skill(
        &self,
        project: Option<&str>,
        step: WorkflowStep,
    ) -> Result<SkillStatus, BoardError> {
        let _changing = self.lock_skill_changes().await;
        let SkillSetting::Imported(skill) = self.skill_setting(project, step).await? else {
            return Err(BoardError::refused(
                Refusal::NotImported,
                "No skill was imported for this step",
            ));
        };
        let original = PathBuf::from(&skill.original);
        if !original.is_dir() {
            return Err(BoardError::refused(
                Refusal::OriginalNotFound,
                format!("Original not found: {}", skill.original),
            ));
        }
        self.import_locked(project, step, &original).await
    }

    /// Use built-in (global) or Use global (a project) (WSK-04): the setting goes back and its copy is deleted.
    /// When the copy was edited since its import and `confirmed` is false, refused with `needs_confirmation` and
    /// nothing changes.
    pub async fn reset_workflow_skill(
        &self,
        project: Option<&str>,
        step: WorkflowStep,
        confirmed: bool,
    ) -> Result<SkillStatus, BoardError> {
        let _changing = self.lock_skill_changes().await;
        let current = self.skill_setting(project, step).await?;
        let copy = current
            .imported()
            .and_then(|skill| Some((self.data().skill_path(&skill.id).ok()?, skill.clone())));
        if let Some((copy, skill)) = &copy
            && !confirmed
        {
            let (copy, fingerprint) = (copy.clone(), skill.fingerprint.clone());
            let edited = blocking(move || {
                copy.is_dir() && folder::fingerprint(&copy).is_ok_and(|now| now != fingerprint)
            })
            .await;
            if edited {
                let back = if project.is_some() {
                    "the global skill"
                } else {
                    "the built-in"
                };
                return Err(BoardError::refused(
                    Refusal::NeedsConfirmation,
                    format!(
                        "{}'s copy has edits that aren't in the original. Going back to {back} deletes them.",
                        naming::PRODUCT_NAME
                    ),
                ));
            }
        }
        let back = match project {
            None => SkillSetting::BuiltIn,
            Some(_) => SkillSetting::UseGlobal,
        };
        self.put_skill_setting(project, step, back.clone()).await?;
        if let Some((copy, _)) = copy {
            blocking(move || folder::remove(&copy)).await;
        }
        Ok(SkillStatus::of(step, &back, self.data()))
    }

    /// `settings.set`: stores the global settings but keeps the workflow skills as they are (they change only
    /// through import and reset); a typed name the request changes is refused (WSK-01).
    pub async fn save_global_settings(&self, settings: &GlobalSettings) -> Result<(), BoardError> {
        let _changing = self.lock_skill_changes().await;
        let stored = self.global_settings().await?.workflow_skills;
        for step in WorkflowStep::ALL {
            refuse_typed_name(settings.workflow_skills.get(step), stored.get(step))?;
        }
        let mut settings = settings.clone();
        settings.workflow_skills = stored;
        self.set_global_settings(&settings).await
    }

    /// `project_settings.set`, as [`Self::save_global_settings`].
    pub async fn save_project_settings(
        &self,
        project: &str,
        settings: &ProjectSettings,
    ) -> Result<(), BoardError> {
        let _changing = self.lock_skill_changes().await;
        let stored = self.project_settings(project).await?.workflow_skills;
        for step in WorkflowStep::ALL {
            refuse_typed_name(settings.workflow_skills.get(step), stored.get(step))?;
        }
        let mut settings = settings.clone();
        settings.workflow_skills = stored;
        self.set_project_settings(project, &settings).await
    }

    /// Deletes every folder under `skills/` that no setting holds (at service start): copies left by a crash
    /// between a copy and its setting, or by a project removed while its copies couldn't be deleted.
    pub async fn remove_unreferenced_skill_copies(&self) -> Result<usize, BoardError> {
        let _changing = self.lock_skill_changes().await;
        let mut held: HashSet<String> = HashSet::new();
        let global = self.global_settings().await?.workflow_skills;
        held.extend(
            WorkflowStep::ALL
                .into_iter()
                .filter_map(|step| global.get(step).imported().map(|s| s.id.clone())),
        );
        for project in self.projects().await? {
            let own = self.project_settings(&project.key).await?.workflow_skills;
            held.extend(
                WorkflowStep::ALL
                    .into_iter()
                    .filter_map(|step| own.get(step).imported().map(|s| s.id.clone())),
            );
        }
        let dir = self.data().skills_dir();
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

    /// Deletes the copies a project's settings hold (the project is being removed).
    pub(crate) async fn remove_skill_copies_of(&self, settings: &ProjectSettings) {
        let copies: Vec<PathBuf> = WorkflowStep::ALL
            .into_iter()
            .filter_map(|step| settings.workflow_skills.get(step).imported())
            .filter_map(|skill| self.data().skill_path(&skill.id).ok())
            .collect();
        blocking(move || copies.iter().for_each(|copy| folder::remove(copy))).await;
    }

    async fn import_locked(
        &self,
        project: Option<&str>,
        step: WorkflowStep,
        folder: &Path,
    ) -> Result<SkillStatus, BoardError> {
        let previous = self.skill_setting(project, step).await?;
        if !folder.is_absolute() {
            return Err(BoardError::refused(
                Refusal::BadSkillFolder,
                format!("{} isn't an absolute path", folder.display()),
            ));
        }
        let id = new_id();
        let (data, from, copy_id) = (self.data().clone(), folder.to_owned(), id.clone());
        let copied = blocking(move || folder::import(&data, &from, &copy_id))
            .await
            .map_err(|folder::Refused(why)| BoardError::refused(Refusal::BadSkillFolder, why))?;
        let setting = SkillSetting::Imported(Box::new(ImportedSkill {
            id: id.clone(),
            name: copied.name,
            original: folder.display().to_string(),
            imported_at: now_ms(),
            fingerprint: copied.fingerprint,
            warnings: copied.warnings,
        }));
        let new_copy = self.data().skill_path(&id).ok();
        if let Err(e) = self.put_skill_setting(project, step, setting.clone()).await {
            if let Some(copy) = new_copy {
                blocking(move || folder::remove(&copy)).await;
            }
            return Err(e);
        }
        if let Some(old) = previous
            .imported()
            .and_then(|skill| self.data().skill_path(&skill.id).ok())
        {
            blocking(move || folder::remove(&old)).await;
        }
        let data = self.data().clone();
        Ok(blocking(move || SkillStatus::of(step, &setting, &data)).await)
    }

    async fn skill_setting(
        &self,
        project: Option<&str>,
        step: WorkflowStep,
    ) -> Result<SkillSetting, BoardError> {
        Ok(match project {
            None => self
                .global_settings()
                .await?
                .workflow_skills
                .get(step)
                .clone(),
            Some(key) => self
                .project_settings(key)
                .await?
                .workflow_skills
                .get(step)
                .clone(),
        })
    }

    async fn put_skill_setting(
        &self,
        project: Option<&str>,
        step: WorkflowStep,
        setting: SkillSetting,
    ) -> Result<(), BoardError> {
        match project {
            None => {
                let mut settings = self.global_settings().await?;
                *settings.workflow_skills.get_mut(step) = setting;
                self.set_global_settings(&settings).await
            }
            Some(key) => {
                let mut settings = self.project_settings(key).await?;
                *settings.workflow_skills.get_mut(step) = setting;
                self.set_project_settings(key, &settings).await
            }
        }
    }
}

/// A settings save may not bring a typed name the setting doesn't already hold (WSK-01: a typed name can't be
/// entered any more).
fn refuse_typed_name(sent: &SkillSetting, stored: &SkillSetting) -> Result<(), BoardError> {
    match sent {
        SkillSetting::Legacy(name) if sent != stored => Err(BoardError::refused(
            Refusal::TypedSkillName,
            format!("{name} is a typed name; import a skill folder instead"),
        )),
        _ => Ok(()),
    }
}
