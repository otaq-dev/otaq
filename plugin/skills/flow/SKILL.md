---
name: flow
description: Change an Otaq project's board flow as its Project agent - read the current flow with `otaq flow show`, write the whole flow file (agents and stages, every field), give each stage a why line that names its source, propose it with `otaq flow propose --stdin`, then end your turn and wait for the user's Apply, Cancel or Ask for more changes as your next message.
disable-model-invocation: true
---

# Change the project's flow

You are a **Project agent** of an Otaq project, and the user wants its **flow** changed: the stages a ticket walks
between Backlog and Done, and the agents that run them. They started you with this skill (Settings → Flow → **Change
with the Project agent**, or by typing `/flow`), Otaq started you for a new project's first flow (see "A first flow"),
or they just asked ("add a Security check after Validate").

**You never change the live flow.** You write the whole flow file and **propose** it; Otaq shows the user a preview
and nothing changes until they click **Apply**.

## Steps

1. **Get the context first.** Run `otaq prime --project` and read all of it, then **read the current flow**:

   ```sh
   otaq flow show
   ```

   It prints the project's applied flow and the agents it names, as the file `propose` reads (`--json` prints the
   same as JSON). This is your starting point: change it, don't start from nothing.
2. **Follow the Project agent instructions** that `prime --project` prints. They add to these steps; they never
   remove proposing before anything changes (step 5) or waiting for the answer (step 6).
3. **Understand the request.** If it leaves something open that changes the flow (which agent, which model, what a
   check looks for), ask the user one question at a time with `otaq ask human --stdin` (2–4 options with notes when
   there are clear alternatives), then end your turn and wait for the answer. Don't ask what the request already
   says. To start from a ready-made flow, `otaq flow presets` lists the ones Otaq ships, with one line each.
4. **Write the whole file**: every agent the stages name, and every stage, in the format below. Keep the stages and
   agents you aren't asked to change exactly as `flow show` printed them, and keep each stage's `id`: Otaq matches
   stages by id, name or position, so renaming Review to QA stays a rename.
5. **Propose it**, with a `why:` line on each stage (see "Why lines" below), on stdin:

   ```sh
   otaq flow propose --stdin <<'FLOW'
   agents:
     - name: Security reviewer
       instructions: |
         Never edit files.
         Fail on unescaped SQL.
       skills: [security-review]
       model: claude:opus
       why: The user asked for a security check on opus in this conversation.
   flow:
     - id: dev
       name: Dev
       kind: agent
       column: own
       agent: Developer
       skill: work-ticket
       why: Unchanged from the current flow.
     …
   FLOW
   ```

   (`otaq flow propose -f flow.yaml` reads a file instead.) Otaq checks everything first: the rules of a flow (below),
   the agents' names and models, the skills. **If it refuses**, it names the stage or agent and the reason (for
   example "Review is a check by you before any Agent stage: a Fail would have nowhere to go"): fix exactly that and
   propose again. Don't tell the user about a refusal you can fix yourself.
6. **End your turn and wait.** A valid proposal returns at once ("Proposal 3 is waiting for the user") and the preview
   opens beside you. Tell the user in one or two lines what you proposed, then end your turn; don't wait in a loop or
   propose again meanwhile. **The user's answer arrives as your next message**:
   - **Applied:** say what changes and when, in a line ("Security runs from the next ticket that reaches it; adding
     the stage waits until the board is quiet"). You're done.
   - **Cancelled:** acknowledge it in a line. Don't propose again unless the user asks.
   - **Ask for more changes**, with the user's text ("use sonnet for Security instead"): change the file, propose it
     again (step 5), and wait again.

## A first flow

When the first message says it is the project's **first flow** ("This is shop-web's first flow. Look at how the user
works: …"), Otaq started you right after the project was added: there is no request yet, so you **look at how the
user works and propose a whole flow** from it. The message names what there is; look at each of those, and anything
else that helps:

- **Their Claude Code history**: what they typed into Claude Code in every project, slash commands included (pasted
  text is left out). The last 12 months, in pages of 500, oldest first; the last line names the next page's command:

  ```sh
  otaq history
  otaq history --project shop-web
  otaq history --command /security-review
  otaq history --page 2
  ```

  Look for the slash commands they run often, and where (`/security-review`, `/commit`, a review or test skill),
  and what they ask for before and after code ("write the tests first", "check the migration"). Older history only
  when the user asks: `otaq history --all`.
- **Their skills, plugins and instruction files**: `~/.claude/skills/`, the plugins they installed, `CLAUDE.md` and
  `AGENTS.md` in their home folder and in this repo. A skill they run often can become a stage's skill (its folder's
  path in `skills:`).
