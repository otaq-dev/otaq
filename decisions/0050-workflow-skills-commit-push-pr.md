# 0050. Workflow skills (Commit, Push, Open PR), branch names chosen at the first push, and a confirmation dialog

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0009, 0015 (provider interface superseded), 0017, 0018, 0023, 0029 (branch name refined), 0031, 0032,
  0038, 0040 (refined), 0041 (who pushes superseded), 0043
- **Supersedes:** 0015's "PR providers sit behind a provider interface" and 0041's "The agent never pushes"

## Context
Grilling session, questions 10 and 11. No decision said who commits the agent's work. The Diff and Local merge only see
commits, so uncommitted files could silently miss the merge. The user also wants commit, push and PR creation to follow
their own team conventions (commit message style, PR templates, labels), and their branch names depend on the kind of work
(`bugfix/…`, `feature/…`, `poc/…`, `hotfix/…`, and others). A fixed name like `otaq/SHOP-18-checkout-api` (0029), or
a placeholder template, can't express that.

## Decision
**Workflow skills.** Three steps are Claude Code skills that the user can replace:

| Step | When it runs | Built-in default |
|---|---|---|
| **Commit** | The agent runs it before `otaq review submit` (taught by `work-ticket` and `address-review`) | `/otaq:commit`: commit everything, `SHOP-19: …` messages |
| **Push** | On the user's **Push to PR** click | `/otaq:push` |
| **Open PR** | On the user's **Open PR** click | `/otaq:open-pr`: uses `gh` / `az` (0017 auth) |
| **Validate** (0063) | In a separate validator session after `otaq review submit` | `/otaq:validate`: read-only judge |

- Settings → **Workflow skills**: a global default plus a per-project override for each step (same pattern as 0040), for
  example `/my-commit`, `/my-pr`. Stored in the app, never in the repo (0023).
- Push and Open PR run in the ticket's session (resumed if it's gone, like Send back, 0034). The user's click is the
  go-ahead for the tokens (0018).
- GitHub vs Azure DevOps (and later GitLab) lives in the Open PR skill, not in a Rust provider interface.
- **No "PR provider" setting:** the provider is **detected from the repo's remote URL** (`github.com` → GitHub / `gh`;
  `dev.azure.com` or `visualstudio.com` → Azure DevOps / `az`). Project settings show it read-only ("PR provider: GitHub
  (detected from origin) · gh: logged in"). Any other remote shows "Not detected: use your own Open PR skill", and Open PR
  works only with a custom skill. The built-in `/otaq:open-pr` does the same detection.

**Who commits.**
- **The agent commits**, through the Commit skill.
- `otaq review submit` **refuses while the worktree has uncommitted changes** ("2 files are not committed. Commit them,
  or discard them if they're scratch files").
- Local merge, Open PR and Push to PR **refuse too** if uncommitted changes appeared later ("SHOP-19 has 2 uncommitted
  files", with **View** and **Send back**).
- **Otaq never commits by itself**, except the temporary checkpoint it undoes around rebases (0005).

**Branch names (refines 0029).**
- While the agent works, the branch has a **temporary local name**, `otaq/SHOP-18`.
- The **Open PR skill proposes the real name** at the first push, following the user's rules (written in their own skill or in
  Agent instructions, 0040), for example `bugfix/SHOP-18-login-redirect`. The built-in default proposes
  `otaq/SHOP-18-<slug>`.
- **Locked after the first push.** `otaq branch rename` refuses from then on, and later pushes reuse the name.
- Worktree folders keep their key-based names (`wt/shop-web/SHOP-18`).

**Confirmation dialog before anything is pushed.**
1. On Open PR, the skill **doesn't push**. It proposes the branch name, a message per commit, and the PR title and description
   with `otaq pr propose --stdin`.
2. Otaq shows a dialog where **all of these are editable**: the branch name (validated live: a valid ref, not already
   taken locally or on the remote), each commit message, and the PR title and description. Buttons: **Push & open PR** /
   **Cancel**.
3. On confirm, **Otaq applies the edits itself** (renames the branch, rewrites the commit messages; no tokens, no file
   changes), then tells the skill to push and open the PR with the confirmed title and description. The skill reports back
   with `otaq pr opened <url>`, and the card shows "PR #481".
4. Otaq checks that the pushed **content** equals what the user reviewed. It compares content, because rewording changes
   commit IDs. A mismatch shows the 0043 warning.
- **Push to PR** uses the same dialog, with the branch name locked and only the **new** commits' messages editable.
- **Local merge stays mechanical** (no skill, no tokens, 0038). Its squash message stays editable in its own confirmation.

**New CLI commands (refines 0032):** `otaq pr propose --stdin`, `otaq pr opened <url>`,
`otaq branch rename <name>` (refused after the first push).

## Alternatives considered
- **Otaq commits everything automatically at submit:** rejected, because it can sweep in junk files or secrets (env
  files copied by `env_from`).
- **A branch-name template with placeholders:** rejected, because the prefix depends on the kind of work, which often isn't
  known until the agent has looked at the code.
- **The skill pushes without a confirmation:** rejected. The user wants to see and edit what leaves the machine.

## Consequences
- The PR provider code in Rust shrinks to nothing. Built-in skills carry it, and users can replace them.
- Pushing and opening a PR cost some tokens (the skill runs in the ticket's session).
- New designs: the Open PR / Push to PR confirmation dialog, the uncommitted-files refusal, and the Workflow skills settings.

## Changelog
- 2026-10-09: created (grilling session, user decision)
- 2026-10-09: no PR provider setting; detected from the remote URL (SDD preparation, user decision).
- 2026-10-09: Open PR / Push apply to delivery units (epics and standalone tickets); branch names and PR titles can use the External ID (0059).
- 2026-10-11: fourth workflow skill, Validate (0063).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
