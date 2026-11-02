//! Prototype world integration. All numeric vehicle values are scenario approximations.
use std::collections::{BTreeMap, VecDeque};
use tankwar_protocol::{MATCH_TICKS, MatchState, Order};
use crate::{Tank, geometry::{Bounds, Vec3, first_contact}};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Vehicle {
    pub tank: Tank,
    pub team: u8,
    pub position: Vec3,
    pub heading_radians: f64,
    pub speed_metres_per_second: f64,
    pub turn_radians_per_second: f64,
    pub half_extent_metres: f64,
    pub hit_points: u32,
}
impl Vehicle {
    fn bounds(&self) -> Bounds {
        let p = self.position; let r = self.half_extent_metres;
        Bounds { min: Vec3 { x:p.x-r,y:p.y-r,z:p.z-r }, max: Vec3 { x:p.x+r,y:p.y+r,z:p.z+r } }
    }
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WorldEvent {
    OrderApplied { id: String },
    OrderFailed { id: String, reason: String },
    Shot { seat: String, round: String },
    Impact { target: Option<String> },
    Destroyed { seat: String },
    Finished { winner: Option<u8> },
    Aborted { reason: String },
    Cancelled { reason: String },
    Recovered { paused_millis: u64 },
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Pending { pub id: String, pub seat: String, pub order: Order }
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct World {
    pub state: MatchState,
    pub tick: u64,
    pub vehicles: BTreeMap<String, Vehicle>,
    pub obstacles: Vec<Bounds>,
    pending: VecDeque<Pending>,
}
impl World {
    pub fn new(vehicles: BTreeMap<String, Vehicle>, obstacles: Vec<Bounds>) -> Self { Self { state: MatchState::Ready, tick: 0, vehicles, obstacles, pending: VecDeque::new() } }
    /// Placeholder hull-fixed sight. Cone, range and precision are unsourced approximations.
    pub fn look(&self, match_id: &str, seat: &str) -> Result<tankwar_protocol::SightReport, &'static str> {
        use tankwar_protocol::{SightContact, SightReport, PROTOCOL_VERSION};
        let observer = self.vehicles.get(seat).ok_or("unknown_seat")?;
        if observer.hit_points == 0 { return Err("vehicle_destroyed"); }
        let camera = crate::geometry::Camera { position: observer.position,
            forward: Vec3 { x: observer.heading_radians.cos(), y: observer.heading_radians.sin(), z: 0. },
            horizontal_half_angle_radians: 30_f64.to_radians(), max_range: 1000. };
        let mut contacts = Vec::new();
        for (id, target) in &self.vehicles {
            if id == seat { continue; }
            let mut occluders = self.obstacles.clone();
            occluders.extend(self.vehicles.iter().filter(|(other, _)| *other != seat && *other != id).map(|(_, v)| v.bounds()));
            if !camera.sees_point(target.position, &occluders) { continue; }
            let dx = target.position.x - observer.position.x;
            let dy = target.position.y - observer.position.y;
            let bearing = (dy.atan2(dx) - observer.heading_radians + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
            contacts.push(SightContact { bearing_degrees: (bearing.to_degrees() / 5.).round() as i32 * 5,
                approximate_range_metres: ((dx.hypot(dy) / 50.).round() as u32).max(1).saturating_mul(50) });
        }
        contacts.sort_by_key(|c| (c.bearing_degrees, c.approximate_range_metres));
        Ok(SightReport { protocol_version: PROTOCOL_VERSION, match_id: match_id.into(), tick: self.tick, station: "prototype_forward_sight".into(), contacts })
    }
    /// Omniscient spectator projection. This must never be returned as an agent view.
    pub fn spectator_snapshot(&self, match_id: &str, sequence: u64) -> tankwar_protocol::Snapshot {
        use tankwar_protocol::{Snapshot, TankSnapshot, PROTOCOL_VERSION};
        Snapshot { protocol_version: PROTOCOL_VERSION, match_id: match_id.into(), state: self.state.clone(), tick: self.tick, sequence,
            tanks: self.vehicles.iter().map(|(seat, v)| TankSnapshot {
                seat_id: seat.clone(), fuel_millilitres: v.tank.fuel_ml, engine_running: v.tank.engine_running,
                ammunition: v.tank.ammunition.iter().map(|(kind, count)| (kind.clone(), *count)).collect(),
                loaded_round: v.tank.chamber.clone(), x_m: Some(v.position.x), y_m: Some(v.position.y),
                heading_rad: Some(v.heading_radians), team: Some(v.team),
                reload_ticks_remaining: v.tank.loading.as_ref().map(|(_, ticks)| *ticks), knocked_out: Some(v.hit_points == 0),
            }).collect() }
    }
    pub fn enqueue(&mut self, order: Pending) -> Result<(), &'static str> {
        if self.state != MatchState::Running { return Err("match_not_running"); }
        if !self.vehicles.contains_key(&order.seat) { return Err("unknown_seat"); }
        self.pending.push_back(order); Ok(())
    }
    /// Apply accepted orders in arrival order at the next 50 ms tick.
    pub fn step(&mut self) -> Vec<WorldEvent> {
        let mut events = Vec::new();
        if self.state != MatchState::Running { return events; }
        self.tick += 1;
        while let Some(pending) = self.pending.pop_front() {
            let fire = matches!(pending.order, Order::Fire);
            let vehicle = self.vehicles.get_mut(&pending.seat).expect("enqueue checks seat");
            let result = if vehicle.hit_points == 0 { Err("vehicle_destroyed") } else { match &pending.order {
                Order::Engine { running } => { if *running && vehicle.tank.fuel_ml == 0 { Err("out_of_fuel") } else { vehicle.tank.engine_running = *running; Ok(None) } },
                Order::Drive { throttle, steering, ttl_ticks } => vehicle.tank.drive(self.tick, *throttle, *steering, *ttl_ticks).map(|_| None),
                Order::Load { ammunition } => vehicle.tank.load(ammunition).map(|_| None),
                Order::Fire => vehicle.tank.fire().map(Some),
            }};
            match result {
                Ok(round) => {
                    events.push(WorldEvent::OrderApplied { id: pending.id });
                    if fire { if let Some(round) = round { events.push(WorldEvent::Shot { seat: pending.seat.clone(), round }); self.resolve_shot(&pending.seat, &mut events); } }
                }
                Err(reason) => events.push(WorldEvent::OrderFailed { id: pending.id, reason: reason.to_owned() }),
            }
        }
        // Seat order makes simultaneous collision resolution reproducible.
        let seats: Vec<_> = self.vehicles.keys().cloned().collect();
        for seat in seats {
            let others: Vec<_> = self.vehicles.iter().filter(|(id,_)| **id != seat).map(|(_,v)| v.bounds()).collect();
            let vehicle = self.vehicles.get_mut(&seat).expect("seat exists");
            vehicle.tank.step(self.tick);
            if vehicle.hit_points == 0 || !vehicle.tank.engine_running { continue; }
            vehicle.heading_radians += vehicle.turn_radians_per_second * (vehicle.tank.steering as f64 / 1000.) / 20.;
            let distance = vehicle.speed_metres_per_second * (vehicle.tank.throttle as f64 / 1000.) / 20.;
            let start = vehicle.position;
            let end = Vec3 { x: start.x + vehicle.heading_radians.cos() * distance, y: start.y + vehicle.heading_radians.sin() * distance, z: start.z };
            let radius = vehicle.half_extent_metres;
            let blocked = self.obstacles.iter().chain(others.iter()).any(|b| {
                let expanded = Bounds { min: Vec3 { x:b.min.x-radius, y:b.min.y-radius, z:b.min.z-radius }, max: Vec3 { x:b.max.x+radius, y:b.max.y+radius, z:b.max.z+radius } };
                expanded.intersection(start,end).is_some()
            });
            if !blocked { vehicle.position = end; }
        }
        let teams: std::collections::BTreeSet<_> = self.vehicles.values().filter(|v| v.hit_points > 0).map(|v| v.team).collect();
        if teams.len() <= 1 || self.tick >= MATCH_TICKS {
            self.state = MatchState::Finished;
            events.push(WorldEvent::Finished { winner: if teams.len() == 1 && self.tick < MATCH_TICKS { teams.first().copied() } else { None } });
        }
        events
    }
    fn resolve_shot(&mut self, shooter: &str, events: &mut Vec<WorldEvent>) {
        let gun = &self.vehicles[shooter]; let start = gun.position;
        // Instantaneous 2 km ray and one hit point damage are approximations.
        let end = Vec3 { x:start.x + gun.heading_radians.cos()*2000., y:start.y + gun.heading_radians.sin()*2000., z:start.z };
        let targets: Vec<_> = self.vehicles.iter().filter(|(seat,v)| seat.as_str()!=shooter && v.hit_points>0).map(|(seat,v)| (seat.clone(),v.bounds())).collect();
        let mut bounds = self.obstacles.clone(); bounds.extend(targets.iter().map(|(_,b)| *b));
        if let Some((index,_)) = first_contact(start,end,&bounds) {
            let target = index.checked_sub(self.obstacles.len()).map(|i| targets[i].0.clone());
            if let Some(seat) = &target {
                let v = self.vehicles.get_mut(seat).expect("target exists"); v.hit_points = v.hit_points.saturating_sub(1);
                if v.hit_points == 0 { events.push(WorldEvent::Destroyed { seat: seat.clone() }); }
            }
            events.push(WorldEvent::Impact { target });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn vehicle(team:u8,x:f64) -> Vehicle {
        Vehicle { tank: Tank { fuel_ml:100, idle_fuel_ml_per_tick:0, engine_running:true, ammunition:BTreeMap::new(), chamber:Some("ap".into()), reload_ticks:2, loading:None, throttle:0, steering:0, drive_expires_at:None }, team, position:Vec3{x,y:0.,z:0.},heading_radians:0.,speed_metres_per_second:10.,turn_radians_per_second:1.,half_extent_metres:1.,hit_points:1 }
    }
    fn world(obstacles:Vec<Bounds>) -> World {
        let mut w=World::new(BTreeMap::from([("a".into(),vehicle(0,0.)),("b".into(),vehicle(1,10.))]),obstacles); w.state=MatchState::Running; w
    }
    #[test] fn spectator_snapshot_reports_authoritative_pose_and_resources() {
        let mut w = world(vec![]);
        w.vehicles.get_mut("a").unwrap().position.y = 12.0;
        let s = w.spectator_snapshot("match-1", 9);
        assert_eq!(s.sequence, 9); assert_eq!(s.tanks[0].seat_id, "a");
        assert_eq!(s.tanks[0].x_m, Some(0.0)); assert_eq!(s.tanks[0].y_m, Some(12.0));
        assert_eq!(s.tanks[0].heading_rad, Some(0.0)); assert_eq!(s.tanks[0].knocked_out, Some(false));
    }
    #[test] fn sight_respects_cone_range_cover_and_other_vehicles() {
        let mut w = world(vec![]);
        let report = w.look("match", "a").unwrap();
        assert_eq!(report.contacts, vec![tankwar_protocol::SightContact { bearing_degrees: 0, approximate_range_metres: 50 }]);
        w.vehicles.get_mut("b").unwrap().position.x = 1001.;
        assert!(w.look("match", "a").unwrap().contacts.is_empty());
        w.vehicles.get_mut("b").unwrap().position = Vec3 { x: -10., y: 0., z: 0. };
        assert!(w.look("match", "a").unwrap().contacts.is_empty());
        w.vehicles.get_mut("b").unwrap().position.x = 100.;
        w.obstacles.push(Bounds { min: Vec3{x:40.,y:-2.,z:-2.}, max: Vec3{x:41.,y:2.,z:2.} });
        assert!(w.look("match", "a").unwrap().contacts.is_empty());
        w.obstacles.clear();
        w.vehicles.insert("c".into(), vehicle(1, 50.));
        assert_eq!(w.look("match", "a").unwrap().contacts.len(), 1);
        assert!(w.look("match", "unknown").is_err());
        w.vehicles.get_mut("a").unwrap().hit_points = 0;
        assert_eq!(w.look("match", "a"), Err("vehicle_destroyed"));
    }
    #[test] fn firing_is_server_resolved_and_ends_match() {
        let mut w=world(vec![]); w.enqueue(Pending{id:"fire".into(),seat:"a".into(),order:Order::Fire}).unwrap();
        let e=w.step(); assert!(e.contains(&WorldEvent::Destroyed{seat:"b".into()}));
        assert!(e.contains(&WorldEvent::Finished{winner:Some(0)})); assert_eq!(w.vehicles["a"].tank.chamber,None);
    }
    #[test] fn obstacle_blocks_shot() {
        let wall=Bounds{min:Vec3{x:4.,y:-2.,z:-2.},max:Vec3{x:5.,y:2.,z:2.}};
        let mut w=world(vec![wall]); w.enqueue(Pending{id:"fire".into(),seat:"a".into(),order:Order::Fire}).unwrap();
        let e=w.step(); assert!(e.contains(&WorldEvent::Impact{target:None})); assert_eq!(w.vehicles["b"].hit_points,1); assert_eq!(w.state,MatchState::Running);
    }
    #[test] fn movement_stops_at_cover_and_state_round_trips() {
        let wall=Bounds{min:Vec3{x:1.4,y:-2.,z:-2.},max:Vec3{x:2.,y:2.,z:2.}};
        let mut w=world(vec![wall]); w.enqueue(Pending{id:"drive".into(),seat:"a".into(),order:Order::Drive{throttle:1000,steering:0,ttl_ticks:3}}).unwrap();
        let encoded=serde_json::to_vec(&w).unwrap(); let mut restored:World=serde_json::from_slice(&encoded).unwrap();
        restored.step(); assert_eq!(restored.vehicles["a"].position.x,0.);
        assert_eq!(restored.tick,1);
    }
    #[test] fn queued_drive_acts_at_tick_and_expires() {
        let mut w=world(vec![]); w.enqueue(Pending{id:"drive".into(),seat:"a".into(),order:Order::Drive{throttle:1000,steering:0,ttl_ticks:1}}).unwrap();
        w.step(); assert_eq!(w.vehicles["a"].position.x,0.5); w.step(); assert_eq!(w.vehicles["a"].position.x,0.5);
    }
    #[test] fn moving_out_of_the_line_of_fire_changes_the_outcome() {
        let mut still = world(vec![]);
        still.enqueue(Pending{id:"shot".into(),seat:"a".into(),order:Order::Fire}).unwrap();
        assert!(still.step().contains(&WorldEvent::Destroyed { seat:"b".into() }));

        let mut moving = world(vec![]);
        moving.vehicles.get_mut("b").unwrap().heading_radians = std::f64::consts::FRAC_PI_2;
        moving.enqueue(Pending{id:"evade".into(),seat:"b".into(),order:Order::Drive{throttle:1000,steering:0,ttl_ticks:3}}).unwrap();
        for _ in 0..3 { moving.step(); }
        assert_eq!(moving.vehicles["b"].position.y, 1.5);
        moving.enqueue(Pending{id:"shot".into(),seat:"a".into(),order:Order::Fire}).unwrap();
        assert!(!moving.step().contains(&WorldEvent::Destroyed { seat:"b".into() }));
        assert_eq!(moving.state, MatchState::Running);
    }
}
