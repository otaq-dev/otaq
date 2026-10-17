//! Limits per tool (USG-10, USG-06, USG-07, AGT-08; d0086, d0020, d0046, d0018; T-0358): the banner's data, one
//! line per tool ([`ToolLine`]), **Resume** per tool (`limits.resume_tool`) and **Move waiting to…**
//! (`limits.move_waiting`).
//!
//! A tool gets a line while its limit is active (session / weekly / another limit, reset known or not), and while
//! it isn't available (logged out, not installed, hidden, update needed) with tickets waiting on it (a held ticket
//! with reason `tool_unavailable`) or moved off it since it was last available ("Codex isn't available · logged out
//! · 5 tickets moved to Sonnet", with the automatic fallback on); a tool both at its limit and unavailable gets
//! both. Each line says
//! how many agents the limit stopped, how many tickets wait on the tool (their holds), how many moved off it since
//! the line started and to which agents ("5 tickets moved to Sonnet"), whether Resume can be pressed (after a known
//! reset, or always when the reset is unknown, USG-06), and the agents Move waiting to… offers: named agents on
//! **other** tools that can start now, none while only one tool is connected (it reads as M22's banner then).
//!
//! **Move waiting to…** changes the Developer row of every not-yet-started ticket held on the tool (a limit or an
//! unavailable tool; never a model) to the chosen agent, as the user's click (*set by you*), recording where it
//! moved from and why ([`super::RowMove`]); the gate then starts each one (the check runs again). Validator and lead
//! holds don't move (Q-C); running and parked agents never move.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use super::gate::{self, Facts, Held, LaunchGate, still_waits};
use super::{HoldReason, MoveReason, NamedAgent, Role, RowMove, RowSetBy, resolve_ticket_role};
use crate::agent_tools::Tool;
use crate::agent_tools::connected::{AgentToolState, ConnectedTools};
use crate::attention::Inbox;
use crate::board::tickets::{EventKind, record_event};
use crate::board::{Board, BoardError, Refusal};
use crate::domain::{Actor, Status, TicketKey};
use crate::protocol::Event;
use crate::service::EventBus;
use crate::usage::limit::{LimitKind, PlanLimit, matching_window};
use crate::usage::plan;

/// The banner's lines, in [`Tool::ALL`] order, and whether several tools are connected (with one, the banner reads
/// as before: no tool name, no Move waiting to…).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolLines {
    pub lines: Vec<ToolLine>,
    pub several_tools: bool,
}

/// One tool's banner line (T-0364 words it): "Claude Code limit reached · resets 3:45pm · 4 tickets waiting ·
/// Resume Claude Code · Move waiting to… ▾", "Codex isn't available · logged out · 5 tickets waiting · Sign in…".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolLine {
    pub tool: Tool,
    pub situation: Situation,
    /// Agents the limit stopped mid-turn (Waiting for limit reset; Project agents too). 0 for an unavailable tool.
    pub stopped: u32,
    /// Tickets that would start on the tool and wait (their holds for this situation's reason).
    pub waiting: u32,
    /// Tickets moved off the tool since the line started (the fallback, Use fallback, Move waiting to…), per agent
    /// they moved to: "5 tickets moved to Sonnet".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moved: Vec<MovedTo>,
    /// Resume can be pressed: the limit reset, or its reset is unknown (USG-06). Never for an unavailable tool.
    pub resume_enabled: bool,
    /// What Move waiting to… lists: agents on other tools that can start now; empty with one tool connected.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<MoveTarget>,
}

/// Why a tool has a line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "situation", rename_all = "snake_case")]
pub enum Situation {
    /// Its limit is active.
    Limit {
        kind: LimitKind,
        /// The length of the tool's window for that limit, when it reports one (a month for Copilot's quota).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        window_minutes: Option<u32>,
        /// When it resets; `None` = unknown (an API-key user has no windows).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resets_at: Option<i64>,
        /// The reset was seen (nothing resumes until the user's Resume).
        reset_seen: bool,
        hit_at: i64,
    },
    /// Tickets wait on it while it isn't available.
    Unavailable {
        /// "logged out", "not installed", "hidden in Settings", "update needed".
        reason: String,
        /// **Sign in…** applies (found, signed out, with a sign-in command).
        sign_in: bool,
        /// Hidden by the user: "Show in Agent tools" instead (Q-E).
        hidden: bool,
        since: i64,
    },
}

/// Where tickets moved to off a tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovedTo {
    pub agent_id: String,
    pub name: String,
    pub count: u32,
}

