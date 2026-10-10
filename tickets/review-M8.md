# M8 · Usage — review

## 1. What was built

| Ticket | Title | Commit |
|---|---|---|
| T-0125 | Token usage reader (read-only) | b3e6d0d |
| T-0126 | Tokens in header, cards and panel; dollar estimate | 8bae0a0 |
| T-0127 | Transparent status-line command and launch `--settings` | 1c4b6e0 |
| T-0128 | Limit detection (StopFailure) and reset time | 71d4e6b |
| T-0129 | Plan-limit hold, no inbox items, Resume all → continue | 71d4e6b |
| T-0130 | Plan meter in the header | 67459fd |
| T-0131 | Usage-limit banner | 6a7f75c |
| T-0132 | Safety net: "This is the usage limit" | 5f22778 |

T-0128 and T-0129 share one commit: the plan-limit component (`usage::limit::PlanLimit`) holds the hit, the reset
bookkeeping and Resume all together, and splitting it would have left a commit with half a component.

No test flaked in this milestone. From T-0127 on, each ticket's checks ran in a separate git worktree with its own
build folder (in the session's scratch folder) on exactly the staged files, so work on the next ticket never
mixed into a check.

## 2. How to try it

```sh
scripts/check.sh
cargo build --workspace
export CREWBOARD_HOME=$(mktemp -d)                 # a short path: the socket lives in it
export CLAUDE_CONFIG_DIR=$CREWBOARD_HOME/claude    # a stand-in for ~/.claude: the service reads conversations here
mkdir -p $CLAUDE_CONFIG_DIR $CREWBOARD_HOME/repo && (cd $CREWBOARD_HOME/repo && git init -q -b main && echo a > a.txt && git add . && git commit -qm init)
B=$PWD/target/debug/crewboard
PATH=/usr/bin:/bin $B daemon start
echo '{"protocol_version":1,"id":1,"request":{"method":"project.add","params":{"path":"'$CREWBOARD_HOME/repo'"}}}' | nc -U -w 1 $CREWBOARD_HOME/crewboard.sock
for t in "Price table" "Coupon codes" "Order email"; do $B -P REPO create "$t" --done-when works; done
```

Settings → Notifications can be turned off first if OS notifications are unwanted while trying.

**Tokens (T-0125, T-0126).** A conversation for REPO-1 in the stand-in Claude folder, then the service reads it
(within a minute, or at once when a session starts or ends):

```sh
ID=0199a1b2-3c4d-7e5f-8a9b-0c1d2e3f4a5b
sqlite3 $CREWBOARD_HOME/crewboard.db "
  UPDATE tickets SET state='working', board_column='dev' WHERE number=1;
  INSERT INTO sessions (id, project_id, ticket_id, kind, claude_session_id, state, started_at, ended_at)
    SELECT 's1', project_id, id, 'agent', '$ID', 'ended', 0, 1 FROM tickets WHERE number=1;"
mkdir -p $CLAUDE_CONFIG_DIR/projects/x
NOW=$(date -u +%Y-%m-%dT%H:%M:%S.000Z)
echo '{"type":"assistant","timestamp":"'$NOW'","message":{"id":"m1","model":"claude-opus-5-5","usage":{"input_tokens":400,"output_tokens":2000,"cache_creation_input_tokens":180000,"cache_read_input_tokens":4000000,"cache_creation":{"ephemeral_1h_input_tokens":180000}}}}' > $CLAUDE_CONFIG_DIR/projects/x/$ID.jsonl
PATH=/usr/bin:/bin target/debug/crewboard-app
```

The header shows "182k today" (coins icon), REPO-1's card and panel header "182k", its History row "182k". Settings →
Usage: "Token count · Always on" and the Dollar estimate toggle; turned on, the header reads "182k tokens · ≈ $2.28
today" with "the $ figure is an estimate" on hover. Point a session at a missing conversation id (another `INSERT`
with a fresh id and no file) to see "—" with its tooltip.

