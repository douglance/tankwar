//! Spectator event stream.
//!
//! Until `docs/CONTRACT.md` exists this module follows the spectator event list
//! in BUILD_PLAN.md section 8: ordered events with sequence and tick IDs,
//! commands as submitted, validation decisions, execution phases, shots,
//! impacts, periodic authoritative snapshots, lifecycle, result and abort.
//! The viewer only renders this stream; it never decides outcomes.
//!
//! `demo_matches` builds clearly labelled demo feeds so the screens have
//! something to show before the authoritative server publishes real ones.

use crate::lesmont;
use bevy::math::{Vec2, Vec3};

pub const TICK_HZ: f32 = 20.0;
/// Longest command text the viewer will display.
pub const MAX_COMMAND_CHARS: usize = 140;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    West,
    East,
}

impl Side {
    pub fn label(self) -> &'static str {
        match self {
            Side::West => "West",
            Side::East => "East",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vehicle {
    PzIIIJ,
    TigerI,
}

impl Vehicle {
    pub fn label(self) -> &'static str {
        match self {
            Vehicle::PzIIIJ => "Panzer III Ausf. J",
            Vehicle::TigerI => "Tiger I",
        }
    }
    pub fn model(self) -> &'static str {
        match self {
            Vehicle::PzIIIJ => "tanks/pz3j.glb",
            Vehicle::TigerI => "tanks/tiger1.glb",
        }
    }
    /// Demo values only; not sourced vehicle data.
    pub fn demo_fuel_capacity_l(self) -> f32 {
        match self {
            Vehicle::PzIIIJ => 300.0,
            Vehicle::TigerI => 500.0,
        }
    }
    pub fn demo_loadout(self) -> [u16; 3] {
        match self {
            Vehicle::PzIIIJ => [40, 30, 0],
            Vehicle::TigerI => [45, 40, 0],
        }
    }
    pub fn demo_reload_s(self) -> f32 {
        match self {
            Vehicle::PzIIIJ => 5.0,
            Vehicle::TigerI => 8.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Round {
    Ap,
    He,
    Smoke,
}

impl Round {
    pub const ALL: [Round; 3] = [Round::Ap, Round::He, Round::Smoke];
    pub fn label(self) -> &'static str {
        match self {
            Round::Ap => "AP",
            Round::He => "HE",
            Round::Smoke => "Smoke",
        }
    }
    pub fn index(self) -> usize {
        self as usize
    }
}

pub const CREW_ROLES: [&str; 5] = ["Commander", "Gunner", "Loader", "Driver", "Radio op."];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crew {
    Ready,
    Busy,
    Wounded,
    Out,
}

impl Crew {
    pub fn label(self) -> &'static str {
        match self {
            Crew::Ready => "ready",
            Crew::Busy => "busy",
            Crew::Wounded => "wounded",
            Crew::Out => "out",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Ok,
    Damaged,
    Destroyed,
}

impl Part {
    pub fn label(self) -> &'static str {
        match self {
            Part::Ok => "ok",
            Part::Damaged => "damaged",
            Part::Destroyed => "destroyed",
        }
    }
    fn worse(self) -> Part {
        match self {
            Part::Ok => Part::Damaged,
            _ => Part::Destroyed,
        }
    }
}

pub const PARTS: [&str; 5] = ["Engine", "Left track", "Right track", "Turret ring", "Main gun"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TankSnap {
    pub pos: Vec2,
    pub heading: f32,
    /// Turret angle relative to hull, radians.
    pub turret: f32,
    pub gun_elev_deg: f32,
    pub speed: f32,
    pub engine_on: bool,
    pub fuel_l: f32,
    pub ammo: [u16; 3],
    pub loaded: Option<Round>,
    /// 0..1 while the loader is working; 0 when idle.
    pub reload: f32,
    pub crew: [Crew; 5],
    pub parts: [Part; 5],
    pub on_fire: bool,
    pub knocked_out: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Started,
    Completed,
    Interrupted,
    Expired,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Started => "started",
            Phase::Completed => "completed",
            Phase::Interrupted => "interrupted",
            Phase::Expired => "expired",
        }
    }
}

#[derive(Clone, Debug)]
pub enum EventKind {
    MatchStarted,
    Snapshot { tank: usize, snap: TankSnap },
    CommandSubmitted { cmd: u32, tank: usize, text: String },
    CommandDecision { cmd: u32, tank: usize, accepted: bool, reason: String },
    CommandPhase { cmd: u32, tank: usize, phase: Phase },
    Shot { tank: usize, round: Round, from: Vec3, to: Vec3 },
    Impact { target: Option<usize>, at: Vec3, effect: String },
    AgentDisconnected { tank: usize },
    AgentReconnected { tank: usize },
    ServerRecovery { paused_ticks: u32 },
    MissingInterval { from_tick: u32, to_tick: u32 },
    Result { text: String },
    Aborted { reason: String },
}

#[derive(Clone, Debug)]
pub struct Event {
    pub seq: u64,
    pub tick: u32,
    pub kind: EventKind,
}

impl Event {
    pub fn tank(&self) -> Option<usize> {
        match &self.kind {
            EventKind::Snapshot { tank, .. }
            | EventKind::CommandSubmitted { tank, .. }
            | EventKind::CommandDecision { tank, .. }
            | EventKind::CommandPhase { tank, .. }
            | EventKind::Shot { tank, .. }
            | EventKind::AgentDisconnected { tank }
            | EventKind::AgentReconnected { tank } => Some(*tank),
            EventKind::Impact { target, .. } => *target,
            _ => None,
        }
    }
    pub fn is_command(&self) -> bool {
        matches!(
            self.kind,
            EventKind::CommandSubmitted { .. }
                | EventKind::CommandDecision { .. }
                | EventKind::CommandPhase { .. }
        )
    }
}

#[derive(Clone, Debug)]
pub struct Seat {
    pub side: Side,
    pub agent: String,
    pub vehicle: Vehicle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Live,
    Finished,
    Aborted,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Live => "LIVE",
            Status::Finished => "RESULT",
            Status::Aborted => "ABORTED",
        }
    }
}

