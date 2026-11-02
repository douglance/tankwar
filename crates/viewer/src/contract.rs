//! Adapter from the authoritative contract (docs/CONTRACT.md, protocol v1)
//! to the viewer's spectator stream.
//!
//! Contract v1 carries command lifecycle, match state, fuel, engine,
//! ammunition and the loaded round. It does not yet carry position, heading,
//! turret, crew, damage, shots or impacts (see docs/CONTRACT-REQUESTS.md), so
//! tanks are held at their side's approach and those panels show defaults.
//! Contract matches are never labelled as demos.

use crate::feed::{self, Event, EventKind, Match, Phase, Round, Seat, Side, Status, Vehicle};
use bevy::math::{Vec2, Vec3};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use tankwar_protocol::{Command, Event as PEvent, EventKind as PKind, MatchState, Order, Snapshot};

/// A recorded contract stream: snapshots and events as published.
#[derive(Deserialize)]
pub struct Recording {
    pub snapshots: Vec<Snapshot>,
    pub events: Vec<PEvent>,
}

/// Sim heading is counterclockwise from east; the viewer heading h faces
/// (sin h, cos h) on the (x, z) ground plane.
pub fn sim_heading_to_feed(a: f32) -> f32 {
    a + std::f32::consts::FRAC_PI_2
}

/// Sim coordinates are x east, y north; the village is x east, z south.
pub fn sim_to_feed(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, -y)
}

fn round(s: &str) -> Option<Round> {
    match s.to_ascii_lowercase().as_str() {
        "ap" | "apcbc" | "armour_piercing" | "armor_piercing" => Some(Round::Ap),
        "he" | "high_explosive" => Some(Round::He),
        "smoke" => Some(Round::Smoke),
        _ => None,
    }
}

fn describe(o: &Order) -> String {
    match o {
        Order::Engine { running } => format!("engine {}", if *running { "start" } else { "stop" }),
        Order::Drive { throttle, steering, ttl_ticks } => format!("drive throttle {throttle} steering {steering} for {ttl_ticks} ticks"),
        Order::Load { ammunition } => format!("load {ammunition}"),
        Order::Fire => "fire".into(),
    }
}

fn side_of(seat_id: &str, order: usize) -> Side {
    let l = seat_id.to_ascii_lowercase();
    if l.starts_with("east") {
        Side::East
    } else if l.starts_with("west") {
        Side::West
    } else if order % 2 == 0 {
        Side::West
    } else {
        Side::East
    }
}

pub fn from_json(text: &str) -> Result<Match, serde_json::Error> {
    Ok(from_recording(serde_json::from_str(text)?))
}

