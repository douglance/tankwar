//! Pure command validation. Persistence must complete before an acceptance is returned.
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use tankwar_protocol::{Command, Event, EventKind, MatchState, Order, PROTOCOL_VERSION};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat { pub agent_id: String, pub seat_id: String }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ledger {
    pub match_id: String,
    pub state: MatchState,
    pub tick: u64,
    pub version: u64,
    pub seats: Vec<Seat>,
    pub events: Vec<Event>,
    accepted: BTreeMap<String, Command>,
    #[serde(default)] per_seat_tick: BTreeMap<String, (u64, u32)>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision { Accept(Event), Duplicate(Event), Reject(&'static str) }

impl Ledger {
    pub fn new(match_id: String, seats: Vec<Seat>) -> Self {
        Self { match_id, state: MatchState::Assigned, tick: 0, version: 0, seats, events: Vec::new(), accepted: BTreeMap::new(), per_seat_tick: BTreeMap::new() }
    }
    /// Remove newly archived entries after the caller has prepared durable writes.
    pub fn take_history(&mut self) -> Vec<Event> {
        self.accepted.clear();
        std::mem::take(&mut self.events)
    }
    /// Validate a persisted idempotency record without exposing another seat's command.
    pub fn decide_archived(&self, agent_id: &str, command: &Command, prior: &Event) -> Decision {
        if !self.seats.iter().any(|s| s.agent_id == agent_id && s.seat_id == command.seat_id) { return Decision::Reject("unauthorized_seat"); }
        if command.match_id != self.match_id { return Decision::Reject("wrong_match"); }
        match &prior.event {
            EventKind::Accepted { command: previous } if previous == command => Decision::Duplicate(prior.clone()),
            EventKind::Accepted { .. } => Decision::Reject("command_id_conflict"),
            _ => Decision::Reject("corrupt_ledger"),
        }
    }
    /// This changes only an in-memory copy; the caller must persist it atomically.
    pub fn decide(&mut self, agent_id: &str, command: Command) -> Decision {
        if !self.seats.iter().any(|s| s.agent_id == agent_id && s.seat_id == command.seat_id) { return Decision::Reject("unauthorized_seat"); }
        if command.match_id != self.match_id { return Decision::Reject("wrong_match"); }
        if command.id.is_empty() || command.id.len() > 128 { return Decision::Reject("invalid_command_id"); }
        let key = command.id.clone();
        if let Some(previous) = self.accepted.get(&key) {
            if previous != &command { return Decision::Reject("command_id_conflict"); }
            return self.events.iter().find(|e| matches!(&e.event, EventKind::Accepted { command: c } if c == previous)).cloned().map(Decision::Duplicate).unwrap_or(Decision::Reject("corrupt_ledger"));
        }
        if command.protocol_version != PROTOCOL_VERSION { return Decision::Reject("protocol_version"); }
        if self.state != MatchState::Running { return Decision::Reject("match_not_running"); }
        if command.expected_version.is_some_and(|v| v != self.version) { return Decision::Reject("stale_version"); }
        if let Order::Drive { throttle, steering, ttl_ticks } = &command.order {
            if *ttl_ticks == 0 || *ttl_ticks > 200 || !(-1000..=1000).contains(throttle) || !(-1000..=1000).contains(steering) { return Decision::Reject("invalid_drive"); }
        }
        let used = self.per_seat_tick.get(&command.seat_id).filter(|(tick, _)| *tick == self.tick).map(|(_, count)| *count).unwrap_or(0);
        if used >= 4 { return Decision::Reject("rate_limited"); }
        self.per_seat_tick.insert(command.seat_id.clone(), (self.tick, used + 1));
        self.version += 1;
        let event = Event { sequence: self.version, tick: self.tick, event: EventKind::Accepted { command: command.clone() } };
        self.accepted.insert(key, command);
        self.events.push(event.clone());
        Decision::Accept(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn command() -> Command { Command { protocol_version: 1, id: "one".into(), match_id: "match".into(), seat_id: "seat".into(), expected_version: None, order: Order::Fire } }
    fn ledger() -> Ledger { let mut l = Ledger::new("match".into(), vec![Seat { agent_id: "agent".into(), seat_id: "seat".into() }]); l.state = MatchState::Running; l }
    #[test] fn identical_retries_and_conflicting_payloads() {
        let mut l = ledger(); let c = command();
        let accepted = match l.decide("agent", c.clone()) { Decision::Accept(e) => e, _ => panic!() };
        assert_eq!(l.decide("agent", c.clone()), Decision::Duplicate(accepted));
        let mut changed = c; changed.order = Order::Engine { running: true };
        assert_eq!(l.decide("agent", changed), Decision::Reject("command_id_conflict"));
        assert_eq!(l.events.len(), 1);
    }
    #[test] fn command_rate_limit_is_per_tick_and_retries_remain_idempotent() {
        let mut l = ledger();
        let mut first = command();
        let accepted = match l.decide("agent", first.clone()) { Decision::Accept(e) => e, _ => panic!() };
        for id in ["two", "three", "four"] { let mut next=first.clone(); next.id=id.into(); assert!(matches!(l.decide("agent",next),Decision::Accept(_))); }
        assert_eq!(l.decide("agent", first.clone()), Decision::Duplicate(accepted));
        first.id="five".into();
        assert_eq!(l.decide("agent", first.clone()), Decision::Reject("rate_limited"));
        l.tick += 1;
        assert!(matches!(l.decide("agent", first), Decision::Accept(_)));
    }
    #[test] fn same_id_on_another_seat_is_a_conflict() {
        let mut l = ledger();
        l.seats.push(Seat { agent_id: "other".into(), seat_id: "other-seat".into() });
        let mut c = command();
        assert!(matches!(l.decide("agent", c.clone()), Decision::Accept(_)));
        c.seat_id = "other-seat".into();
        assert_eq!(l.decide("other", c), Decision::Reject("command_id_conflict"));
    }
    #[test] fn seat_auth_is_checked_before_duplicate_lookup() {
        let mut l = ledger(); let c = command(); l.decide("agent", c.clone());
        assert_eq!(l.decide("attacker", c), Decision::Reject("unauthorized_seat"));
    }
    #[test] fn stale_and_invalid_orders_do_not_mutate() {
        let mut l = ledger(); let mut c = command(); c.expected_version = Some(2);
        assert_eq!(l.decide("agent", c.clone()), Decision::Reject("stale_version"));
        c.expected_version = None; c.order = Order::Drive { throttle: 0, steering: 0, ttl_ticks: 0 };
        assert_eq!(l.decide("agent", c), Decision::Reject("invalid_drive")); assert_eq!(l.version, 0);
    }
}