/// A feed-delivery stall used by demo live matches to exercise the lag label.
#[derive(Clone, Copy, Debug)]
pub struct Stall {
    pub at_s: f32,
    pub len_s: f32,
}

#[derive(Clone, Debug)]
pub struct Match {
    pub id: String,
    pub mode: &'static str,
    pub map: &'static str,
    pub status: Status,
    pub seats: Vec<Seat>,
    pub events: Vec<Event>,
    pub end_tick: u32,
    pub demo: bool,
    pub stalls: Vec<Stall>,
    /// Structure states as snapshots reported them: (tick, [(id, integrity, destroyed)]).
    pub structures: Vec<(u32, Vec<(String, u32, bool)>)>,
    snaps: Vec<Vec<(u32, TankSnap)>>,
}

impl Match {
    /// Build a match from an already ordered event list (used by adapters).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_parts(id: String, mode: &'static str, map: &'static str, status: Status, seats: Vec<Seat>, events: Vec<Event>, end_tick: u32, demo: bool) -> Self {
        Match { id, mode, map, status, seats, events, end_tick, demo, stalls: Vec::new(), structures: Vec::new(), snaps: Vec::new() }.index()
    }

    pub(crate) fn index(mut self) -> Self {
        self.reindex();
        self
    }

    /// Rebuild snapshot tracks after events are appended.
    pub(crate) fn reindex(&mut self) {
        self.snaps = vec![Vec::new(); self.seats.len()];
        for e in &self.events {
            if let EventKind::Snapshot { tank, snap } = &e.kind {
                if *tank < self.snaps.len() {
                    self.snaps[*tank].push((e.tick, *snap));
                }
            }
        }
    }

    /// True when every seat has positions from tick 0 with no gap large enough
    /// to freeze interpolation. The battlefield holds a tank still across a
    /// gap of more than 20 ticks.
    pub(crate) fn has_dense_track(&self) -> bool {
        if self.end_tick == 0 {
            return true;
        }
        if self.snaps.is_empty() {
            return false;
        }
        self.snaps.iter().all(|list| {
            let Some((first, _)) = list.first() else { return false };
            if *first != 0 {
                return false;
            }
            let Some((last, _)) = list.last() else { return false };
            self.end_tick.saturating_sub(*last) <= 20 && list.windows(2).all(|w| w[1].0.saturating_sub(w[0].0) <= 20)
        })
    }

    /// Authoritative tank state at a fractional tick, interpolated between
    /// recorded snapshots. Never extrapolates past the last snapshot.
    pub fn tank_at(&self, tank: usize, tick: f32) -> Option<TankSnap> {
        let list = self.snaps.get(tank)?;
        let i = list.partition_point(|(t, _)| (*t as f32) <= tick);
        if i == 0 {
            return list.first().map(|s| s.1);
        }
        let (t0, a) = list[i - 1];
        let Some(&(t1, b)) = list.get(i) else { return Some(a) };
        if t1 - t0 > 20 {
            // Do not invent motion across a recording gap.
            return Some(a);
        }
        let f = ((tick - t0 as f32) / (t1 - t0) as f32).clamp(0.0, 1.0);
        let mut s = a;
        s.pos = a.pos.lerp(b.pos, f);
        s.heading = lerp_angle(a.heading, b.heading, f);
        s.turret = lerp_angle(a.turret, b.turret, f);
        s.fuel_l = a.fuel_l + (b.fuel_l - a.fuel_l) * f;
        if b.reload >= a.reload {
            s.reload = a.reload + (b.reload - a.reload) * f;
        }
        Some(s)
    }

    /// Latest structure states reported at or before `tick`, if any.
    pub fn structures_at(&self, tick: f32) -> Option<&[(String, u32, bool)]> {
        let i = self.structures.partition_point(|(t, _)| (*t as f32) <= tick);
        if i == 0 { None } else { Some(&self.structures[i - 1].1) }
    }

    /// Number of events whose tick is at or before `tick`.
    pub fn events_until(&self, tick: f32) -> usize {
        self.events.partition_point(|e| (e.tick as f32) <= tick)
    }

    pub fn duration_s(&self) -> f32 {
        self.end_tick as f32 / TICK_HZ
    }

    pub fn missing_intervals(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.events.iter().filter_map(|e| match e.kind {
            EventKind::MissingInterval { from_tick, to_tick } => Some((from_tick, to_tick)),
            _ => None,
        })
    }

    pub fn result_text(&self) -> Option<&str> {
        self.events.iter().rev().find_map(|e| match &e.kind {
            EventKind::Result { text } => Some(text.as_str()),
            EventKind::Aborted { reason } => Some(reason.as_str()),
            _ => None,
        })
    }
}