/// An agent Move waiting to… offers: "Strong GPT · Codex · GPT Sol · always the newest".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveTarget {
    pub id: String,
    pub name: String,
    pub tool: Tool,
    /// As stored (`None`: the tool's default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// The agents Move waiting to… offers for `tool`'s waiting tickets (pure): those on other tools that pass the check
/// before a launch (available, not at their limit, their model listed).
pub fn move_targets(tool: Tool, agents: &[NamedAgent], facts: &Facts) -> Vec<MoveTarget> {
    agents
        .iter()
        .filter(|a| a.tool != tool && gate::check(a, facts).is_ok())
        .map(|a| MoveTarget {
            id: a.id.clone(),
            name: a.name.clone(),
            tool: a.tool,
            model: a.model.clone(),
        })
        .collect()
}

/// A tool that isn't available while tickets wait on it, or moved off it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableSituation {
    pub tool: Tool,
    /// Tickets waiting (one per ticket, whatever its held roles).
    pub tickets: u32,
    /// Tickets moved off it since it stopped being available, per agent they moved to.
    pub moved: Vec<MovedTo>,
    /// The oldest hold's, or the first move's.
    pub since: i64,
}

/// The tickets each tool holds for `reason` that still wait: (tool → ticket ids, the oldest hold).
async fn waiting_for(
    board: &Board,
    reason: HoldReason,
) -> Result<BTreeMap<Tool, (Vec<String>, i64)>, BoardError> {
    let mut by_tool: BTreeMap<Tool, (Vec<String>, i64)> = BTreeMap::new();
    for hold in board.store().holds_for(reason).await? {
        let Some(id) = &hold.ticket_id else {
            continue;
        };
        let Ok(ticket) = board.ticket_by_id(id).await else {
            continue;
        };
        if !still_waits(&ticket, hold.role) {
            continue;
        }
        let (ids, since) = by_tool.entry(hold.tool).or_insert((Vec::new(), hold.since));
        if !ids.contains(id) {
            ids.push(id.clone());
        }
        *since = (*since).min(hold.since);
    }
    Ok(by_tool)
}

/// The tools that aren't available while tickets wait on them, or while tickets moved off them (the automatic
/// fallback, Use fallback, Move waiting to…: "Codex isn't available · logged out · 5 tickets moved to Sonnet"),
/// for the line and its one notification (USG-10). `unavailable` are the tools not available now; a tool's moves
/// count from `floor(tool)` on (when it was last seen available). A tool with held tickets has a situation even
/// when it was just seen available again (its holds are about to lift).
pub async fn unavailable_situations(
    board: &Board,
    unavailable: &HashMap<Tool, String>,
    floor: impl Fn(Tool) -> i64,
) -> Result<Vec<UnavailableSituation>, BoardError> {
    let held = waiting_for(board, HoldReason::ToolUnavailable).await?;
    let mut out = Vec::new();
    for &tool in Tool::ALL {
        let held = held.get(&tool);
        let (moved, first_move) = match unavailable.contains_key(&tool) {
            true => moved_since(board, tool, &[MoveReason::ToolUnavailable], floor(tool)).await?,
            false => (Vec::new(), None),
        };
        if held.is_none() && moved.is_empty() {
            continue;
        }
        let since = [held.map(|(_, since)| *since), first_move]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or_default();
        out.push(UnavailableSituation {
            tool,
            tickets: held.map_or(0, |(ids, _)| count(ids.len())),
            moved,
            since,
        });
    }
    Ok(out)
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// What the lines read: the service's parts.
#[derive(Clone, Copy)]
pub struct LineSources<'a> {
    pub board: &'a Board,
    pub gate: &'a LaunchGate,
    pub plan_limit: &'a PlanLimit,
    pub connected: &'a ConnectedTools,
    pub inbox: &'a Inbox,
}

