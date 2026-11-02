//! Adapter from the authoritative contract (docs/CONTRACT.md, protocol v1)
//! to the viewer's spectator stream.
//!
//! Contract v1 carries command lifecycle, match state, fuel, engine,
//! ammunition and the loaded round. It does not yet carry position, heading,
//! turret, crew, damage, shots or impacts (see docs/CONTRACT-REQUESTS.md), so
//! tanks are held at their side's approach and those panels show defaults.
//! Contract matches are never labelled as demos.

use crate::feed::{self, Event, EventKind, Match, Phase, Round, Seat, Side, Status, Vehicle};
use bevy::math::Vec2;
use serde::Deserialize;
use std::collections::HashMap;
use tankwar_protocol::{Command, Event as PEvent, EventKind as PKind, MatchState, Order, Snapshot};

/// A recorded contract stream: snapshots and events as published.
#[derive(Deserialize)]
pub struct Recording {
    pub snapshots: Vec<Snapshot>,
    pub events: Vec<PEvent>,
}

/// Sim coordinates are x east, y north on a flat plane; the village is
/// x east, z south. Assumption until the contract states its axes.
#[allow(dead_code)] // used once the contract carries tank positions
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
            let lane = (i / 2) as f32 * 26.0 - 13.0;
            let (x, h) = match s.side {
                Side::West => (-270.0, std::f32::consts::FRAC_PI_2),
                Side::East => (270.0, -std::f32::consts::FRAC_PI_2),
            };
            let mut snap = feed::spawn_snap(s.vehicle, Vec2::new(x, lane), h);
            snap.engine_on = false;
            snap.ammo = [0; 3];
            snap
        })
        .collect();

    for s in &r.snapshots {
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
    Match::from_parts(feed::sanitize_command_text(&match_id), "Contract v1", "Village of Lesmont (fictional)", status, seats, events, end_tick, false)
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
}