pub fn lerp_angle(a: f32, b: f32, f: f32) -> f32 {
    let mut d = (b - a) % std::f32::consts::TAU;
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    } else if d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    a + d * f
}

/// Make agent-supplied command text safe to show: control, bidirectional
/// override and other invisible formatting characters become a visible
/// placeholder, and the text is capped. Cut text ends with a notice saying
/// how much of the original is shown. The viewer renders the result as plain
/// glyphs; it never interprets markup. Running it again on its own output
/// changes nothing, so the notice keeps the original length.
pub fn sanitize_command_text(raw: &str) -> String {
    if let Some(done) = already_cut(raw) {
        return done.to_string();
    }
    let mut out = String::new();
    let mut count = 0;
    for c in raw.chars() {
        if count == MAX_COMMAND_CHARS {
            out.push_str(&cut_notice(raw.chars().count()));
            break;
        }
        let bidi = matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}');
        if c.is_control() || bidi {
            out.push_str(&format!("<U+{:04X}>", c as u32));
        } else {
            out.push(c);
        }
        count += 1;
    }
    out
}

/// The notice appended to command text that was cut.
pub fn cut_notice(total: usize) -> String {
    format!("... [cut: first {MAX_COMMAND_CHARS} of {total} characters shown]")
}

/// `raw` if it is already the output of a cut: a clean head of exactly the
/// cap (placeholders count as one character) followed by a cut notice for a
/// longer original. Anything else, a forged notice included, is cut again.
fn already_cut(raw: &str) -> Option<&str> {
    let (head, tail) = raw.rsplit_once("... [cut: first ")?;
    let total: usize = tail.strip_prefix(&format!("{MAX_COMMAND_CHARS} of "))?.strip_suffix(" characters shown]")?.parse().ok()?;
    let clean = !head.chars().any(|c| c.is_control() || matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}'));
    let mut shown = head.chars().count();
    let mut rest = head;
    while let Some(i) = rest.find("<U+") {
        let p = &rest[i..];
        if p.len() >= 8 && p.as_bytes()[7] == b'>' && p[3..7].chars().all(|c| c.is_ascii_hexdigit()) {
            shown -= 7;
            rest = &p[8..];
        } else {
            rest = &p[3..];
        }
    }
    (clean && shown == MAX_COMMAND_CHARS && total > MAX_COMMAND_CHARS).then_some(raw)
}

// ---------------------------------------------------------------------------
// Demo feeds
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Order {
    Drive(Vec2),
    Halt,
    Traverse(usize),
    Load(Round),
    Fire(usize),
}

impl Order {
    fn text(&self, names: &[String]) -> String {
        match self {
            Order::Drive(p) => format!("driver: advance to grid {:.0} {:.0}", p.x, p.y),
            Order::Halt => "driver: halt".into(),
            Order::Traverse(t) => format!("gunner: traverse onto {}", names[*t]),
            Order::Load(r) => format!("loader: load {}", r.label()),
            Order::Fire(t) => format!("gunner: fire at {}", names[*t]),
        }
    }
}

struct Script {
    at_s: f32,
    tank: usize,
    order: Order,
    raw_text: Option<String>,
}

struct SimTank {
    vehicle: Vehicle,
    s: TankSnap,
    drive: Option<(u32, Vec2)>,
    traverse: Option<(u32, usize)>,
    load: Option<(u32, Round, f32)>,
    fire: Option<(u32, usize)>,
    hits_taken: u32,
}

struct Builder {
    events: Vec<Event>,
    seq: u64,
}

impl Builder {
    fn push(&mut self, tick: u32, kind: EventKind) {
        self.seq += 1;
        self.events.push(Event { seq: self.seq, tick, kind });
    }
}

pub(crate) fn spawn_snap(v: Vehicle, pos: Vec2, heading: f32) -> TankSnap {
    TankSnap {
        pos,
        heading,
        turret: 0.0,
        gun_elev_deg: 0.0,
        speed: 0.0,
        engine_on: true,
        fuel_l: v.demo_fuel_capacity_l() * 0.9,
        ammo: v.demo_loadout(),
        loaded: None,
        reload: 0.0,
        crew: [Crew::Ready; 5],
        parts: [Part::Ok; 5],
        on_fire: false,
        knocked_out: false,
    }
}

fn hash(a: u32, b: u32) -> u32 {
    let mut x = a.wrapping_mul(0x9E37_79B9) ^ b.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^ (x >> 12)
}

struct DemoSpec {
    id: &'static str,
    mode: &'static str,
    status: Status,
    seats: Vec<Seat>,
    script: Vec<Script>,
    max_s: f32,
    seed: u32,
    disconnect: Option<(usize, f32, f32)>,
    recovery_at_s: Option<f32>,
    gap: Option<(f32, f32)>,
    abort_at_s: Option<f32>,
    stalls: Vec<Stall>,
}