pub fn from_recording(r: Recording) -> Match {
    let match_id = r.snapshots.first().map(|s| s.match_id.clone()).unwrap_or_else(|| "unknown".into());
    // Seats in first-seen order from snapshots, then commands.
    let mut seat_ids: Vec<String> = Vec::new();
    let add = |id: &str, v: &mut Vec<String>| {
        if !v.iter().any(|s| s == id) {
            v.push(id.to_string());
        }
    };
    for s in &r.snapshots {
        for t in &s.tanks {
            add(&t.seat_id, &mut seat_ids);
        }
    }
    for e in &r.events {
        if let PKind::Accepted { command } = &e.event {
            add(&command.seat_id, &mut seat_ids);
        }
    }
    let seats: Vec<Seat> = seat_ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let side = side_of(id, i);
            Seat { side, agent: feed::sanitize_command_text(id), vehicle: if side == Side::West { Vehicle::PzIIIJ } else { Vehicle::TigerI } }
        })
        .collect();
    let seat_idx = |id: &str| seat_ids.iter().position(|s| s == id);

    let mut events: Vec<Event> = Vec::new();
    let mut base: Vec<feed::TankSnap> = seats
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let k = (i / 2) as f32 * 12.0;
            let (p, h) = match s.side {
                Side::West => (crate::lesmont::spawn_west() - Vec2::new(0.0, k), std::f32::consts::FRAC_PI_2),
                Side::East => (crate::lesmont::spawn_east() + Vec2::new(0.0, k), -std::f32::consts::FRAC_PI_2),
            };
            let mut snap = feed::spawn_snap(s.vehicle, p, h);
            snap.engine_on = false;
            snap.ammo = [0; 3];
            snap
        })
        .collect();

    let mut structures: Vec<(u32, Vec<(String, u32, bool)>)> = Vec::new();
    for s in &r.snapshots {
        if !s.structures.is_empty() {
            let list = s.structures.iter().map(|st| (st.id.clone(), st.integrity, st.destroyed)).collect();
            structures.push((s.tick as u32, list));
        }
        for t in &s.tanks {
            let Some(i) = seat_idx(&t.seat_id) else { continue };
            let snap = &mut base[i];
            snap.fuel_l = t.fuel_millilitres as f32 / 1000.0;
            snap.engine_on = t.engine_running;
            snap.ammo = [0; 3];
            for (name, n) in &t.ammunition {
                if let Some(r) = round(name) {
                    snap.ammo[r.index()] = snap.ammo[r.index()].saturating_add((*n).min(u16::MAX as u32) as u16);
                }
            }
            snap.loaded = t.loaded_round.as_deref().and_then(round);
            if let (Some(x), Some(y)) = (t.x_m, t.y_m) {
                snap.pos = sim_to_feed(x as f32, y as f32);
            }
            if let Some(a) = t.heading_rad {
                snap.heading = sim_heading_to_feed(a as f32);
            }
            if let Some(k) = t.knocked_out {
                snap.knocked_out = k;
            }
            events.push(Event { seq: s.sequence, tick: s.tick as u32, kind: EventKind::Snapshot { tank: i, snap: *snap } });
        }
    }

    let mut cmd_ids: HashMap<String, (u32, usize)> = HashMap::new();
    let mut next_cmd = 1u32;
    let mut status = Status::Live;
    let mut end_tick = 0u32;
    let mut cmd_ref = |id: &str, seat: Option<usize>, cmd_ids: &mut HashMap<String, (u32, usize)>| -> (u32, usize) {
        if let Some(v) = cmd_ids.get(id) {
            return *v;
        }
        let v = (next_cmd, seat.unwrap_or(0));
        next_cmd += 1;
        cmd_ids.insert(id.to_string(), v);
        v
    };
    for e in &r.events {
        let tick = e.tick as u32;
        end_tick = end_tick.max(tick);
        let mut push = |kind| events.push(Event { seq: e.sequence, tick, kind });
        match &e.event {
            PKind::Accepted { command } => {
                let Command { id, seat_id, order, .. } = command;
                let (cmd, tank) = cmd_ref(id, seat_idx(seat_id), &mut cmd_ids);
                push(EventKind::CommandSubmitted { cmd, tank, text: feed::sanitize_command_text(&describe(order)) });
                push(EventKind::CommandDecision { cmd, tank, accepted: true, reason: String::new() });
            }
            PKind::Rejected { command_id, reason } => {
                let (cmd, tank) = cmd_ref(command_id, None, &mut cmd_ids);
                push(EventKind::CommandDecision { cmd, tank, accepted: false, reason: feed::sanitize_command_text(reason) });
            }
            PKind::Started { command_id } => {
                let (cmd, tank) = cmd_ref(command_id, None, &mut cmd_ids);
                push(EventKind::CommandPhase { cmd, tank, phase: Phase::Started });
            }
            PKind::Completed { command_id } => {
                let (cmd, tank) = cmd_ref(command_id, None, &mut cmd_ids);
                push(EventKind::CommandPhase { cmd, tank, phase: Phase::Completed });
            }
            PKind::Interrupted { command_id, .. } => {
                let (cmd, tank) = cmd_ref(command_id, None, &mut cmd_ids);
                push(EventKind::CommandPhase { cmd, tank, phase: Phase::Interrupted });
            }
            PKind::Expired { command_id } => {
                let (cmd, tank) = cmd_ref(command_id, None, &mut cmd_ids);
                push(EventKind::CommandPhase { cmd, tank, phase: Phase::Expired });
            }
            PKind::StateChanged { state } => match state {
                MatchState::Running => push(EventKind::MatchStarted),
                MatchState::Finished => {
                    status = Status::Finished;
                    if !events.iter().any(|e| matches!(e.kind, EventKind::Result { .. })) {
                        events.push(Event { seq: e.sequence, tick, kind: EventKind::Result { text: "Match finished".into() } });
                    }
                }
                MatchState::Aborted | MatchState::Cancelled => {
                    status = Status::Aborted;
                    push(EventKind::Aborted { reason: format!("Match {}", if *state == MatchState::Aborted { "aborted" } else { "cancelled" }) });
                }
                _ => {}
            },
            PKind::MatchDrawn => push(EventKind::Result { text: "Draw".into() }),
        }
    }
    for s in &r.snapshots {
        end_tick = end_tick.max(s.tick as u32);
    }
    events.sort_by_key(|e| (e.tick, e.seq));
    structures.sort_by_key(|s| s.0);
    let mut m = Match::from_parts(feed::sanitize_command_text(&match_id), "Contract v1", "Village of Lesmont (fictional)", status, seats, events, end_tick, false);
    m.structures = structures;
    m
}

