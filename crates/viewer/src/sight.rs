//! The agent's view: the sight report one seat is given, rebuilt by the
//! viewer from the spectator replay.
//!
//! An agent gets no picture. It gets a hull-fixed forward sight report:
//! contacts inside a 60 degree cone out to 1000 m, blocked by standing
//! buildings, walls and other vehicles, with bearing rounded to 5 degrees,
//! range rounded to 50 m, and no names or sides. This module applies the
//! server's published prototype sight rule (docs/CONTRACT.md, Lesmont shared
//! geometry) to replay snapshots. The replay does not record the reports the
//! agent actually received, so the panel says this is a reconstruction.

use crate::feed::{Match, TankSnap};
use crate::lesmont::{Kind, FOOTPRINTS};
use bevy::math::Vec2;

/// Half the horizontal sight cone, degrees.
pub const HALF_ANGLE_DEG: f64 = 30.0;
/// Sight range, metres.
pub const MAX_RANGE_M: f64 = 1000.0;
/// Half the side of the box a vehicle blocks sight with, metres.
const VEHICLE_HALF_EXTENT_M: f64 = 2.0;

/// One contact as the agent receives it: no identity, no side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Contact {
    /// Degrees from the hull's forward line; positive is to the left.
    pub bearing_deg: i32,
    pub range_m: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sight {
    Report(Vec<Contact>),
    /// A knocked-out vehicle gets no sight report.
    Destroyed,
}

/// Sim ground point (x east, y north) from a feed point (x east, z south).
fn to_sim(p: Vec2) -> (f64, f64) {
    (p.x as f64, -(p.y as f64))
}

/// Sim rectangle (x0, y0, x1, y1) blocks the segment a->b.
fn blocks((x0, y0, x1, y1): (f64, f64, f64, f64), a: (f64, f64), b: (f64, f64)) -> bool {
    let (mut near, mut far) = (0.0f64, 1.0f64);
    for (o, d, lo, hi) in [(a.0, b.0 - a.0, x0, x1), (a.1, b.1 - a.1, y0, y1)] {
        if d.abs() < f64::EPSILON {
            if o < lo || o > hi {
                return false;
            }
            continue;
        }
        let (u, v) = ((lo - o) / d, (hi - o) / d);
        near = near.max(u.min(v));
        far = far.min(u.max(v));
        if near > far {
            return false;
        }
    }
    true
}