fn run(spec: DemoSpec) -> Match {
    let names: Vec<String> = spec.seats.iter().map(|s| s.agent.clone()).collect();
    let mut tanks: Vec<SimTank> = spec
        .seats
        .iter()
        .enumerate()
        .map(|(i, seat)| {
            let k = (i / 2) as f32 * 12.0;
            let (p, h) = match seat.side {
                Side::West => (lesmont::spawn_west() - Vec2::new(0.0, k), std::f32::consts::FRAC_PI_2),
                Side::East => (lesmont::spawn_east() + Vec2::new(0.0, k), -std::f32::consts::FRAC_PI_2),
            };
            SimTank {
                vehicle: seat.vehicle,
                s: spawn_snap(seat.vehicle, p, h),
                drive: None,
                traverse: None,
                load: None,
                fire: None,
                hits_taken: 0,
            }
        })
        .collect();

    let mut b = Builder { events: Vec::new(), seq: 0 };
    b.push(0, EventKind::MatchStarted);
    let mut script = spec.script;
    script.sort_by(|a, b| a.at_s.total_cmp(&b.at_s));
    let mut next = 0;
    let mut cmd_id = 100;
    let max_tick = (spec.max_s * TICK_HZ) as u32;
    let abort_tick = spec.abort_at_s.map(|s| (s * TICK_HZ) as u32);
    let mut shot_n = 0u32;
    let mut end_tick = max_tick;
    let mut finished = false;
    // Demo structure state: homes and the church start at full integrity.
    let mut structs: Vec<(String, u32, bool)> = lesmont::FOOTPRINTS
        .iter()
        .filter(|f| f.kind != lesmont::Kind::Wall)
        .map(|f| (f.id.to_string(), f.max_integrity(), false))
        .collect();
    let mut structures: Vec<(u32, Vec<(String, u32, bool)>)> = vec![(0, structs.clone())];

    for tick in 0..=max_tick {
        let dt = 1.0 / TICK_HZ;
        let now_s = tick as f32 / TICK_HZ;

        if let Some((tank, off, on)) = spec.disconnect {
            if tick == (off * TICK_HZ) as u32 {
                b.push(tick, EventKind::AgentDisconnected { tank });
                // Held inputs expire to neutral on disconnect.
                if let Some((id, _)) = tanks[tank].drive.take() {
                    b.push(tick, EventKind::CommandPhase { cmd: id, tank, phase: Phase::Expired });
                    tanks[tank].s.speed = 0.0;
                }
            }
            if tick == (on * TICK_HZ) as u32 {
                b.push(tick, EventKind::AgentReconnected { tank });
            }
        }
        if let Some(r) = spec.recovery_at_s {
            if tick == (r * TICK_HZ) as u32 {
                b.push(tick, EventKind::ServerRecovery { paused_ticks: 240 });
            }
        }

        while next < script.len() && script[next].at_s <= now_s {
            let sc = &script[next];
            next += 1;
            let t = sc.tank;
            if tanks[t].s.knocked_out {
                continue;
            }
            if let Some((tank, off, on)) = spec.disconnect {
                if t == tank && now_s >= off && now_s < on {
                    continue;
                }
            }
            cmd_id += 1;
            let text = sc.raw_text.clone().unwrap_or_else(|| sc.order.text(&names));
            b.push(tick, EventKind::CommandSubmitted { cmd: cmd_id, tank: t, text });
            let tk = &mut tanks[t];
            let reject = match &sc.order {
                Order::Load(r) if tk.s.ammo[r.index()] == 0 => Some(format!("no {} rounds aboard", r.label())),
                Order::Load(_) if tk.s.loaded.is_some() => Some("breech already loaded".into()),
                Order::Fire(_) if tk.s.loaded.is_none() => Some("no round in the breech".into()),
                Order::Drive(_) if tk.s.parts[1] == Part::Destroyed || tk.s.parts[2] == Part::Destroyed => {
                    Some("track broken; vehicle immobile".into())
                }
                _ => None,
            };
            let accepted = reject.is_none();
            b.push(tick + 1, EventKind::CommandDecision {
                cmd: cmd_id,
                tank: t,
                accepted,
                reason: reject.unwrap_or_else(|| "accepted".into()),
            });
            if !accepted {
                continue;
            }
            b.push(tick + 1, EventKind::CommandPhase { cmd: cmd_id, tank: t, phase: Phase::Started });
            match sc.order.clone() {
                Order::Drive(p) => {
                    if let Some((old, _)) = tk.drive.replace((cmd_id, p)) {
                        b.push(tick + 1, EventKind::CommandPhase { cmd: old, tank: t, phase: Phase::Interrupted });
                    }
                }
                Order::Halt => {
                    if let Some((old, _)) = tk.drive.take() {
                        b.push(tick + 1, EventKind::CommandPhase { cmd: old, tank: t, phase: Phase::Interrupted });
                    }
                    tk.s.speed = 0.0;
                    b.push(tick + 2, EventKind::CommandPhase { cmd: cmd_id, tank: t, phase: Phase::Completed });
                }
                Order::Traverse(target) => tk.traverse = Some((cmd_id, target)),
                Order::Load(r) => {
                    tk.load = Some((cmd_id, r, 0.0));
                    tk.s.crew[2] = Crew::Busy;
                }
                Order::Fire(target) => tk.fire = Some((cmd_id, target)),
            }
        }

        let positions: Vec<(Vec2, bool)> = tanks.iter().map(|t| (t.s.pos, t.s.knocked_out)).collect();
        let mut hits: Vec<(usize, usize, Round)> = Vec::new();
        for (i, tk) in tanks.iter_mut().enumerate() {
            if tk.s.knocked_out {
                continue;
            }
            // Driving.
            if let Some((id, target)) = tk.drive {
                let to = target - tk.s.pos;
                if to.length() < 3.0 {
                    tk.drive = None;
                    tk.s.speed = 0.0;
                    b.push(tick, EventKind::CommandPhase { cmd: id, tank: i, phase: Phase::Completed });
                } else {
                    let want = to.x.atan2(to.y);
                    let d = lerp_angle(tk.s.heading, want, 1.0) - tk.s.heading;
                    tk.s.heading += d.clamp(-0.5 * dt, 0.5 * dt);
                    let top = if tk.s.parts[0] == Part::Damaged { 3.0 } else { 7.0 };
                    tk.s.speed = (tk.s.speed + 1.5 * dt).min(top);
                    tk.s.crew[3] = Crew::Busy;
                }
            } else {
                tk.s.speed = (tk.s.speed - 3.0 * dt).max(0.0);
                if tk.s.crew[3] == Crew::Busy {
                    tk.s.crew[3] = Crew::Ready;
                }
            }
            let fwd = Vec2::new(tk.s.heading.sin(), tk.s.heading.cos());
            tk.s.pos += fwd * tk.s.speed * dt;
            let burn = if tk.s.engine_on { 0.012 + tk.s.speed * 0.006 } else { 0.0 };
            tk.s.fuel_l = (tk.s.fuel_l - burn * dt * 20.0).max(0.0);

            // Turret traverse.
            if let Some((id, target)) = tk.traverse {
                let to = positions[target].0 - tk.s.pos;
                let want = to.x.atan2(to.y) - tk.s.heading;
                let d = lerp_angle(tk.s.turret, want, 1.0) - tk.s.turret;
                let rate = if tk.s.parts[3] == Part::Damaged { 0.05 } else { 0.25 };
                tk.s.turret += d.clamp(-rate * dt, rate * dt);
                tk.s.crew[1] = Crew::Busy;
                if d.abs() < 0.01 && id != 0 {
                    b.push(tick, EventKind::CommandPhase { cmd: id, tank: i, phase: Phase::Completed });
                    // Keep tracking without a pending command.
                    tk.traverse = Some((0, target));
                    tk.s.crew[1] = Crew::Ready;
                }
            }

            // Loading.
            if let Some((id, r, p)) = tk.load {
                let p = p + dt / tk.vehicle.demo_reload_s();
                if p >= 1.0 {
                    tk.s.ammo[r.index()] -= 1;
                    tk.s.loaded = Some(r);
                    tk.s.reload = 0.0;
                    tk.load = None;
                    tk.s.crew[2] = Crew::Ready;
                    b.push(tick, EventKind::CommandPhase { cmd: id, tank: i, phase: Phase::Completed });
                } else {
                    tk.s.reload = p;
                    tk.load = Some((id, r, p));
                }
            }

            // Firing.
            if let Some((id, target)) = tk.fire {
                let aligned = tk.traverse.is_some_and(|(c, t)| c == 0 && t == target);
                if aligned && tk.s.parts[4] != Part::Destroyed {
                    if let Some(r) = tk.s.loaded.take() {
                        shot_n += 1;
                        let from = Vec3::new(tk.s.pos.x, 2.0, tk.s.pos.y);
                        let tp = positions[target].0;
                        let hit = hash(spec.seed, shot_n) % 10 < 6 && !positions[target].1;
                        let miss = Vec2::new((hash(shot_n, 7) % 30) as f32 - 15.0, (hash(shot_n, 9) % 30) as f32 - 15.0);
                        let land = if hit { tp } else { tp + miss };
                        // The first standing structure on the line stops the shot.
                        let mut block: Option<(f32, usize)> = None;
                        for (fi, f) in lesmont::FOOTPRINTS.iter().enumerate() {
                            let standing = f.kind == lesmont::Kind::Wall
                                || structs.iter().any(|(id, _, d)| id == f.id && !d);
                            if !standing {
                                continue;
                            }
                            if let Some(t) = lesmont::segment_hits_rect(tk.s.pos, land, f.viewer_rect()) {
                                if t > 0.0 && block.is_none_or(|(bt, _)| t < bt) {
                                    block = Some((t, fi));
                                }
                            }
                        }
                        let land = block.map_or(land, |(t, _)| tk.s.pos.lerp(land, t));
                        let to = Vec3::new(land.x, if block.is_some() { 2.5 } else { 1.2 }, land.y);
                        b.push(tick, EventKind::Shot { tank: i, round: r, from, to });
                        if let Some((_, fi)) = block {
                            let f = &lesmont::FOOTPRINTS[fi];
                            let effect = match structs.iter_mut().find(|(id, _, _)| id == f.id) {
                                Some(st) => {
                                    st.1 = st.1.saturating_sub(1);
                                    st.2 = st.1 == 0;
                                    let msg = if st.2 { format!("{} struck; destroyed", f.id) } else { format!("{} struck; integrity {}", f.id, st.1) };
                                    structures.push((tick + 4, structs.clone()));
                                    msg
                                }
                                None => format!("{} struck; wall holds", f.id),
                            };
                            b.push(tick + 4, EventKind::Impact { target: None, at: to, effect });
                        } else if hit {
                            hits.push((i, target, r));
                        } else {
                            b.push(tick + 4, EventKind::Impact { target: None, at: to, effect: "miss: ground strike".into() });
                        }
                        b.push(tick, EventKind::CommandPhase { cmd: id, tank: i, phase: Phase::Completed });
                        tk.fire = None;
                    }
                }
            }
        }

        for (shooter, target, r) in hits {
            let t = &mut tanks[target];
            t.hits_taken += 1;
            let roll = hash(spec.seed ^ target as u32, t.hits_taken);
            let part = (roll % 5) as usize;
            let mut effect = String::new();
            if r == Round::He && roll % 3 == 0 {
                effect.push_str("HE splash, no penetration");
            } else {
                t.s.parts[part] = t.s.parts[part].worse();
                effect = format!("{} {}", PARTS[part].to_lowercase(), t.s.parts[part].label());
                let crew = (roll / 7 % 5) as usize;
                if roll % 4 == 0 {
                    t.s.crew[crew] = if t.s.crew[crew] == Crew::Wounded { Crew::Out } else { Crew::Wounded };
                    effect.push_str(&format!("; {} {}", CREW_ROLES[crew].to_lowercase(), t.s.crew[crew].label()));
                }
                if part == 0 && t.s.parts[0] == Part::Destroyed {
                    t.s.on_fire = true;
                    t.s.engine_on = false;
                    effect.push_str("; engine fire");
                }
                if part == 1 || part == 2 {
                    if t.s.parts[part] == Part::Destroyed {
                        t.drive = None;
                        t.s.speed = 0.0;
                    }
                }
            }
            if t.hits_taken >= 3 || (t.s.on_fire && t.hits_taken >= 2) {
                t.s.knocked_out = true;
                t.s.speed = 0.0;
                t.s.on_fire = true;
                t.s.crew = t.s.crew.map(|c| if c == Crew::Out { c } else { Crew::Wounded });
                effect.push_str("; vehicle knocked out");
            }
            let at = Vec3::new(t.s.pos.x, 1.4, t.s.pos.y);
            let _ = shooter;
            b.push(tick + 4, EventKind::Impact { target: Some(target), at, effect });
        }

        {
            for (i, tk) in tanks.iter().enumerate() {
                b.push(tick, EventKind::Snapshot { tank: i, snap: tk.s });
            }
        }

        if Some(tick) == abort_tick {
            b.push(tick, EventKind::Aborted { reason: "simulation host lost; match aborted and unrated".into() });
            end_tick = tick;
            break;
        }
        for side in [Side::West, Side::East] {
            let alive = spec.seats.iter().zip(&tanks).any(|(s, t)| s.side == side && !t.s.knocked_out);
            if !alive && !finished {
                let winner = if side == Side::West { Side::East } else { Side::West };
                b.push(tick + 5, EventKind::Result { text: format!("{} wins by elimination", winner.label()) });
                end_tick = tick + 5;
                finished = true;
            }
        }
        if finished {
            break;
        }
        if tick == max_tick && spec.status == Status::Finished {
            b.push(tick, EventKind::Result { text: "time limit reached; draw".into() });
        }
    }

    // Close the record with the final state so the last snapshot agrees with
    // every shot and load that happened after the previous periodic one.
    for (i, tk) in tanks.iter().enumerate() {
        b.push(end_tick, EventKind::Snapshot { tank: i, snap: tk.s });
    }

    let mut events = b.events;
    if let Some((from_s, to_s)) = spec.gap {
        let (f, t) = ((from_s * TICK_HZ) as u32, (to_s * TICK_HZ) as u32);
        events.retain(|e| e.tick < f || e.tick >= t);
        let at = events.partition_point(|e| e.tick < f);
        events.insert(at, Event { seq: 0, tick: f, kind: EventKind::MissingInterval { from_tick: f, to_tick: t } });
        // Structure changes inside the gap are first seen after it.
        for st in structures.iter_mut() {
            if st.0 >= f && st.0 < t {
                st.0 = t;
            }
        }
    }
    events.sort_by_key(|e| (e.tick, e.seq));

    Match {
        id: spec.id.into(),
        mode: spec.mode,
        map: "Village of Lesmont (fictional)",
        status: spec.status,
        seats: spec.seats,
        events,
        end_tick,
        demo: true,
        stalls: spec.stalls,
        structures,
        snaps: Vec::new(),
    }
    .index()
}

