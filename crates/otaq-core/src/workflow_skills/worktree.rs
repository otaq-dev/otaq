//! The worktree copy (WSK-05; d0092): before a session runs a workflow skill, Otaq's copy of an imported skill is
//! written into the worktree as `.agents/skills/otaq-custom-<step>/` (where Codex, Copilot and OpenCode look) and
//! `.claude/skills/otaq-custom-<step>/` (Claude Code), fresh each time: its SKILL.md's `name:` rewritten to
//! `otaq-custom-<step>` (only that line), the files' modes kept, and a `.gitignore` holding `*` in each folder so
//! neither shows in `git status` or a commit. A step on the built-in has its folders removed. Nothing else in the
//! worktree or its `.git` folder is written (d0023), and no git command runs.
//!
//! Agents are pointed at the first folder by its path ([`named_folder`], WSK-06): it is gitignored, so a search may
//! not find it. Blocking file work: callers run it off the service's async threads ([`Board::copy_skills_into`]).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::folder::{self, SKILL_FILE, skill_name};
use super::{ResolvedSkill, ResolvedSkills, blocking};
use crate::board::settings::WorkflowStep;
use crate::board::{Board, BoardError};

/// Where the copies go, relative to the worktree: the folder every tool but Claude Code reads first (the one agents
/// are pointed at), then Claude Code's.
const SKILL_ROOTS: [&str; 2] = [".agents/skills", ".claude/skills"];

/// The `.gitignore` written into each copy: everything in the folder, itself too.
const IGNORE_ALL: &str = "*\n";

/// `otaq-custom-commit`, `otaq-custom-open-pr`: the copy's folder name and its SKILL.md's `name:`.
pub fn folder_name(step: WorkflowStep) -> String {
    crate::naming::custom_skill(step.own_skill())
}

/// The folder an agent is pointed at, relative to the worktree: `.agents/skills/otaq-custom-commit/` (WSK-06).
pub fn named_folder(step: WorkflowStep) -> String {
    format!("{}/{}/", SKILL_ROOTS[0], folder_name(step))
}

/// Every step's copies, relative to the worktree: what the warm build never keeps or seeds (d0082).
pub fn all_folders() -> Vec<PathBuf> {
    WorkflowStep::ALL
        .into_iter()
        .flat_map(|step| SKILL_ROOTS.map(|root| Path::new(root).join(folder_name(step))))
        .collect()
}

/// Whether the copy of `step` is in `worktree` (its named folder's SKILL.md): `prime` names it only then.
pub fn is_written(worktree: &Path, step: WorkflowStep) -> bool {
    worktree.join(named_folder(step)).join(SKILL_FILE).is_file()
}

/// Writes `skill`, the skill in force for `step`, into `worktree`: removes the step's two folders, then for an
/// imported skill writes both again from Otaq's copy. Each folder is written under a temporary name in the same
/// parent and renamed into place, so a failure leaves no partial folder: both are removed and the reason returned
/// (the step uses the built-in for this session).
pub fn write(worktree: &Path, step: WorkflowStep, skill: &ResolvedSkill) -> Result<(), String> {
    let name = folder_name(step);
    let remove = || {
        for root in SKILL_ROOTS {
            let parent = worktree.join(root);
            folder::remove(&parent.join(&name));
            folder::remove(&parent.join(partial_name(&name)));
        }
    };
    remove();
    let ResolvedSkill::Imported { folder: copy, .. } = skill else {
        return Ok(());
    };
    let written = if worktree.is_dir() {
        SKILL_ROOTS
            .into_iter()
            .try_for_each(|root| write_one(copy, &worktree.join(root), &name))
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} isn't a folder", worktree.display()),
        ))
    };
    written.map_err(|e| {
        remove();
        format!("Can't write {name} into the worktree: {e}")
    })
}

/// `.otaq-custom-commit.partial`: where a copy is written before it is renamed into place.
fn partial_name(name: &str) -> String {
    format!(".{name}.partial")
}

/// One copy in `parent` (`<worktree>/.agents/skills`), created when missing: it holds only ignored folders.
fn write_one(copy: &Path, parent: &Path, name: &str) -> io::Result<()> {
    fs::create_dir_all(parent)?;
    let partial = parent.join(partial_name(name));
    fs::create_dir(&partial)?;
    let made = (|| {
        // Ignored from the first moment, even while partial.
        fs::write(partial.join(".gitignore"), IGNORE_ALL)?;
        folder::copy_into(copy, &partial)?;
        // The skill's own `.gitignore`, if it has one, gives way.
        fs::write(partial.join(".gitignore"), IGNORE_ALL)?;
        let skill_md = partial.join(SKILL_FILE);
        let text = fs::read_to_string(&skill_md)?;
        let renamed = with_name(&text, name).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{SKILL_FILE}'s name: can't be rewritten"),
            )
        })?;
        fs::write(&skill_md, renamed)?;
        fs::set_permissions(&partial, fs::metadata(copy)?.permissions())?;
        fs::rename(&partial, parent.join(name))
    })();
    if made.is_err() {
        folder::remove(&partial);
    }
    made
}