/// Build a spectator match from server snapshots, command events, and world
/// events. Snapshots must be close enough together for the battlefield to
/// move between them; one snapshot only shows where the tanks were then.
pub fn from_server(mut snapshots: Vec<Snapshot>, events: Vec<PEvent>, world: &[Value]) -> Option<Match> {
    if snapshots.is_empty() {
        return None;
    }
    snapshots.sort_by_key(|s| (s.tick, s.sequence));
    snapshots.dedup_by_key(|s| s.tick);
    let end_tick = snapshots.last().map(|s| s.tick).unwrap_or(0);
    let state = snapshots.last().map(|s| s.state.clone()).unwrap_or(MatchState::Running);
    let mut m = from_recording(Recording { snapshots: snapshots.clone(), events });
    m.mode = "1v1";
    m.status = status_of(state);
    m.end_tick = m.end_tick.max(end_tick as u32);
    apply_world(&mut m, world, &snapshots);
    m.events.sort_by_key(|e| (e.tick, e.seq));
    m.reindex();
    Some(m)
}

fn status_of(state: MatchState) -> Status {
    match state {
        MatchState::Finished => Status::Finished,
        MatchState::Aborted | MatchState::Cancelled => Status::Aborted,
        _ => Status::Live,
    }
}

fn seat_at(snaps: &[Snapshot], seat: &str, tick: u32) -> Option<(Vec3, f64)> {
    let mut found = None;
    for s in snaps {
        if s.tick as u32 > tick {
            break;
        }
        let Some(t) = s.tanks.iter().find(|t| t.seat_id == seat) else { continue };
        let (Some(x), Some(y)) = (t.x_m, t.y_m) else { continue };
        found = Some((feed_point(x, y), t.heading_rad.unwrap_or(0.0)));
    }
    found
}

fn feed_point(x: f64, y: f64) -> Vec3 {
    let p = sim_to_feed(x as f32, y as f32);
    Vec3::new(p.x, 1.8, p.y)
}

fn along_heading(from: Vec3, heading: f64, metres: f64) -> Vec3 {
    let step = sim_to_feed((heading.cos() * metres) as f32, (heading.sin() * metres) as f32);
    from + Vec3::new(step.x, 0.0, step.y)
}

fn casualty(rows: &[Value], tick: u64, shooter: &str) -> Option<String> {
    for row in rows {
        if row.get("tick").and_then(|t| t.as_u64()) != Some(tick) {
            continue;
        }
        let Some(ev) = row.get("event") else { continue };
        if let Some(seat) = ev.get("Destroyed").and_then(|d| d.get("seat")).and_then(|s| s.as_str()) {
            if seat != shooter {
                return Some(seat.to_string());
            }
        }
        if let Some(seat) = ev.get("Impact").and_then(|d| d.get("target")).and_then(|s| s.as_str()) {
            if seat != shooter {
                return Some(seat.to_string());
            }
        }
    }
    None
}