/// The banner's lines now (USG-10).
pub async fn tool_lines(src: LineSources<'_>) -> Result<ToolLines, BoardError> {
    let store = src.board.store();
    let facts = src.gate.facts().await?;
    let agents = store.named_agents().await?;
    let several_tools = src.connected.connected().len() > 1;
    let limits = plan::limits(store).await?;
    let held_limit = waiting_for(src.board, HoldReason::ToolLimit).await?;
    let unavailable = unavailable_situations(src.board, &facts.unavailable, |tool| {
        src.inbox.available_floor(tool)
    })
    .await?;
    let targets = |tool| match several_tools {
        true => move_targets(tool, &agents, &facts),
        false => Vec::new(),
    };
    let mut lines = Vec::new();
    for &tool in Tool::ALL {
        if let Some(limit) = limits.iter().find(|l| l.tool == tool) {
            let windows = plan::windows(store, tool).await?;
            let window_minutes = matching_window(
                src.plan_limit.adapter(tool),
                limit.kind,
                windows.iter().map(|w| &w.window),
            )
            .and_then(|w| w.length_minutes);
            lines.push(ToolLine {
                tool,
                situation: Situation::Limit {
                    kind: limit.kind,
                    window_minutes,
                    resets_at: limit.resets_at,
                    reset_seen: limit.reset_seen_at.is_some(),
                    hit_at: limit.hit_at,
                },
                stopped: src.plan_limit.stopped(tool).await?,
                waiting: held_limit.get(&tool).map_or(0, |(ids, _)| count(ids.len())),
                moved: moved_since(src.board, tool, &BOTH, limit.hit_at).await?.0,
                resume_enabled: limit.resets_at.is_none() || limit.reset_seen_at.is_some(),
                targets: targets(tool),
            });
        }
        if let Some(situation) = unavailable.iter().find(|u| u.tool == tool) {
            let status = src.connected.status(tool);
            let reason = facts
                .unavailable
                .get(&tool)
                .cloned()
                .or_else(|| status.state.unavailable_reason().map(str::to_owned))
                .unwrap_or_else(|| "not available".to_owned());
            let since = src.inbox.unavailable_since(tool).unwrap_or(situation.since);
            lines.push(ToolLine {
                tool,
                situation: Situation::Unavailable {
                    reason,
                    sign_in: status.can_sign_in,
                    hidden: matches!(status.state, AgentToolState::Hidden { .. }),
                    since,
                },
                stopped: 0,
                waiting: situation.tickets,
                moved: situation.moved.clone(),
                resume_enabled: false,
                targets: targets(tool),
            });
        }
    }
    Ok(ToolLines {
        lines,
        several_tools,
    })
}

/// A limit line counts both kinds of move off its tool.
const BOTH: [MoveReason; 2] = [MoveReason::ToolLimit, MoveReason::ToolUnavailable];

/// The Developer rows moved off `tool` for one of `reasons` since `since`, per agent they moved to, and when the
/// first of them moved.
async fn moved_since(
    board: &Board,
    tool: Tool,
    reasons: &[MoveReason],
    since: i64,
) -> Result<(Vec<MovedTo>, Option<i64>), BoardError> {
    let rows: Vec<(String, String, String, i64, i64)> = sqlx::query_as(
        "SELECT r.named_agent_id, a.name, r.moved_reason, COUNT(*), MIN(r.moved_at) FROM ticket_agent_rows r
         JOIN named_agents a ON a.id = r.named_agent_id
         WHERE r.role = ? AND r.moved_tool = ? AND r.moved_at >= ?
         GROUP BY r.named_agent_id, a.name, r.moved_reason ORDER BY MIN(r.moved_at), a.name",
    )
    .bind(Role::Developer.as_str())
    .bind(tool.as_str())
    .bind(since)
    .fetch_all(board.store().pool())
    .await?;
    let mut moved: Vec<MovedTo> = Vec::new();
    let mut first: Option<i64> = None;
    for (agent_id, name, reason, n, at) in rows {
        if !reasons.iter().any(|r| r.as_str() == reason) {
            continue;
        }
        first = Some(first.map_or(at, |f| f.min(at)));
        let n = u32::try_from(n).unwrap_or(0);
        match moved.iter_mut().find(|m| m.agent_id == agent_id) {
            Some(m) => m.count += n,
            None => moved.push(MovedTo {
                agent_id,
                name,
                count: n,
            }),
        }
    }
    Ok((moved, first))
}

/// `limits.resume_tool`: **Resume `tool`** (the user's click): that tool's limit-stopped agents continue, its limit
/// clears, the validations that waited for a reset start (SCH-04, d0063; one whose validator's tool is still at
/// its limit is held again by its check), and the tool's holds lift (the held starts run, once). Returns the
/// tickets told to continue and those whose validator started, in key order.
pub async fn resume_tool(
    plan_limit: &PlanLimit,
    validation: &crate::validation::Validation,
    gate: &LaunchGate,
    tool: Tool,
) -> Result<Vec<TicketKey>, BoardError> {
    let mut resumed = plan_limit.resume_tool(tool).await?;
    let started = Box::pin(validation.start_waiting())
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "can't start the validations that waited"))
        .unwrap_or_default();
    for id in started {
        if let Ok(ticket) = plan_limit.board().ticket_by_id(&id).await {
            resumed.push(ticket.key);
        }
    }
    gate.lift().await?;
    resumed.sort();
    resumed.dedup();
    Ok(resumed)
}