**Plan meter (T-0127, T-0130).** The status line command records what Claude Code would pass it (no service start,
the user's own status line runs; here there is none, so it prints nothing):

```sh
R5=$(( $(date +%s) + 80*60 )); R7=$(( $(date +%s) + 3*86400 ))
echo '{"cwd":"'$CREWBOARD_HOME/repo'","rate_limits":{"five_hour":{"used_percentage":82,"resets_at":'$R5'},"seven_day":{"used_percentage":40,"resets_at":'$R7'}}}' | $B statusline
```

The header shows "5h 82% · week 40%", 82 % in amber; hover shows both reset times and "New tickets hold at 100% ·
nothing resumes by itself". With a status line in `$CLAUDE_CONFIG_DIR/settings.json` (`{"statusLine":
{"type":"command","command":"cat"}}`), the same command prints its input back unchanged.

**Usage limit (T-0128, T-0129, T-0131).** Without claude the hit can be written as the hook would leave it, then any
status-line update tells the app:

```sh
sqlite3 $CREWBOARD_HOME/crewboard.db "
  UPDATE tickets SET state='waiting_for_limit_reset', board_column='dev' WHERE number IN (1, 2);
  UPDATE tickets SET state='queued', board_column='dev', queued_at=1 WHERE number=3;
  INSERT INTO plan_limits (id, limit_active, limit_kind, limit_text, limit_hit_at, limit_resets_at, limit_reset_source, observed_at)
    VALUES ('l1', 1, 'session', 'You''ve hit your session limit · resets 3:45pm', $(date +%s)000, $(( $(date +%s) + 50*60 ))000, 'status_line', $(date +%s)001);"