fn seat(side: Side, agent: &str, vehicle: Vehicle) -> Seat {
    Seat { side, agent: agent.into(), vehicle }
}

fn s(at_s: f32, tank: usize, order: Order) -> Script {
    Script { at_s, tank, order, raw_text: None }
}

/// Lane routes for each seat: (order time, viewer waypoint). Tanks follow the
/// main road and farm lanes into cover behind Lesmont's homes.
pub const ROUTES: [&[(f32, (f32, f32))]; 4] = [
    // West lead: along the main road, then behind home h3.
    &[(2.0, (-150.0, -8.0)), (16.0, (-112.0, 2.0)), (28.0, (-102.0, 10.0))],
    // East lead: in from the east approach to a firing spot beside h4.
    &[(2.0, (148.0, -34.0)), (18.0, (112.0, -22.0)), (30.0, (95.0, -22.0))],
    // West second: through the village centre, then up beside home h5.
    &[(3.0, (-150.0, -14.0)), (18.0, (-60.0, -14.0)), (60.0, (-42.0, -34.0)), (100.0, (-44.0, -60.0)), (112.0, (-10.0, -62.0))],
    // East second: across the east fields into cover behind home h6.
    &[(3.0, (150.0, -30.0)), (18.0, (100.0, -34.0)), (62.0, (52.0, -40.0)), (76.0, (48.0, -24.0))],
];