/// `limits.move_waiting`: **Move waiting to… `agent`** for `tool`'s waiting tickets (AGT-08; the user's click).
/// Refused when `agent` is on the same tool or can't start now. Returns the tickets moved, in key order (the toast:
/// "4 tickets moved to Strong GPT").
pub async fn move_waiting(
    board: &Board,
    gate: &LaunchGate,
    events: &EventBus,
    tool: Tool,
    agent: &str,
) -> Result<Vec<TicketKey>, BoardError> {
    let store = board.store();
    let refused = |message: String| BoardError::refused(Refusal::MoveRefused, message);
    let target = store
        .named_agent(agent)
        .await?
        .ok_or_else(|| BoardError::NotFound(agent.to_owned()))?;
    if target.tool == tool {
        return Err(refused(format!(
            "{} runs on {}: pick an agent on another tool",
            target.name,
            tool.display_name()
        )));
    }
    if let Err(blocked) = gate::check(&target, &gate.facts().await?) {
        let why = Held {
            agent: target.clone(),
            blocked,
            fallback: None,
            fallback_held: false,
        }
        .message();
        return Err(refused(format!(
            "{} can\u{2019}t start: {why}",
            target.name
        )));
    }
    let mut moved = Vec::new();
    for hold in store.holds_on_tool(tool).await? {
        let reason = match hold.reason {
            HoldReason::ToolLimit => MoveReason::ToolLimit,
            HoldReason::ToolUnavailable => MoveReason::ToolUnavailable,
            HoldReason::ModelUnavailable => continue,
        };
        let Some(id) = hold
            .ticket_id
            .as_deref()
            .filter(|_| hold.role == Role::Developer)
        else {
            continue;
        };
        let ticket = board.ticket_by_id(id).await?;
        // Only a not-yet-started ticket held in Ready for Dev moves.
        if ticket.status != Status::Held
            || !still_waits(&ticket, Role::Developer)
            || store.developer_started(id).await?
        {
            continue;
        }
        let from = resolve_ticket_role(board, &ticket, Role::Developer).await?;
        let row_move = RowMove {
            from_id: from.as_ref().map(|r| r.agent.id.clone()),
            from_name: from
                .as_ref()
                .map_or_else(|| tool.display_name().to_owned(), |r| r.agent.name.clone()),
            reason,
            tool: Some(tool),
            at: crate::store::now_ms(),
        };
        store
            .move_ticket_agent_row(id, Role::Developer, &target.id, &RowSetBy::User, &row_move)
            .await?;
        // History says it as the gate's moves do: the user's click, set by you.
        let mut tx = store.begin_write().await?;
        record_event(
            &mut tx,
            id,
            EventKind::AgentRowSet,
            &Actor::current_user(),
            super::rows::row_move_event(
                Role::Developer,
                &target.name,
                target.tool,
                &row_move,
                &RowSetBy::User,
                true,
            ),
        )
        .await?;
        tx.commit().await?;
        events.publish(Event::TicketChanged {
            project: ticket.key.project.to_string(),
            id: ticket.id.clone(),
            key: ticket.key.clone(),
        });
        tracing::info!(key = %ticket.key, from = row_move.from_name, to = target.name, %tool, "moved waiting");
        moved.push(ticket.key);
    }
    moved.sort();
    // Each moved ticket starts through the gate (its check runs again on the new agent).
    if !moved.is_empty() {
        gate.lift().await?;
    }
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(id: &str, tool: Tool) -> NamedAgent {
        NamedAgent {
            id: id.into(),
            name: id.into(),
            tool,
            model: None,
            use_for: String::new(),
            fallback: None,
            mark: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn targets_are_agents_on_other_tools_that_can_start() {
        let [claude, codex, copilot, opencode] =
            [Tool::ALL[0], Tool::ALL[1], Tool::ALL[2], Tool::ALL[3]];
        let agents = [
            agent("strong", claude),
            agent("light", claude),
            agent("strong-gpt", codex),
            agent("verifier", copilot),
            agent("oc", opencode),
        ];
        let mut facts = Facts::default();
        facts.at_limit.insert(claude);
        facts
            .unavailable
            .insert(copilot, "hidden in Settings".into());
        facts.at_limit.insert(opencode);
        let names: Vec<String> = move_targets(claude, &agents, &facts)
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, ["strong-gpt"]);
        // For Codex's waiting tickets: Claude Code and OpenCode are at their limits, Copilot is hidden.
        assert!(move_targets(codex, &agents, &facts).is_empty());
    }
}