fn apply_world(m: &mut Match, rows: &[Value], snaps: &[Snapshot]) {
    for (i, row) in rows.iter().enumerate() {
        let tick_u = row.get("tick").and_then(|t| t.as_u64()).unwrap_or(0);
        let tick = tick_u as u32;
        let Some(ev) = row.get("event") else { continue };
        let seq = 10_000_000 + i as u64;
        if let Some(shot) = ev.get("Shot") {
            let Some(seat) = shot.get("seat").and_then(|s| s.as_str()) else { continue };
            let Some(tank) = m.seats.iter().position(|s| s.agent == seat) else { continue };
            let Some((from, heading)) = seat_at(snaps, seat, tick) else { continue };
            let trace = shot.get("trace").filter(|t| !t.is_null());
            let to = if let Some(target) = trace.and_then(|t| t.get("target")).and_then(|s| s.as_str()).and_then(|id| seat_at(snaps, id, tick)) {
                target.0
            } else if let (Some(x), Some(y)) = (trace.and_then(|t| t.get("to_x_m")).and_then(|n| n.as_f64()), trace.and_then(|t| t.get("to_y_m")).and_then(|n| n.as_f64())) {
                feed_point(x, y)
            } else if let Some(id) = casualty(rows, tick_u, seat).and_then(|id| seat_at(snaps, &id, tick)) {
                id.0
            } else {
                along_heading(from, heading, 40.0)
            };
            let round = shot.get("round").and_then(|r| r.as_str()).and_then(round).unwrap_or(Round::Ap);
            m.events.push(Event { seq, tick, kind: EventKind::Shot { tank, round, from, to } });
            m.events.push(Event { seq: seq + 1, tick, kind: EventKind::Impact { target: None, at: to, effect: "hit".into() } });
        } else if ev.get("Finished").is_some() {
            let winner = ev.get("Finished").and_then(|f| f.get("winner")).and_then(|w| w.as_u64());
            let text = match winner {
                Some(0) => "West wins",
                Some(1) => "East wins",
                _ => "Draw",
            };
            if !m.events.iter().any(|e| matches!(e.kind, EventKind::Result { .. })) {
                m.events.push(Event { seq, tick, kind: EventKind::Result { text: text.into() } });
            }
            m.status = Status::Finished;
        } else if let Some(ended) = ev.get("Aborted").or_else(|| ev.get("Cancelled")) {
            let reason = ended.get("reason").and_then(|r| r.as_str()).unwrap_or("ended");
            m.events.push(Event { seq, tick, kind: EventKind::Aborted { reason: feed::sanitize_command_text(&format!("Match {reason}")) } });
            m.status = Status::Aborted;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../fixtures/contract-v1-sample.json");

    #[test]
    fn parses_contract_sample() {
        let m = from_json(SAMPLE).expect("sample parses");
        assert!(!m.demo);
        assert_eq!(m.seats.len(), 2);
        assert_eq!(m.status, Status::Finished);
        assert_eq!(m.result_text(), Some("Draw"));
        let s = m.tank_at(0, 120.0).unwrap();
        assert!(s.engine_on);
        assert_eq!(s.loaded, Some(Round::Ap));
        assert_eq!(s.ammo[Round::Ap.index()], 39);
        assert!((s.fuel_l - 299.2).abs() < 0.01);
        let phases: Vec<Phase> = m.events.iter().filter_map(|e| match e.kind { EventKind::CommandPhase { phase, .. } => Some(phase), _ => None }).collect();
        assert!(phases.contains(&Phase::Interrupted) && phases.contains(&Phase::Expired));
        // Rejection reason is escaped: no raw angle brackets or bidi override.
        let reason = m.events.iter().find_map(|e| match &e.kind { EventKind::CommandDecision { accepted: false, reason, .. } => Some(reason.clone()), _ => None }).unwrap();
        assert!(!reason.contains('\u{202e}'));
        assert!(reason.is_ascii());
    }

    #[test]
    fn axes() {
        assert_eq!(sim_to_feed(10.0, 5.0), Vec2::new(10.0, -5.0));
    }

    /// The hostile rejection reason in the sample must reach the screen as
    /// literal, capped text: markup visible as characters, bidi and control
    /// characters made visible, and the tail cut off.
    #[test]
    fn hostile_reason_is_escaped_and_capped() {
        use crate::feed::EventKind;
        let m = from_json(include_str!("../fixtures/contract-v1-sample.json")).unwrap();
        let reason = m
            .events
            .iter()
            .find_map(|e| match &e.kind {
                EventKind::CommandDecision { accepted: false, reason, .. } => Some(reason.clone()),
                _ => None,
            })
            .expect("sample has a rejection");
        assert!(reason.starts_with("<script>alert(1)</script><img src=x"));
        assert!(reason.contains("<U+202E>") && reason.contains("<U+0007>"));
        assert!(!reason.chars().any(|c| c == '\u{202e}' || c == '\u{7}'));
        assert!(!reason.contains("END-OF-PAYLOAD"));
        assert!(reason.ends_with(" characters shown]") && reason.contains("... [cut: first 140 of "), "{reason}");
        assert_eq!(crate::feed::sanitize_command_text(&reason), reason);
    }

    #[test]
    fn sim_heading_maps_to_viewer_forward() {
        // Expected forward vectors derived from the contract axes, not the code.
        for (a, want) in [(0.0f32, Vec2::new(1.0, 0.0)), (std::f32::consts::FRAC_PI_2, Vec2::new(0.0, -1.0)), (std::f32::consts::PI, Vec2::new(-1.0, 0.0))] {
            let h = sim_heading_to_feed(a);
            let fwd = Vec2::new(h.sin(), h.cos());
            assert!((fwd - want).length() < 1e-5, "a={a} fwd={fwd:?}");
        }
    }

    fn tank(seat: &str, x: f64, y: f64, knocked_out: bool) -> tankwar_protocol::TankSnapshot {
        tankwar_protocol::TankSnapshot {
            seat_id: seat.into(),
            fuel_millilitres: 90_000,
            engine_running: true,
            ammunition: vec![("ap".into(), 20)],
            loaded_round: Some("ap".into()),
            x_m: Some(x),
            y_m: Some(y),
            heading_rad: Some(0.0),
            team: Some(if seat.starts_with("west") { 0 } else { 1 }),
            reload_ticks_remaining: Some(0),
            knocked_out: Some(knocked_out),
        }
    }

    fn frame(tick: u64, west: (f64, f64), east: (f64, f64), knocked: bool, state: MatchState) -> Snapshot {
        Snapshot {
            protocol_version: 1,
            match_id: "village-7".into(),
            state,
            tick,
            sequence: tick,
            scheduled_at_ms: None,
            processed_at_ms: None,
            tanks: vec![tank("west-0", west.0, west.1, false), tank("east-0", east.0, east.1, knocked)],
            structures: vec![],
        }
    }

    #[test]
    fn server_frames_move_and_name_the_winner() {
        let world = serde_json::json!([
            {"tick": 40, "event": {"Shot": {"seat": "west-0", "round": "ap", "trace": null}}},
            {"tick": 40, "event": {"Destroyed": {"seat": "east-0"}}},
            {"tick": 40, "event": {"Impact": {"target": "east-0"}}},
            {"tick": 40, "event": {"Finished": {"winner": 0}}}
        ]);
        let rows = world.as_array().unwrap();
        let thin = from_server(vec![frame(40, (-10.0, 5.0), (30.0, 8.0), true, MatchState::Finished)], vec![], rows).unwrap();
        assert!(!thin.has_dense_track());
        assert_eq!(thin.result_text(), Some("West wins"));
        let full = from_server(
            vec![
                frame(0, (-200.0, 0.0), (200.0, 40.0), false, MatchState::Running),
                frame(20, (-120.0, 20.0), (140.0, 30.0), false, MatchState::Running),
                frame(40, (-40.0, 40.0), (80.0, 20.0), true, MatchState::Finished),
            ],
            vec![],
            rows,
        )
        .unwrap();
        assert!(full.has_dense_track());
        assert!(!full.demo);
        assert_eq!(full.mode, "1v1");
        let start = full.tank_at(0, 0.0).unwrap().pos;
        let later = full.tank_at(0, 40.0).unwrap().pos;
        assert!((start.x - (-200.0)).abs() < 0.1, "{start:?}");
        assert!((later.x - (-40.0)).abs() < 0.1, "{later:?}");
        assert!(full.events.iter().any(|e| matches!(e.kind, EventKind::Shot { tank: 0, .. })));
        let mid = full.tank_at(0, 10.0).unwrap().pos;
        assert!(mid.x > start.x && mid.x < later.x, "tick 10 should sit between the frames, got {mid:?}");
    }
}