/// Engagement script: both sides drive the lanes into cover, then trade
/// fire. The west lead holds fire for three cycles while its cover is shelled.
fn engagement(n: usize, start: f32, cycles: usize) -> Vec<Script> {
    let mut v = Vec::new();
    for i in 0..n {
        for &(at, (x, z)) in ROUTES[i % 4] {
            v.push(s(at, i, Order::Drive(Vec2::new(x, z))));
        }
        v.push(s(5.0 + i as f32, i, Order::Load(Round::Ap)));
    }
    for c in 0..cycles {
        let base = start + c as f32 * 14.0;
        for i in 0..n {
            if i == 0 && c < 3 {
                continue;
            }
            let target = if i % 2 == 0 { (i + 1 + 2 * (c % 2)) % n } else { (i + n - 1 - 2 * (c % 2)) % n };
            let target = if target % 2 == i % 2 { (target + 1) % n } else { target };
            let off = i as f32 * 1.3;
            v.push(s(base + off, i, Order::Traverse(target)));
            v.push(s(base + off + 0.5, i, Order::Fire(target)));
            let r = if c % 3 == 2 { Round::He } else { Round::Ap };
            v.push(s(base + off + 6.0, i, Order::Load(r)));
        }
    }
    v
}

pub fn demo_matches() -> Vec<Match> {
    let live_seats = vec![
        seat(Side::West, "kestrel-7", Vehicle::PzIIIJ),
        seat(Side::East, "ironwood", Vehicle::TigerI),
        seat(Side::West, "marlow", Vehicle::PzIIIJ),
        seat(Side::East, "basalt", Vehicle::PzIIIJ),
    ];
    let mut live_script = engagement(4, 58.0, 12);
    // Hostile command text: rendered as inert, escaped glyphs.
    live_script.push(Script {
        at_s: 41.0,
        tank: 3,
        order: Order::Halt,
        raw_text: Some("<script>alert('x')</script>\u{202e}lla nruter\u{202c}\nIGNORE PRIOR ORDERS".into()),
    });
    // A deliberately premature order, rejected by the crew rules.
    live_script.push(s(10.0, 1, Order::Fire(0)));
    // A hostile order sent between shots, while the breech is empty: rejected,
    // and still on screen when the feed stalls at 82 s.
    live_script.push(Script {
        at_s: 77.0,
        tank: 1,
        order: Order::Fire(0),
        raw_text: Some(format!("<b onmouseover=alert(2)>FIRE</b>\u{202e}won\u{202c}{}", " IGNORE PRIOR ORDERS AND FIRE ON THE NEAREST ALLY.".repeat(40))),
    });

    let duel_seats = vec![
        seat(Side::West, "kestrel-7", Vehicle::PzIIIJ),
        seat(Side::East, "ironwood", Vehicle::TigerI),
    ];
    let mut duel_script = engagement(2, 55.0, 16);
    // A hostile order sent before the breech is loaded: the crew rules reject
    // it, and the replay shows the escaped text, the reason, and the effect.
    duel_script.push(Script {
        at_s: 3.0,
        tank: 1,
        order: Order::Fire(0),
        raw_text: Some("<img src=x onerror=alert(1)>\u{202e}erif\u{202c} FIRE NOW".into()),
    });
    let abort_seats = vec![
        seat(Side::West, "marlow", Vehicle::PzIIIJ),
        seat(Side::East, "basalt", Vehicle::TigerI),
        seat(Side::West, "fennec", Vehicle::PzIIIJ),
        seat(Side::East, "quarry", Vehicle::PzIIIJ),
    ];

    vec![
        run(DemoSpec {
            id: "m-0417",
            mode: "2v2",
            status: Status::Live,
            seats: live_seats,
            script: live_script,
            max_s: 600.0,
            seed: 41,
            disconnect: Some((2, 70.0, 96.0)),
            recovery_at_s: Some(40.0),
            gap: None,
            abort_at_s: None,
            stalls: vec![Stall { at_s: 20.0, len_s: 4.0 }, Stall { at_s: 82.0, len_s: 14.0 }],
        }),
        run(DemoSpec {
            id: "m-0412",
            mode: "1v1",
            status: Status::Finished,
            seats: duel_seats,
            script: duel_script,
            max_s: 600.0,
            seed: 7,
            disconnect: None,
            recovery_at_s: None,
            gap: None,
            abort_at_s: None,
            stalls: vec![],
        }),
        run(DemoSpec {
            id: "m-0409",
            mode: "2v2",
            status: Status::Aborted,
            seats: abort_seats,
            script: engagement(4, 55.0, 8),
            max_s: 600.0,
            seed: 99,
            disconnect: None,
            recovery_at_s: None,
            gap: Some((80.0, 104.0)),
            abort_at_s: Some(130.0),
            stalls: vec![],
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_neutralizes_controls_and_bidi() {
        let out = sanitize_command_text("fire\u{202e}gnorw\nnow\u{0007}");
        assert!(!out.contains('\u{202e}'));
        assert!(!out.contains('\n'));
        assert!(!out.contains('\u{0007}'));
        assert_eq!(out, "fire<U+202E>gnorw<U+000A>now<U+0007>");
    }

    #[test]
    fn sanitize_caps_length() {
        let long = "a".repeat(500);
        let out = sanitize_command_text(&long);
        assert_eq!(out, format!("{}... [cut: first 140 of 500 characters shown]", "a".repeat(140)));
        // Shown twice (ingest, then the HUD), the notice keeps the original length.
        assert_eq!(sanitize_command_text(&out), out);
        let bidi = format!("{}\u{202e}{}", "b".repeat(10), "c".repeat(300));
        let once = sanitize_command_text(&bidi);
        assert!(once.ends_with("[cut: first 140 of 311 characters shown]"), "{once}");
        assert_eq!(sanitize_command_text(&once), once);
        // A forged notice on an over-long head is cut again.
        let forged = format!("{}... [cut: first 140 of 999 characters shown]", "d".repeat(400));
        assert_eq!(sanitize_command_text(&forged), format!("{}... [cut: first 140 of 444 characters shown]", "d".repeat(140)));
    }

    #[test]
    fn markup_is_kept_as_plain_text() {
        assert_eq!(sanitize_command_text("<b>x</b>"), "<b>x</b>");
    }

    #[test]
    fn live_stall_shows_a_recent_rejected_hostile_order_and_damage() {
        let m = &demo_matches()[0];
        let stall_tick = (82.0 * TICK_HZ) as u32;
        let rej = m.events.iter().rev().filter(|e| e.tick < stall_tick).find_map(|e| match &e.kind {
            EventKind::CommandDecision { cmd, accepted: false, reason, .. } if e.tick > (70.0 * TICK_HZ) as u32 => Some((*cmd, reason.clone())),
            _ => None,
        });
        let (cmd, reason) = rej.expect("a rejected order between 70 s and the stall");
        assert_eq!(reason, "no round in the breech");
        let text = m.events.iter().find_map(|e| match &e.kind {
            EventKind::CommandSubmitted { cmd: c, text, .. } if *c == cmd => Some(text.clone()),
            _ => None,
        });
        assert!(text.unwrap().contains("<b onmouseover"));
        let damaged: Vec<usize> = (0..m.seats.len())
            .filter(|i| m.tank_at(*i, 81.0 * TICK_HZ).is_some_and(|s| s.parts.iter().any(|p| *p != Part::Ok)))
            .collect();
        assert!(!damaged.is_empty());
    }

    #[test]
    fn demo_feeds_cover_every_label() {
        let m = demo_matches();
        let all: Vec<&EventKind> = m.iter().flat_map(|m| m.events.iter().map(|e| &e.kind)).collect();
        assert!(all.iter().any(|k| matches!(k, EventKind::AgentDisconnected { .. })));
        assert!(all.iter().any(|k| matches!(k, EventKind::ServerRecovery { .. })));
        assert!(all.iter().any(|k| matches!(k, EventKind::MissingInterval { .. })));
        assert!(all.iter().any(|k| matches!(k, EventKind::Result { .. })));
        assert!(all.iter().any(|k| matches!(k, EventKind::Aborted { .. })));
        assert!(all.iter().any(|k| matches!(k, EventKind::Shot { .. })));
        assert!(all.iter().any(|k| matches!(k, EventKind::CommandDecision { accepted: false, .. })));
        assert!(!m[0].stalls.is_empty());
    }

    #[test]
    fn missing_interval_has_no_events_inside() {
        let m = &demo_matches()[2];
        let (f, t) = m.missing_intervals().next().expect("gap");
        assert!(m.events.iter().all(|e| matches!(e.kind, EventKind::MissingInterval { .. }) || e.tick < f || e.tick >= t));
        // Interpolation must not invent motion across the gap.
        let a = m.tank_at(0, f as f32 - 5.0).unwrap();
        let mid = m.tank_at(0, (f + t) as f32 / 2.0).unwrap();
        assert_eq!(a.pos, mid.pos);
    }

    #[test]
    fn ammunition_is_conserved() {
        for m in demo_matches() {
            let shots = |tank: usize| {
                m.events.iter().filter(|e| matches!(e.kind, EventKind::Shot { tank: t, .. } if t == tank)).count() as i32
            };
            for (i, seat) in m.seats.iter().enumerate() {
                if m.missing_intervals().next().is_some() {
                    continue;
                }
                let last = m.tank_at(i, m.end_tick as f32).unwrap();
                let start: i32 = seat.vehicle.demo_loadout().iter().map(|&a| a as i32).sum();
                let aboard: i32 = last.ammo.iter().map(|&a| a as i32).sum::<i32>() + last.loaded.is_some() as i32;
                // A round leaves the rack only when loading completes, so rounds fired,
                // rounds racked and the round in the breech account for the whole loadout.
                assert_eq!(start, aboard + shots(i), "match {} tank {i}", m.id);
            }
        }
    }

    #[test]
    fn duel_destroys_a_home_and_tanks_never_drive_through_standing_ones() {
        let all = demo_matches();
        let duel = all.iter().find(|m| m.id == "m-0412").unwrap();
        let last = duel.structures_at(duel.end_tick as f32).unwrap();
        assert!(last.iter().any(|(id, n, d)| id.starts_with('h') && *d && *n == 0), "{last:?}");
        for m in &all {
            for tick in (0..=m.end_tick).step_by(5) {
                let st = m.structures_at(tick as f32).unwrap_or(&[]);
                for i in 0..m.seats.len() {
                    let Some(t) = m.tank_at(i, tick as f32) else { continue };
                    for f in &lesmont::FOOTPRINTS {
                        let down = st.iter().any(|(id, _, d)| id == f.id && *d);
                        assert!(down || lesmont::rect_distance(t.pos, f.viewer_rect()) > 2.0, "{} tank {i} at {:?} inside {} tick {tick}", m.id, t.pos, f.id);
                    }
                }
            }
        }
    }
}