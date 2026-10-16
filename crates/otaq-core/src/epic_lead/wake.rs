//! Wake-ups and the hand-back (LEAD-04, LEAD-12; d0083).
//!
//! - **Wake-ups:** [`EpicLeads::wake`] types an automatic message into a **live** lead (a `LeadWake` delivery, typed
//!   by the §9.3 rules: only when it is idle and its input line is empty) and sets the reason of the turn it starts,
//!   for the Lead Status. With no live lead nothing is queued. While the plan's usage limit holds the message waits,
//!   and Resume all types it. The board calls it when a child **merges into the epic branch** ("SHOP-40.1 merged:
//!   Wishlist API") or **is closed** ("SHOP-40.6 closed without merging: Wishlist export (CSV)"); questions (T-0284)
//!   and first stops (T-0285) call it too. Nothing else wakes the lead.
//! - **The board's side** ([`LeadEvents`], set on the [`crate::board::Board`] through its [`LeadLink`]): a whole
//!   epic queued starts its lead when it has none; a child merged or closed wakes it; the epic reaching Ready for
//!   Review ends it; a child added or reopened after that resumes it with no message.
//! - **Handovers** (the frame T-0284 and T-0285 build on): a question or a first stop sent to the lead is registered
//!   with [`EpicLeads::wake_with_handover`]. It belongs to the lead's turn its wake-up was typed in. When that turn
//!   ends (Stop) and its owner hasn't settled it ([`EpicLeads::settle_handover`]), or when the lead ends while it is
//!   open, it is **handed back** through the callback its owner registered with [`EpicLeads::on_hand_back`], which
//!   sends it to the user as if no lead were live. Handovers live in memory: a restart ends the lead, and the
//!   owners' own rows say what still waits.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::board::tickets::Ticket;
use crate::board::{BoardError, Refusal};
use crate::domain::{Actor, TicketKey};
use crate::epic_lead::stuck::FirstStop;
use crate::project_agents::lead::{EpicLeads, TurnReason, Wake};
use crate::session::deliveries::DeliveryKind;
use crate::store::new_id;

/// "SHOP-40.1 merged: Wishlist API": the wake-up after a child merged into the epic branch.
pub fn merged_text(child: &TicketKey, title: &str) -> String {
    format!("{child} merged: {}", title.trim())
}

/// "SHOP-40.6 closed without merging: Wishlist export (CSV)": the wake-up after a child was closed.
pub fn closed_text(child: &TicketKey, title: &str) -> String {
    format!("{child} closed without merging: {}", title.trim())
}

/// How a child finished, for its wake-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finished {
    /// Merged into the epic branch: Local merge, Auto-merge, or Mark merged on its PR.
    Merged,
    /// Closed without merging.
    Closed,
}

type Fut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What the board tells the epic leads (d0083). Each call logs its own failures: a lead never fails a board change.
pub trait LeadEvents: Send + Sync {
    /// A whole epic was queued (EPC-12): an epic with no lead at all gets one that checks the plan. Whether one
    /// started.
    fn epic_queued<'a>(&'a self, epic: &'a TicketKey) -> Fut<'a, bool>;
    /// An epic child merged into the epic branch or was closed: its lead wakes up.
    fn child_finished<'a>(&'a self, child: &'a Ticket, how: Finished) -> Fut<'a, ()>;
    /// The epic moved to Ready for Review: its lead ends.
    fn epic_in_review<'a>(&'a self, epic: &'a TicketKey, actor: &'a Actor) -> Fut<'a, ()>;
    /// A child was added to, or reopened in, an epic that had reached Review: its lead (ended there) resumes with
    /// no message.
    fn epic_back<'a>(&'a self, epic: &'a TicketKey) -> Fut<'a, ()>;
    /// Whether the child's next stop would go to its lead (T-0285): a live lead, and the child's attempt unused.
    fn takes_stop<'a>(&'a self, child: &'a Ticket) -> Fut<'a, bool>;
    /// The child reached a stop (T-0285): whether its live lead took it as the child's first stop, which is then
    /// held ([`crate::epic_lead::stuck::hold`]).
    fn child_stuck<'a>(&'a self, child: &'a Ticket, stop: &'a FirstStop) -> Fut<'a, bool>;
}