echo '{"rate_limits":{"five_hour":{"used_percentage":100,"resets_at":'$(( $(date +%s) + 50*60 ))'}}}' | $B statusline
```

The banner "Session limit reached · Resets in 50 min (…) · 2 agents stopped · queued tickets wait" with Resume all
disabled; REPO-1 and REPO-2 show the neutral "Waiting for limit reset"; REPO-3 is Held "Usage limit reached · starts
once it resets" (no Retry); the inbox shows the limit row and no items for them. Changing `limit_resets_at` to the past
(and another status-line update) switches the banner to "Usage limit reset · … 2 agents are waiting · nothing resumes
until you click" with "Resume all (2)" highlighted, and REPO-3 starts on the next scheduler pass (the service was
started with `PATH=/usr/bin:/bin`, so it stops at the launch step: Start interrupted, no claude). With `limit_resets_at` NULL: "Reset time unknown · …", Resume all enabled. Resume all needs
live agent sessions to type "continue" into; without them it only clears the banner.

**Safety net (T-0132).** `UPDATE tickets SET state='idle' WHERE number=1;` (and one in `asks_you`), then the inbox's
Idle row has "This is the usage limit"; the click turns every Idle ticket into Waiting for limit reset (History:
"Marked as usage limit · waiting for limit reset · by you"), their rows leave, the banner says "Reset time unknown".

**With the real claude (once):** an agent session's own status line, the plan meter filling from it, a real limit hit
(the hook payload and wording), Claude Code's automatic continue after the reset (Q27).

**Cleaning up:** `$B daemon stop`, then remove `$CREWBOARD_HOME`.

## 3. Hand checks

Every frame is in `crewboard.pen`. "Light/dark": compare once in each appearance.

**T-0125 · Usage fields of the installed Claude Code**
- [ ] For a real agent session, the card's count grows as the agent works and matches input + output + cache writes of
  `~/.claude/projects/<folder>/<session id>.jsonl` (and its `subagents/` files); a resumed conversation's tokens go
  to the session that wrote them in History.

**T-0126 · Tokens**
- [ ] `02 · App shell & project switcher → Header states → Header · dollar estimate enabled` (and the plain header in
  `Header · plan meter (Pro / Max)`: "1.84M today"): coins icon, mono number, muted label; with the estimate on
  "tokens · ≈ $38.20 today", the hover says it is an estimate.
- [ ] `15 · Settings → Global · Usage, notifications, data, appearance` → Usage: "Token count · Always on" and the
  Dollar estimate toggle, off by default; it switches the header at once.
- [ ] Tokens on cards (`04 · Ticket card variants`, e.g. "active 40 s ago · 182k tokens" rows), the panel header
  (`05 · Ticket side panel`) and History's TOKENS column (`05`, `16 → History · Claude session IDs`); "—" with the
  tooltip for an unreadable conversation. Light/dark.

**T-0127 · Status line**
- [ ] With the real claude: an agent session shows the user's own status line exactly as a plain `claude` does
  (colours, several lines, padding); `claude --debug` shows no status-line error; the user's settings files are
  unchanged; the plan meter fills after the first response (Pro/Max).

**T-0128 · Limit detection**
- [ ] With the real claude at a real limit: the StopFailure hook arrives with `error: rate_limit` and the limit text
  (the service logs it at debug level), the card turns "Waiting for limit reset", the banner appears with the right
  reset time.

**T-0129 · Hold and continue**
- [ ] After the reset, agents Claude Code continued by itself go back to Working without a "continue" from Crewboard
  (Q27); Resume all types "continue" only into those still waiting, once their input line is empty.

**T-0130 · Plan meter**
- [ ] `02 → Header states → Header · plan meter (Pro / Max)` and its hover (`Header · plan meter hover`): "5h 82% ·
  week 40%", amber 82 %, the box, the tooltip's three lines. Light/dark. Known difference: the tooltip title is
  "Plan usage" (the frame's "· Claude Max" isn't known to Crewboard).

**T-0131 · Banner**
- [ ] `11 · Banners, toasts & OS notifications → Banners → Usage limit · just hit`, `· waiting for reset`, `· after
  reset`, `· weekly`, `· reset time unknown`, `Stacking · usage limit + reconnecting`: title, text, Resume all
  (disabled before a known reset, primary "Resume all (8)" after it), countdown in the last hour; no Retry and no
  Request more (USG-08; the enterprise frames are after v0.1). Known difference: the frames' second "Retry" button
  isn't built (USG-08).
- [ ] `04 · Ticket card variants → Dev · waiting for limit reset` (light/dark): neutral pill, not orange.
- [ ] `10 · Needs-you inbox → Usage limit active · no duplicates`: the limit row at the top, no items for stopped
  agents; after the reset the row has Resume all.

**T-0132 · Safety net**
- [ ] `10 → Row · Idle, after reboot → Idle · "This is the usage limit" (safety net, 0020)` and `After "This is the usage
  limit"`: the button on Idle rows only (not on Asks you / Needs you), one click folds them into the banner.
  Known difference: the frame's "Open terminal" button on the Idle row isn't part of this ticket.

Earlier milestones' hand checks stay as listed in review-M1…M7.

## 4. Implementation choices (SDD §26)

- **Usage reader** (`usage::Usage`): reads `<CLAUDE_CONFIG_DIR or ~/.claude>/projects/<folder>/<id>.jsonl` and its
  `<id>/subagents/*.jsonl` read-only, incrementally (byte offset, complete lines only), on a 60 s tick and 1 s after
  a session or card status change; counts per API response id (a response with several blocks repeats its usage);
  responses go to the Crewboard session running when they were written (a resumed conversation shares its Claude
  session ID) and to the local day; `usage` rows hold absolute totals. Unknown = missing, unreadable or unrecognised
  file: one warning, "—" in the app. Finished conversations are read once more, then left; after a restart those
  ended over a day ago with counts aren't read again.