/// `text` (a SKILL.md) with its front matter's `name:` line replaced by `name: <name>`, its line ending kept and
/// every other byte as it was. `None` when the front matter has no plain `name:` line or the result wouldn't read.
fn with_name(text: &str, name: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len() + name.len());
    let mut lines = text.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_start_matches('\u{feff}').trim_end() != "---" {
        return None;
    }
    out.push_str(first);
    let mut renamed = false;
    let mut in_front_matter = true;
    for line in lines {
        if in_front_matter && line.trim_end() == "---" {
            in_front_matter = false;
        } else if in_front_matter && line.starts_with("name:") {
            let ending = &line[line.trim_end_matches(['\r', '\n']).len()..];
            out.push_str(&format!("name: {name}{ending}"));
            renamed = true;
            continue;
        }
        out.push_str(line);
    }
    (renamed && skill_name(&out).as_deref() == Ok(name)).then_some(out)
}

/// What [`prepare`] wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Copies {
    /// The session's steps whose imported skill is in the worktree.
    copied: Vec<WorkflowStep>,
    /// The session's imported steps that couldn't be written, and why: they use the built-in this time.
    pub failures: Vec<(WorkflowStep, String)>,
}

impl Copies {
    /// Whether `step`'s imported skill is in the worktree: the agent is pointed at its folder, else the built-in.
    pub fn copied(&self, step: WorkflowStep) -> bool {
        self.copied.contains(&step)
    }
}

/// Before a session that runs `steps` in `worktree`: each of them is written ([`write`]), and every step on the
/// built-in has its folders removed (a step back on the built-in since the last launch). Another step's imported
/// copy is left for its own session to refresh.
pub fn prepare(worktree: &Path, skills: &ResolvedSkills, steps: &[WorkflowStep]) -> Copies {
    let mut copies = Copies::default();
    for step in WorkflowStep::ALL {
        let skill = skills.get(step);
        if !skill.is_built_in() && !steps.contains(&step) {
            continue;
        }
        match write(worktree, step, skill) {
            Ok(()) if !skill.is_built_in() => copies.copied.push(step),
            Ok(()) => {}
            Err(why) => copies.failures.push((step, why)),
        }
    }
    copies
}

impl Board {
    /// The worktree copy right before a session that runs `steps` in `worktree` (WSK-05): the project's skills in
    /// force, written by [`prepare`]. A copy that can't be written is logged, and its step uses the built-in for this
    /// session.
    pub async fn copy_skills_into(
        &self,
        project: &str,
        worktree: &Path,
        steps: &[WorkflowStep],
    ) -> Result<Copies, BoardError> {
        let skills = self.effective_settings(project).await?.workflow_skills;
        let (at, steps) = (worktree.to_owned(), steps.to_vec());
        let copies = blocking(move || prepare(&at, &skills, &steps)).await;
        for (step, why) in &copies.failures {
            tracing::warn!(
                project,
                worktree = %worktree.display(),
                step = step.field(),
                error = %why,
                "the imported workflow skill wasn't copied; the step uses the built-in"
            );
        }
        Ok(copies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_name_line_changes() {
        let text =
            "---\r\nname: my-commit\r\ndescription: Commits.\r\n---\r\nname: in the body\r\n";
        assert_eq!(
            with_name(text, "custom-commit").unwrap(),
            "---\r\nname: custom-commit\r\ndescription: Commits.\r\n---\r\nname: in the body\r\n"
        );
        assert_eq!(
            with_name("---\nname: \"my pr\"\n---", "custom-open-pr").unwrap(),
            "---\nname: custom-open-pr\n---"
        );
        assert_eq!(with_name("---\ndescription: x\n---\n", "x"), None);
        assert_eq!(with_name("no front matter\nname: x\n", "x"), None);
    }

    #[test]
    fn names_and_folders() {
        let cmd = crate::naming::COMMAND_NAME;
        assert_eq!(
            folder_name(WorkflowStep::OpenPr),
            format!("{cmd}-custom-open-pr")
        );
        assert_eq!(
            named_folder(WorkflowStep::Commit),
            format!(".agents/skills/{cmd}-custom-commit/")
        );
        let all = all_folders();
        assert_eq!(all.len(), 8);
        assert!(all.contains(&PathBuf::from(format!(
            ".claude/skills/{cmd}-custom-validate"
        ))));
    }
}