/// The report `observer` gets, given every other vehicle's position and the
/// latest structure states (`None` means none reported yet: all standing).
pub fn look(observer: &TankSnap, others: &[Vec2], structures: Option<&[(String, u32, bool)]>) -> Sight {
    if observer.knocked_out {
        return Sight::Destroyed;
    }
    let me = to_sim(observer.pos);
    let heading = observer.heading as f64 - std::f64::consts::FRAC_PI_2;
    let standing = |id: &str| match structures.and_then(|s| s.iter().find(|(sid, _, _)| sid == id)) {
        Some((_, integrity, destroyed)) => *integrity > 0 && !destroyed,
        None => true,
    };
    let mut cover: Vec<(f64, f64, f64, f64)> = FOOTPRINTS
        .iter()
        .filter(|f| f.kind == Kind::Wall || standing(f.id))
        .map(|f| (f.sim.0 as f64, f.sim.1 as f64, f.sim.2 as f64, f.sim.3 as f64))
        .collect();
    let base = cover.len();
    let r = VEHICLE_HALF_EXTENT_M;
    cover.extend(others.iter().map(|p| {
        let (x, y) = to_sim(*p);
        (x - r, y - r, x + r, y + r)
    }));
    let mut contacts = Vec::new();
    for (i, p) in others.iter().enumerate() {
        let t = to_sim(*p);
        let (dx, dy) = (t.0 - me.0, t.1 - me.1);
        let dist = dx.hypot(dy);
        if dist == 0.0 || dist > MAX_RANGE_M {
            continue;
        }
        let cos = (dx * heading.cos() + dy * heading.sin()) / dist;
        if cos < HALF_ANGLE_DEG.to_radians().cos() {
            continue;
        }
        let blocked = cover.iter().enumerate().any(|(k, c)| k != base + i && blocks(*c, me, t));
        if blocked {
            continue;
        }
        let bearing = (dy.atan2(dx) - heading + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
        contacts.push(Contact {
            bearing_deg: (bearing.to_degrees() / 5.0).round() as i32 * 5,
            range_m: ((dist / 50.0).round() as u32).max(1).saturating_mul(50),
        });
    }
    contacts.sort_by_key(|c| (c.bearing_deg, c.range_m));
    Sight::Report(contacts)
}

/// The report seat `tank` gets at a replay tick, or `None` before it has a
/// snapshot.
pub fn look_at(m: &Match, tank: usize, tick: f32) -> Option<Sight> {
    let me = m.tank_at(tank, tick)?;
    let others: Vec<Vec2> = (0..m.seats.len()).filter(|&i| i != tank).filter_map(|i| m.tank_at(i, tick)).map(|s| s.pos).collect();
    Some(look(&me, &others, m.structures_at(tick)))
}

/// Plain-language bearing, as a crew member would call it.
pub fn bearing_words(deg: i32) -> String {
    match deg {
        0 => "dead ahead".to_string(),
        d if d > 0 => format!("{d} deg left"),
        d => format!("{} deg right", -d),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tank at sim (x, y) facing `sim_heading_deg` (counterclockwise from east).
    fn at(x: f32, y: f32, sim_heading_deg: f32) -> TankSnap {
        let mut s = crate::feed::demo_matches()[0].tank_at(0, 0.0).unwrap();
        s.pos = Vec2::new(x, -y);
        s.heading = crate::contract::sim_heading_to_feed(sim_heading_deg.to_radians());
        s.knocked_out = false;
        s
    }
    fn p(x: f32, y: f32) -> Vec2 {
        Vec2::new(x, -y)
    }

    #[test]
    fn report_rounds_bearing_and_range_and_drops_targets_outside_the_cone() {
        // Open ground south of the village, observer facing east.
        // Target 1 at +100 east, +50 north: atan(0.5) = 26.57 deg left -> 25,
        // 111.8 m -> 100 m. Target 2 at 38.7 deg right is outside 30 deg.
        // Target 3 is 1200 m out, beyond range. Target 4 is behind.
        let me = at(-200.0, -250.0, 0.0);
        let others = [p(-100.0, -200.0), p(-100.0, -330.0), p(1000.0, -250.0), p(-300.0, -250.0)];
        assert_eq!(look(&me, &others, None), Sight::Report(vec![Contact { bearing_deg: 25, range_m: 100 }]));
        // Right is negative: 100 east, 30 south is 16.7 deg right -> -15.
        assert_eq!(look(&me, &[p(-100.0, -280.0)], None), Sight::Report(vec![Contact { bearing_deg: -15, range_m: 100 }]));
    }

    #[test]
    fn standing_house_blocks_sight_and_a_destroyed_one_does_not() {
        // House h1 spans x -40..-22, y -70..-50. Looking north through it.
        let me = at(-31.0, -100.0, 90.0);
        let target = [p(-31.0, -20.0)];
        assert_eq!(look(&me, &target, None), Sight::Report(vec![]));
        let razed = vec![("h1".to_string(), 0u32, true)];
        assert_eq!(look(&me, &target, Some(&razed)), Sight::Report(vec![Contact { bearing_deg: 0, range_m: 100 }]));
    }

    #[test]
    fn another_vehicle_blocks_sight_and_a_knocked_out_seat_gets_nothing() {
        let me = at(-200.0, -250.0, 0.0);
        let others = [p(-150.0, -250.0), p(-100.0, -250.0)];
        assert_eq!(look(&me, &others, None), Sight::Report(vec![Contact { bearing_deg: 0, range_m: 50 }]));
        let mut dead = me;
        dead.knocked_out = true;
        assert_eq!(look(&dead, &others, None), Sight::Destroyed);
    }
}