- **Shown number** (Q26): input + output + cache writes (`Tokens::shown`); all four counts are stored.
- **Dollar estimate**: priced per response by model when read (`usage::pricing`, the one price table; first-party API
  prices from the Claude API reference bundled with Claude Code 2.1.296; 5-min writes 1.25×, 1-h writes 2× input;
  per-model cache reads); migration 0015 `usage.cost_micro_usd`; unknown models add nothing.
- **Status line**: `crewboard statusline` (hidden); per-ticket settings file `<data>/claude/<KEY>.settings.json`
  with only `statusLine` (plus the user's padding and refreshInterval); the user's command found read-only (local,
  project, user settings) and run with `/bin/sh -c`, stdin the same bytes, stdout/stderr inherited; 500 ms for the
  service; `CREWBOARD_STATUSLINE` guards against a loop. `plan_limits` gets a row only when the windows change.
- **Limit**: `Signal::LimitReached` for `rate_limit` + "hit your … limit"; kinds session / weekly / other; reset from
  the status line → the text ("3:45pm", "Mon 12:00am", "3pm (zone)") → unknown; migration 0016 adds the limit columns
  to `plan_limits`; reset bookkeeping every 30 s and on status-line updates. Waiting for limit reset joined
  `signals::LIVE` (a new turn or command makes it Working).
- **Hold**: tickets that would start become Held with `StartStep::PlanLimit` and "Usage limit reached · starts once it
  resets"; only the scheduler lifts that hold (after the reset or Resume all). Tickets waiting for parents keep their
  text.
- **Inbox**: suppression on at the first hit, stopped tickets marked; marks cleared when an agent works again or by
  Resume all; restored at service start while a limit is active.
- **Resume all** (`usage.resume_all`, user only): "continue" as a `continue_after_limit` delivery for each agent still
  waiting; clears the limit. The banner hides after a reset when no agent is waiting any more (Q27).
- **Safety net** (`usage.mark_limit`, user only): every Idle ticket → Waiting for limit reset, History event
  `limit_marked` by the user; the limit's kind and reset from a full status-line window if there is one, else unknown.
- **App**: header usage item and plan meter (`shell.rs`), `view_models::usage`, `plan_meter`, `limit_banner`;
  `Tooltip::with_lines`; `BannerAction::primary`; the banner recomputed every 500 ms tick for its countdown; the
  inbox's limit row; Held-by-limit cards have no Retry.
- **New protocol**: `usage.summary {project?}`, `usage.sessions {key}`, `usage.record_limits`, `usage.resume_all`,
  `usage.mark_limit`; event `usage_changed`. Migrations 0015, 0016. No new crates.

## 5. Open questions

Added to `tickets/questions.md`, both deferred:
- **Q26** — which tokens the one number adds up (cache reads dwarf everything else). Built as option 1 (input +
  output + cache writes).
- **Q27** — Claude Code ≥ 2.1.234 continues by itself when a usage limit resets (the user's
  `autoContinueAtUsageLimit`, on by default); d0020 says nothing restarts without Resume all. Built as option 1
  (respect the user's Claude Code setting; Crewboard itself still starts nothing; Resume all covers the agents still
  waiting).

Notes for later milestones:
1. **M9 Project terminal** (T-0138, T-0167): a Claude session there with a stored Claude session ID is read like any
   other (today's total counts it; its ticket-less rows don't show on cards). "claude exited · 38k tokens" can come
   from `usage.sessions`-style data per session.
2. **M10 answer and delivery sessions** count toward their ticket automatically (any session with a Claude session
   ID); the status-line `--settings` file is added by the start sequence only, so M10's answer/delivery launches
   should pass `statusline::write_settings` too.
3. **M11 Windows/Linux**: `usage::local_day`, `usage::limit::next_local` and the plan-meter clock use `libc`
   `localtime_r`/`mktime`; the status line runs the user's command with `/bin/sh -c` (Claude Code uses Git Bash or
   PowerShell on Windows).