- **The repo**: `git log` (how commits are written, who commits), branch names, CI files (`.github/workflows/`,
  `.gitlab-ci.yml`), PR templates and `CONTRIBUTING.md` (what a change needs before it merges).
- **GitHub or GitLab**, when the message names them: merged PRs and their reviews (`gh pr list --state merged`,
  `gh pr view <n> --comments`; `glab mr list --merged`), to see whether changes go through PRs and what reviewers
  ask for.

Then **start from the closest preset** (`otaq flow presets`) and change it to fit what you found. Every why line
names its source with a count you actually found ("From 41 `/security-review` runs in shop-web and api (Claude Code
history)", "`CONTRIBUTING.md` asks for a review before merging"), or says "No evidence" (see "Why lines"). Don't add
`skip` or an automatic Delivery with no check before it unless the user asks (see "What you add only when the user
asks"). Then propose it with `otaq flow propose --stdin` and end your turn (steps 5 and 6): the user picks **Use
this flow**, **Ask for changes** or **Use the default**, and their answer is your next message.

## The file

Two lists: `agents:` (who runs the stages) and `flow:` (the stages between Backlog and Done). Backlog, Ready for Dev
and Done are always there, so never write them.

### Agents

```yaml
agents:
  - name: Rust developer           # unique; stages and options name it
    instructions: |                # optional: this agent's own rules, given at every start
      Follow our error-handling rules: no unwrap in library code.
    skills: [work-ticket, commit]  # the skills it has
    model: default                 # default · claude · claude:opus · codex:<model> · copilot · opencode:<model>
    why: From 41 `/commit-compact` runs in shop-web (Claude Code history).   # optional; see "Why lines"
```

- **`name`**: what the board and Settings show. Agents belong to the account and are shared by every project: an
  agent that already exists (in `flow show`, or one the user has) is reused by its name. Changing a shared agent
  changes it everywhere it is used, and the preview lists those projects; to change it for this project only, add a
  new agent with its own name.
- **`role`** (rare): `project-agent` or `epic-lead`. Exactly one agent holds each role; leave it out unless the user
  asks to move a role.
- **`instructions`**: Markdown, optional.
- **`skills`**: the names of skills Otaq ships (`work-ticket`, `commit`, `validate`, `grill`, `open-pr`, `restack`, …)
  or that an agent already has, or the absolute path of a skill folder (holding a `SKILL.md`) to import on Apply,
  for example `/Users/me/.claude/skills/security-review`. Every skill is copied into the worktree as `otaq-<name>`.
- **`why`**: optional, one line on why the agent is there; the preview shows it. Otaq doesn't act on it.
- **`model`**: `default` (the account's default model; prefer it unless the user names one), a tool (`claude`,
  `codex`, `copilot`, `opencode`: that tool's default model) or `tool:model` (`claude:opus`, `claude:haiku`).

### Stages

```yaml
flow:
  - id: grill                    # optional; keep the id flow show printed, never invent one starting with _
    name: Grill                  # unique; the column's title
    kind: agent                  # agent · check · delivery
    column: own                  # own · with-previous
    agent: Griller               # who runs it
    skill: grill                 # the skill it runs first
    options: [skip]              # optional: other agents a ticket may pick instead, or skip
    why: The user asked to settle tickets before any code is written.
```

| Field | Kinds | What it says |
|---|---|---|
| `id` | every stage | Its permanent id. Keep the ones `flow show` printed; a new stage gets one from its name. |
| `name` | every stage | Its name on the board ("Security"). Unique. |
| `kind` | every stage | `agent`: an agent works on the ticket. `check`: an agent or the user passes or fails it. `delivery`: Local merge or PR, always last. |
| `by` | check | `agent` (an agent check: read-only, ends with a verdict) or `you` (the user's check: Pass or Fail). |
| `column` | every stage | `own`: its own column. `with-previous`: its cards show in the column before it. Default `own`. |
| `agent` | agent, agent check, PR delivery | The agent that runs it, by name. A check by you and a Local merge have none. |
| `skill` | agent, agent check, PR delivery | The skill it runs first (`work-ticket`, `validate`, `open-pr`…). |
| `options` | agent, check | Other agents a ticket may use instead (`[Quick developer]`), and `skip` when a ticket may skip the stage. Default: none. |
| `rounds` | agent check | How many fails in a row (1–5) before the card stops for the user. |
| `mode` | delivery | `click`: waits for the user's Local merge or Open PR. `automatic`: merges, or pushes and opens the PR, with no click and no dialog. |
| `why` | every stage | One line on why the stage is there, naming its source (below). The preview shows it; Otaq doesn't act on it. |
| `delivers` | delivery | `local` (Local merge, no agent), `pr` (runs its agent and skill), or `either` (the user picks; click mode only). |

A whole example, the default flow with Grill first:

```yaml
agents:
  - name: Griller
    skills: [grill]
    model: default
  - name: Developer
    skills: [work-ticket, commit]
    model: default
  - name: Validator
    skills: [validate]
    model: default
flow:
  - id: grill
    name: Grill
    kind: agent
    column: own
    agent: Griller
    skill: grill
  - id: dev
    name: Dev
    kind: agent
    column: own
    agent: Developer
    skill: work-ticket
  - id: validate
    name: Validate
    kind: check
    by: agent
    column: own
    agent: Validator
    skill: validate
    rounds: 3
  - id: review
    name: Review
    kind: check
    by: you
    column: own
  - id: deliver
    name: Done
    kind: delivery
    column: with-previous
    agent: Developer
    skill: open-pr
    mode: click
    delivers: either
```

### The rules of a flow

`propose` refuses a file that breaks one, naming the stage:

- At least one `agent` stage; exactly one `delivery` stage, and it is last.
- No check by you before the first Agent stage (a Fail must have an Agent stage to go back to).
- `with-previous` is refused on the first stage, and into a column that belongs to a check by you (only the Delivery
  may share it, as Done shares Review's on the default flow).
- `either` only with `mode: click`; a Delivery can't be skipped; `rounds` from 1 to 5.
- With every `skip` taken, an Agent stage must still run, and every check by you must still have one before it.
- Every agent a stage or an option names is in `agents:` or already exists.

### What you add only when the user asks

- **`skip`** on any stage, and an **automatic Delivery with no check before it**. Otaq allows both; the preview warns
  about the second ("Done opens a PR on GitHub without any check before it"). Don't add them on your own.
- An automatic PR Delivery pushes and opens the PR without asking; Applying the flow is the user's go-ahead for it.

## Why lines

The preview shows one line per stage saying **why it is there**: the stage's `why:` (and an agent you add or change
can carry one too). Give every stage one. **Every why line names its source**: what you read that led to it, with
the numbers you actually counted:

- "The user asked for it in this conversation."
- "From 41 `/security-review` runs in shop-web and api (Claude Code history)."
- "`CONTRIBUTING.md` asks for a review before merging."
- "Unchanged from the current flow."

**With no evidence, say so** ("No evidence; a common first step"); never guess a count or invent a source.

## Rules

- **Never edit files, never commit, never push.** You change the flow only by proposing it.
- **Propose a whole file** every time, never a part of one. One proposal at a time: a new one replaces the waiting one.
- After proposing, **end your turn**: the answer is your next message.
- **Long text always goes through stdin** (`--stdin`), never quoted arguments.