/// The board's link to the epic leads, set once the service has them. Without it (a board on its own) the calls
/// do nothing. Cheap to clone.
#[derive(Clone, Default)]
pub struct LeadLink(Arc<Mutex<Option<Arc<dyn LeadEvents>>>>);

impl std::fmt::Debug for LeadLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LeadLink")
    }
}

impl LeadLink {
    pub fn set(&self, events: Arc<dyn LeadEvents>) {
        *self.0.lock().expect("lead link") = Some(events);
    }

    fn get(&self) -> Option<Arc<dyn LeadEvents>> {
        self.0.lock().expect("lead link").clone()
    }

    /// See [`LeadEvents::epic_queued`]; false without leads.
    pub async fn epic_queued(&self, epic: &TicketKey) -> bool {
        match self.get() {
            Some(events) => events.epic_queued(epic).await,
            None => false,
        }
    }

    /// See [`LeadEvents::child_finished`]; only for an epic child.
    pub async fn child_finished(&self, child: &Ticket, how: Finished) {
        if child.epic.is_none() {
            return;
        }
        if let Some(events) = self.get() {
            events.child_finished(child, how).await;
        }
    }

    /// See [`LeadEvents::epic_in_review`].
    pub async fn epic_in_review(&self, epic: &TicketKey, actor: &Actor) {
        if let Some(events) = self.get() {
            events.epic_in_review(epic, actor).await;
        }
    }

    /// See [`LeadEvents::epic_back`].
    pub async fn epic_back(&self, epic: &TicketKey) {
        if let Some(events) = self.get() {
            events.epic_back(epic).await;
        }
    }

    /// See [`LeadEvents::takes_stop`]; false without leads and for a ticket outside an epic.
    pub async fn takes_stop(&self, child: &Ticket) -> bool {
        if child.epic.is_none() {
            return false;
        }
        match self.get() {
            Some(events) => events.takes_stop(child).await,
            None => false,
        }
    }

    /// See [`LeadEvents::child_stuck`]; false without leads and for a ticket outside an epic.
    pub async fn child_stuck(&self, child: &Ticket, stop: &FirstStop) -> bool {
        if child.epic.is_none() {
            return false;
        }
        match self.get() {
            Some(events) => events.child_stuck(child, stop).await,
            None => false,
        }
    }
}

/// A question or a first stop sent to a lead, waiting for it to act (T-0284, T-0285).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handover {
    pub id: String,
    /// Whose it is: the owner's callback hands it back ("question", "stuck").
    pub owner: &'static str,
    /// The child it is about.
    pub child: TicketKey,
    /// The owner's own reference (the question's id, …): what [`EpicLeads::settle_handover`] names.
    pub reference: String,
    /// The epic, and its lead's Project agent id.
    pub epic: TicketKey,
    pub lead: String,
    /// The wake-up text it was typed with.
    text: String,
    /// The lead's turn its wake-up was typed in; `None` until that turn starts.
    turn: Option<u64>,
}

/// Called with each handover handed back: its owner sends it to the user as if no lead were live.
pub type HandBack = Arc<dyn Fn(Handover) -> Fut<'static, ()> + Send + Sync>;

/// The open handovers of every lead, and the owners' hand-back callbacks. Cheap to clone.
#[derive(Clone, Default)]
pub struct Handovers {
    inner: Arc<Mutex<HandoversInner>>,
}

#[derive(Default)]
struct HandoversInner {
    /// By lead (Project agent id), oldest first.
    open: HashMap<String, Vec<Handover>>,
    /// Each lead's turn count, from its turns' Working hooks.
    turns: HashMap<String, u64>,
    callbacks: HashMap<&'static str, HandBack>,
}

