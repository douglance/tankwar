//! A single storage record joins the command log and pending world orders.
use serde::{Deserialize, Serialize};
use tankwar_protocol::{Command, MatchState};
use tankwar_sim::world::{Pending, World, WorldEvent};
use crate::ledger::{Decision, Ledger};

#[derive(Clone, Serialize, Deserialize)]
pub struct RecordedWorldEvent { pub tick: u64, pub event: WorldEvent }
#[derive(Clone, Serialize, Deserialize)]
pub struct DurableMatch { pub ledger: Ledger, pub world: World, pub world_events: Vec<RecordedWorldEvent>, pub clock: Option<tankwar_sim::clock::Clock>, pub ready_agents: std::collections::BTreeSet<String>, #[serde(default)] pub rating_published: bool, #[serde(default)] pub queue_released: bool }
impl DurableMatch {
    /// Prototype spawn values are approximations and do not describe a sourced vehicle.
    pub fn from_assignment(a: &crate::matchmaking::Assignment) -> Result<Self, &'static str> {
        use std::collections::{BTreeMap, BTreeSet};
        use tankwar_sim::{Tank, geometry::Vec3, world::Vehicle};
        use crate::ledger::Seat;
        use tankwar_sim::competition::Mode;
        let size = match a.mode { Mode::OneOnOne => 1, Mode::TwoOnTwo => 2 };
        if a.match_id.is_empty() || a.first.len() != size || a.second.len() != size { return Err("invalid_assignment"); }
        let mut unique = BTreeSet::new();
        let mut vehicles = BTreeMap::new();
        let mut seats = Vec::new();
        for (team, agents) in [(0_u8, &a.first), (1_u8, &a.second)] {
            for (index, agent) in agents.iter().enumerate() {
                if agent.is_empty() || !unique.insert(agent.clone()) { return Err("invalid_assignment"); }
                let seat_id = format!("{}-{index}", if team == 0 { "west" } else { "east" });
                let tank = Tank { fuel_ml: 100_000, idle_fuel_ml_per_tick: 1, engine_running: false,
                    ammunition: BTreeMap::from([("ap".into(), 20), ("he".into(), 10)]), chamber: None,
                    reload_ticks: 100, loading: None, throttle: 0, steering: 0, drive_expires_at: None };
                let vehicle = Vehicle { tank, team, position: Vec3 { x: if team == 0 { -80. } else { 80. }, y: index as f64 * 10., z: 0. },
                    heading_radians: if team == 0 { 0. } else { std::f64::consts::PI },
                    speed_metres_per_second: 8., turn_radians_per_second: 0.4, half_extent_metres: 2., hit_points: 1 };
                vehicles.insert(seat_id.clone(), vehicle);
                seats.push(Seat { agent_id: agent.clone(), seat_id });
            }
        }
        let world = World::new(vehicles, vec![]);
        let ledger = Ledger::new(a.match_id.clone(), seats);
        Ok(Self { ledger, world, world_events: Vec::new(), clock: None, ready_agents: BTreeSet::new(), rating_published: false, queue_released: false })
    }
    pub fn mark_ready(&mut self, agent: &str) -> Result<bool, &'static str> {
        if !self.ledger.seats.iter().any(|seat| seat.agent_id == agent) { return Err("unauthorized_seat"); }
        if self.ledger.state == MatchState::Running { return Ok(true); }
        if !matches!(self.ledger.state, MatchState::Assigned | MatchState::Ready) { return Err("invalid_match_state"); }
        self.ready_agents.insert(agent.into());
        if self.ledger.seats.iter().all(|seat| self.ready_agents.contains(&seat.agent_id)) {
            self.ledger.state = MatchState::Ready;
            return Ok(true);
        }
        Ok(false)
    }
    pub fn cancel_unready(&mut self) -> Result<(), &'static str> {
        if !matches!(self.ledger.state, MatchState::Assigned | MatchState::Ready) || self.world.state != MatchState::Ready { return Err("invalid_match_state"); }
        self.ledger.state = MatchState::Cancelled;
        self.world.state = MatchState::Cancelled;
        self.world_events.push(RecordedWorldEvent { tick: self.world.tick, event: WorldEvent::Cancelled { reason: "ready_timeout".into() } });
        Ok(())
    }
    /// Start only after readiness; callers persist before acknowledging the start.
    pub fn start(&mut self, now_ms: u64) -> Result<(), &'static str> {
        if self.world.state != MatchState::Ready || self.ledger.state != MatchState::Ready { return Err("match_not_ready"); }
        self.clock = Some(tankwar_sim::clock::Clock::start(now_ms)?);
        self.world.state = MatchState::Running;
        self.ledger.state = MatchState::Running;
        Ok(())
    }
    /// Advance no more than the allowed catch-up window, or abort without inventing ticks.
    pub fn advance(&mut self, now_ms: u64) -> Result<u64, &'static str> {
        self.advance_with_frames(now_ms).map(|frames| frames.len() as u64)
    }
    /// Collect the exact spectator state after each simulated tick for durable replay.
    pub fn advance_with_frames(&mut self, now_ms: u64) -> Result<Vec<tankwar_protocol::Snapshot>, &'static str> {
        if self.world.state != MatchState::Running { return Err("match_not_running"); }
        let due = match self.clock.as_mut().ok_or("clock_not_started")?.due(now_ms) {
            Ok(due) => due,
            Err(reason) => {
                self.world.state = MatchState::Aborted;
                self.ledger.state = MatchState::Aborted;
                self.world_events.push(RecordedWorldEvent { tick: self.world.tick, event: WorldEvent::Aborted { reason: reason.into() } });
                return Err(reason);
            }
        };
        let mut frames = Vec::new();
        for _ in 0..due {
            if self.world.state != MatchState::Running { break; }
            self.tick();
            frames.push(self.world.spectator_snapshot(&self.ledger.match_id, self.ledger.version));
        }
        Ok(frames)
    }
    /// Caller must persist the resulting record before returning an acceptance.
    pub fn accept(&mut self, agent: &str, command: Command) -> Decision {
        let mut ledger = self.ledger.clone();
        let decision = ledger.decide(agent, command.clone());
        if matches!(decision, Decision::Accept(_)) {
            if self.world.state != MatchState::Running { return Decision::Reject("match_not_running"); }
            let pending = Pending { id: command.id, seat: command.seat_id, order: command.order };
            if let Err(reason) = self.world.enqueue(pending) { return Decision::Reject(reason); }
            self.ledger = ledger;
        }
        decision
    }
    /// Agent projection is confined to the authenticated agent's assigned vehicle.
    pub fn observe_self(&self, agent: &str) -> Result<tankwar_protocol::Snapshot, &'static str> {
        let seat = self.ledger.seats.iter().find(|seat| seat.agent_id == agent).ok_or("unauthorized_seat")?;
        let mut snapshot = self.world.spectator_snapshot(&self.ledger.match_id, self.ledger.version);
        snapshot.tanks.retain(|tank| tank.seat_id == seat.seat_id);
        let tank = snapshot.tanks.first_mut().ok_or("unknown_seat")?;
        // Global position and opponent state are not instruments available to the crew.
        tank.x_m = None; tank.y_m = None; tank.heading_rad = None; tank.team = None;
        Ok(snapshot)
    }
    /// Apply a finished match to a season's rating state; the owner must persist it atomically.
    pub fn rate_finished(&self, ratings: &mut tankwar_sim::competition::Ratings, _mode: tankwar_sim::competition::Mode) -> Result<Vec<tankwar_sim::competition::RatingChange>, &'static str> {
        let (mode, first, second, outcome) = self.result()?;
        if mode != _mode { return Err("invalid_mode"); }
        ratings.apply(&self.ledger.match_id, mode, &first, &second, outcome)
    }
    pub fn result(&self) -> Result<(tankwar_sim::competition::Mode, Vec<String>, Vec<String>, tankwar_sim::competition::Outcome), &'static str> {
        use tankwar_sim::competition::{Mode, Outcome};
        let winner = self.world_events.iter().find_map(|e| match &e.event {
            WorldEvent::Finished { winner } => Some(*winner), _ => None,
        }).ok_or("match_not_finished")?;
        let mut first = Vec::new();
        let mut second = Vec::new();
        for seat in &self.ledger.seats {
            let vehicle = self.world.vehicles.get(&seat.seat_id).ok_or("unknown_seat")?;
            match vehicle.team { 0 => first.push(seat.agent_id.clone()), 1 => second.push(seat.agent_id.clone()), _ => return Err("invalid_team") }
        }
        let mode = match (first.len(), second.len()) { (1, 1) => Mode::OneOnOne, (2, 2) => Mode::TwoOnTwo, _ => return Err("invalid_teams") };
        let outcome = match winner { None => Outcome::Draw, Some(0) => Outcome::FirstWins, Some(1) => Outcome::SecondWins, _ => return Err("invalid_winner") };
        Ok((mode, first, second, outcome))
    }
    /// Caller persists the result before publishing it; scheduling is external.
    pub fn tick(&mut self) {
        self.world_events.extend(self.world.step().into_iter().map(|event| RecordedWorldEvent { tick: self.world.tick, event }));
        self.ledger.tick = self.world.tick;
        if self.world.state == MatchState::Finished { self.ledger.state = MatchState::Finished; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::Seat;
    use tankwar_protocol::Order;
    use tankwar_sim::{Tank, geometry::Vec3, world::Vehicle};
    use std::collections::BTreeMap;
    fn fixture() -> DurableMatch {
        let tank = || Tank { fuel_ml: 100, idle_fuel_ml_per_tick: 0, engine_running: true, ammunition: BTreeMap::new(), chamber: None, reload_ticks: 1, loading: None, throttle: 0, steering: 0, drive_expires_at: None };
        let vehicle = |team,x| Vehicle { tank:tank(), team, position:Vec3{x,y:0.,z:0.}, heading_radians:0., speed_metres_per_second:10., turn_radians_per_second:1., half_extent_metres:1., hit_points:1 };
        let mut world = World::new(BTreeMap::from([("seat".into(),vehicle(0,0.)),("other".into(),vehicle(1,100.))]),vec![]); world.state=MatchState::Running;
        let mut ledger=Ledger::new("match".into(),vec![Seat{agent_id:"agent".into(),seat_id:"seat".into()}]); ledger.state=MatchState::Running;
        DurableMatch{ledger,world,world_events:vec![],clock:None,ready_agents:Default::default(),rating_published:false,queue_released:false}
    }
    #[test] fn observation_contains_only_the_assigned_vehicle() {
        let game = fixture();
        let snapshot = game.observe_self("agent").unwrap();
        assert_eq!(snapshot.tanks.len(), 1);
        assert_eq!(snapshot.tanks[0].seat_id, "seat");
        assert_eq!(snapshot.tanks[0].x_m, None);
        assert_eq!(snapshot.tanks[0].heading_rad, None);
        assert_eq!(game.observe_self("outsider"), Err("unauthorized_seat"));
    }
    #[test] fn assignment_creates_unique_opposing_ready_seats() {
        use tankwar_sim::competition::Mode;
        let assignment = crate::matchmaking::Assignment { match_id: "m1".into(), mode: Mode::TwoOnTwo,
            first: vec!["a".into(), "b".into()], second: vec!["c".into(), "d".into()] };
        let game = DurableMatch::from_assignment(&assignment).unwrap();
        assert_eq!(game.world.state, MatchState::Ready);
        assert_eq!(game.ledger.state, MatchState::Assigned);
        assert_eq!(game.world.vehicles.len(), 4);
        assert_eq!(game.ledger.seats.iter().filter(|s| s.agent_id == "a").count(), 1);
        assert_eq!(game.world.vehicles["west-0"].team, 0);
        assert_eq!(game.world.vehicles["east-0"].team, 1);
        let mut duplicate = assignment; duplicate.second[0] = "a".into();
        assert!(DurableMatch::from_assignment(&duplicate).is_err());
    }
    #[test] fn both_agents_must_be_ready_before_start() {
        use tankwar_sim::competition::Mode;
        let assignment = crate::matchmaking::Assignment { match_id:"m".into(), mode:Mode::OneOnOne, first:vec!["a".into()], second:vec!["b".into()] };
        let mut game = DurableMatch::from_assignment(&assignment).unwrap();
        assert_eq!(game.mark_ready("outsider"), Err("unauthorized_seat"));
        assert_eq!(game.mark_ready("a"), Ok(false));
        assert_eq!(game.start(0), Err("match_not_ready"));
        assert_eq!(game.mark_ready("b"), Ok(true));
        assert_eq!(game.start(0), Ok(()));
        assert_eq!(game.mark_ready("a"), Ok(true));
    }
    #[test] fn unready_match_cancels_without_starting_or_rating() {
        let assignment = crate::matchmaking::Assignment { match_id:"m".into(), mode:tankwar_sim::competition::Mode::OneOnOne, first:vec!["a".into()], second:vec!["b".into()] };
        let mut game = DurableMatch::from_assignment(&assignment).unwrap();
        game.mark_ready("a").unwrap();
        game.cancel_unready().unwrap();
        assert_eq!(game.world.state, MatchState::Cancelled);
        assert_eq!(game.ledger.state, MatchState::Cancelled);
        assert_eq!(game.start(100), Err("match_not_ready"));
        assert!(game.result().is_err());
    }
    #[test] fn scheduled_match_ticks_and_aborts_on_unrecoverable_gap() {
        let mut game = fixture(); game.world.state = MatchState::Ready; game.ledger.state = MatchState::Ready;
        game.start(1000).unwrap();
        assert_eq!(game.advance(1049), Ok(0));
        assert_eq!(game.advance(1100), Ok(2));
        assert_eq!(game.world.tick, 2);
        assert_eq!(game.advance(2200), Err("clock_overrun"));
        assert_eq!(game.world.tick, 2);
        assert_eq!(game.world.state, MatchState::Aborted);
        assert!(matches!(game.world_events.last().unwrap().event, WorldEvent::Aborted { .. }));
    }
    #[test] fn catchup_records_each_authoritative_frame() {
        let mut game = fixture(); game.world.state = MatchState::Ready; game.ledger.state = MatchState::Ready;
        game.start(1000).unwrap();
        let command=Command{protocol_version:1,id:"drive".into(),match_id:"match".into(),seat_id:"seat".into(),expected_version:None,order:Order::Drive{throttle:1000,steering:0,ttl_ticks:10}};
        assert!(matches!(game.accept("agent",command),Decision::Accept(_)));
        let frames=game.advance_with_frames(1100).unwrap();
        assert_eq!(frames.len(),2);
        assert_eq!((frames[0].tick,frames[1].tick),(1,2));
        assert_eq!(frames[0].tanks[0].x_m,Some(100.0));
        assert_eq!(frames[0].tanks[1].x_m,Some(0.5));
        assert_eq!(frames[1].tanks[1].x_m,Some(1.0));
        assert_eq!(frames[1],game.world.spectator_snapshot("match",1));
    }
    #[test] fn accepted_command_survives_serialization_and_applies_once() {
        let mut game=fixture(); let command=Command{protocol_version:1,id:"order".into(),match_id:"match".into(),seat_id:"seat".into(),expected_version:None,order:Order::Drive{throttle:1000,steering:0,ttl_ticks:1}};
        assert!(matches!(game.accept("agent",command.clone()),Decision::Accept(_)));
        let saved=serde_json::to_vec(&game).unwrap(); let mut restored:DurableMatch=serde_json::from_slice(&saved).unwrap();
        assert!(matches!(restored.accept("agent",command),Decision::Duplicate(_)));
        restored.tick(); assert_eq!(restored.world.vehicles["seat"].position.x,0.5);
        assert_eq!(restored.world_events.len(),1);
    }
    #[test] fn timeout_applies_exactly_one_draw() {
        use tankwar_sim::competition::{Mode, Outcome, Ratings};
        let mut game = fixture();
        game.ledger.seats.push(Seat { agent_id: "opponent".into(), seat_id: "other".into() });
        let mut ratings = Ratings::for_season("season-1").unwrap();
        ratings.apply("previous", Mode::OneOnOne, &["agent".into()], &["opponent".into()], Outcome::FirstWins).unwrap();
        game.world.tick = tankwar_protocol::MATCH_TICKS - 1;
        game.tick();
        let changes = game.rate_finished(&mut ratings, Mode::OneOnOne).unwrap();
        assert!(changes[0].after < changes[0].before);
        assert_eq!(game.rate_finished(&mut ratings, Mode::OneOnOne).unwrap(), changes);
        assert_eq!(ratings.games(Mode::OneOnOne, "agent"), 2);
    }
    #[test] fn missing_vehicle_is_not_recorded_as_accepted() {
        let mut game=fixture(); game.world.vehicles.remove("seat");
        let command=Command{protocol_version:1,id:"order".into(),match_id:"match".into(),seat_id:"seat".into(),expected_version:None,order:Order::Fire};
        assert_eq!(game.accept("agent",command),Decision::Reject("unknown_seat")); assert!(game.ledger.events.is_empty());
    }
}