impl Handovers {
    fn lock(&self) -> std::sync::MutexGuard<'_, HandoversInner> {
        self.inner.lock().expect("handovers")
    }

    fn register(&self, handover: Handover) {
        self.lock()
            .open
            .entry(handover.lead.clone())
            .or_default()
            .push(handover);
    }

    /// The lead `lead`'s turn started: its handovers whose wake-up was typed (no longer waiting in `pending`) belong
    /// to this turn.
    pub(crate) fn turn_started(&self, lead: &str, pending: &[String]) {
        let mut inner = self.lock();
        let turn = {
            let n = inner.turns.entry(lead.to_owned()).or_default();
            *n += 1;
            *n
        };
        if let Some(open) = inner.open.get_mut(lead) {
            for h in open
                .iter_mut()
                .filter(|h| h.turn.is_none() && !pending.contains(&h.text))
            {
                h.turn = Some(turn);
            }
        }
    }

    /// The lead `lead`'s turn ended: the handovers of that turn it didn't settle, taken out.
    pub(crate) fn turn_ended(&self, lead: &str) -> Vec<Handover> {
        let mut inner = self.lock();
        let turn = inner.turns.get(lead).copied();
        let Some(open) = inner.open.get_mut(lead) else {
            return Vec::new();
        };
        let (back, keep) = std::mem::take(open)
            .into_iter()
            .partition(|h| h.turn.is_some() && h.turn == turn);
        *open = keep;
        back
    }

    /// The lead `lead` ended: every open handover, taken out.
    pub(crate) fn lead_ended(&self, lead: &str) -> Vec<Handover> {
        self.lock().open.remove(lead).unwrap_or_default()
    }

    fn settle(&self, owner: &str, reference: &str) -> bool {
        let mut inner = self.lock();
        for open in inner.open.values_mut() {
            if let Some(i) = open
                .iter()
                .position(|h| h.owner == owner && h.reference == reference)
            {
                open.remove(i);
                return true;
            }
        }
        false
    }

    fn of(&self, lead: &str) -> Vec<Handover> {
        self.lock().open.get(lead).cloned().unwrap_or_default()
    }

    /// Hands each handover back through its owner's callback.
    pub(crate) async fn hand_back(&self, handovers: Vec<Handover>) {
        for handover in handovers {
            let callback = self.lock().callbacks.get(handover.owner).cloned();
            tracing::info!(
                owner = handover.owner, child = %handover.child, epic = %handover.epic,
                "handed back by the lead"
            );
            match callback {
                Some(callback) => callback(handover).await,
                None => tracing::warn!(
                    owner = handover.owner,
                    "no hand-back registered for this handover"
                ),
            }
        }
    }
}

impl EpicLeads {
    /// Wakes the epic's lead (LEAD-04): `text` is queued as a `LeadWake` delivery on its live session, typed once
    /// it is idle with an empty input line, and the turn it starts gets `wake` as its reason. With no live lead
    /// nothing is queued and the reason is dropped. While the usage limit holds, the message waits for Resume all.
    /// Whether it was queued.
    pub async fn wake(&self, epic: &str, wake: Wake, text: &str) -> Result<bool, BoardError> {
        Ok(self.queue_wake(epic, wake, text).await?.is_some())
    }

    /// Wakes the lead like [`wake`](Self::wake) and registers a handover for `child` (T-0284, T-0285), owned by
    /// `owner` with the owner's `reference`. Its id, or `None` when no lead is live: the caller then sends it to
    /// the user as today.
    pub async fn wake_with_handover(
        &self,
        epic: &str,
        wake: Wake,
        text: &str,
        owner: &'static str,
        child: &TicketKey,
        reference: &str,
    ) -> Result<Option<String>, BoardError> {
        // Registered before it is typed, so the turn it starts finds it.
        let Some(lead) = self.live_lead(epic).await? else {
            return Ok(None);
        };
        let handover = Handover {
            id: new_id(),
            owner,
            child: child.clone(),
            reference: reference.to_owned(),
            epic: lead.epic.clone(),
            lead: lead.agent.id.clone(),
            text: text.to_owned(),
            turn: None,
        };
        let id = handover.id.clone();
        self.handovers().register(handover);
        if self.queue_wake(epic, wake, text).await?.is_none() {
            self.handovers().settle(owner, reference);
            return Ok(None);
        }
        Ok(Some(id))
    }

    /// The lead acted on a handover (answered, passed on, sent a note): it is no longer handed back. Whether it
    /// was open.
    pub fn settle_handover(&self, owner: &str, reference: &str) -> bool {
        self.handovers().settle(owner, reference)
    }

    /// Registers what hands back `owner`'s handovers (once per owner, at service start).
    pub fn on_hand_back(&self, owner: &'static str, callback: HandBack) {
        self.handovers().lock().callbacks.insert(owner, callback);
    }

    /// The open handovers of the epic's lead, oldest first.
    pub async fn handovers_of(&self, epic: &str) -> Result<Vec<Handover>, BoardError> {
        Ok(match self.get(epic).await? {
            Some(lead) => self.handovers().of(&lead.agent.id),
            None => Vec::new(),
        })
    }

    /// Types the wake-ups held during the usage limit into every live lead that is idle (Resume all).
    pub async fn deliver_held(&self) -> Result<(), BoardError> {
        for session in self.live_sessions().await? {
            if let Err(e) = self.deliveries().deliver_if_ready(&session).await {
                tracing::warn!(session, error = %e, "can't type the lead's wake-up");
            }
        }
        Ok(())
    }

    /// Queues the wake-up on the live lead's session: its session id, or `None` with no live lead.
    async fn queue_wake(
        &self,
        epic: &str,
        wake: Wake,
        text: &str,
    ) -> Result<Option<String>, BoardError> {
        let Some(lead) = self.live_lead(epic).await? else {
            tracing::debug!(epic, "no live lead to wake");
            return Ok(None);
        };
        let session = lead
            .agent
            .session
            .as_ref()
            .map(|s| s.id.clone())
            .expect("a live lead has a session");
        self.set_turn_reason(epic, TurnReason::Wake { wake })
            .await?;
        self.set_next_woke(&lead.agent.id, text);
        let deliveries = self.deliveries();
        let queued = if self.limit_held_now().await? {
            deliveries
                .hold_delivery(&session, DeliveryKind::LeadWake, text)
                .await
        } else {
            deliveries
                .queue_delivery(&session, DeliveryKind::LeadWake, text)
                .await
        };
        queued.map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        tracing::info!(epic = %lead.epic, "lead woken");
        Ok(Some(session))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handover(id: &str, text: &str) -> Handover {
        Handover {
            id: id.into(),
            owner: "dummy",
            child: "SHOP-40.2".parse().unwrap(),
            reference: id.into(),
            epic: "SHOP-40".parse().unwrap(),
            lead: "lead".into(),
            text: text.into(),
            turn: None,
        }
    }

    #[test]
    fn a_handover_belongs_to_the_turn_its_wake_up_was_typed_in() {
        let h = Handovers::default();
        // Registered while the lead is busy: its wake-up waits.
        h.turn_started("lead", &[]);
        h.register(handover("7", "asks 7"));
        assert!(h.turn_ended("lead").is_empty(), "not typed in that turn");
        // The user's own turn starts before the wake-up is typed: still not this turn.
        h.turn_started("lead", &["asks 7".to_owned()]);
        assert!(h.turn_ended("lead").is_empty());
        // Typed: the next turn is its own, and its end hands it back.
        h.turn_started("lead", &[]);
        let back = h.turn_ended("lead");
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].reference, "7");
        assert!(h.of("lead").is_empty());
    }

    #[test]
    fn a_settled_handover_isnt_handed_back_and_the_lead_ending_takes_the_rest() {
        let h = Handovers::default();
        h.register(handover("7", "asks 7"));
        h.register(handover("8", "asks 8"));
        h.turn_started("lead", &[]);
        assert!(h.settle("dummy", "7"));
        assert!(!h.settle("dummy", "7"), "once");
        assert!(!h.settle("other", "8"), "the owner's own");
        let back = h.turn_ended("lead");
        assert_eq!(
            back.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
            ["8"]
        );
        h.register(handover("9", "asks 9"));
        assert_eq!(h.lead_ended("lead").len(), 1);
        assert!(h.lead_ended("lead").is_empty());
    }

    #[test]
    fn wake_up_texts() {
        let key = |k: &str| k.parse::<TicketKey>().unwrap();
        assert_eq!(
            merged_text(&key("SHOP-40.1"), "Wishlist API "),
            "SHOP-40.1 merged: Wishlist API"
        );
        assert_eq!(
            closed_text(&key("SHOP-40.6"), "Wishlist export (CSV)"),
            "SHOP-40.6 closed without merging: Wishlist export (CSV)"
        );
    }
}
