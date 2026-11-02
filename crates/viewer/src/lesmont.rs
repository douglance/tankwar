//! Lesmont shared geometry, copied from the "Lesmont shared geometry" table in
//! docs/CONTRACT.md. Footprints are in sim metres: x east, y north. The viewer
//! uses x east, z south, so viewer z = -sim y.

use bevy::math::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Home,
    Church,
    Wall,
}

#[derive(Clone, Copy, Debug)]
pub struct Footprint {
    pub id: &'static str,
    pub kind: Kind,
    /// Sim rectangle: (x0, y0, x1, y1).
    pub sim: (f32, f32, f32, f32),
}

impl Footprint {
    /// Viewer rectangle as (min, max) on the ground plane (x, z).
    pub fn viewer_rect(&self) -> (Vec2, Vec2) {
        let (x0, y0, x1, y1) = self.sim;
        (Vec2::new(x0, -y1), Vec2::new(x1, -y0))
    }
    pub fn center(&self) -> Vec2 {
        let (a, b) = self.viewer_rect();
        (a + b) * 0.5
    }
    pub fn size(&self) -> Vec2 {
        let (a, b) = self.viewer_rect();
        b - a
    }
    /// Integrity a fresh structure starts with; walls are permanent.
    pub fn max_integrity(&self) -> u32 {
        match self.kind {
            Kind::Home => 3,
            Kind::Church => 5,
            Kind::Wall => 0,
        }
    }
    pub fn contains(&self, p: Vec2) -> bool {
        let (a, b) = self.viewer_rect();
        p.x >= a.x && p.x <= b.x && p.y >= a.y && p.y <= b.y
    }
}

const fn h(id: &'static str, x0: f32, y0: f32, x1: f32, y1: f32) -> Footprint {
    Footprint { id, kind: Kind::Home, sim: (x0, y0, x1, y1) }
}

pub const FOOTPRINTS: [Footprint; 16] = [
    h("h1", -40.0, -70.0, -22.0, -50.0),
    h("h2", 16.0, -78.0, 36.0, -56.0),
    h("h3", -90.0, -20.0, -70.0, 2.0),
    h("h4", 60.0, -16.0, 82.0, 6.0),
    h("h5", -30.0, 24.0, -8.0, 46.0),
    h("h6", 20.0, 30.0, 42.0, 52.0),
    h("h7", -150.0, 40.0, -126.0, 64.0),
    h("h8", 120.0, -60.0, 146.0, -36.0),
    h("h9", -180.0, 80.0, -156.0, 104.0),
    h("h10", 160.0, 70.0, 184.0, 96.0),
    h("h11", -70.0, 90.0, -48.0, 112.0),
    h("h12", 70.0, -110.0, 94.0, -86.0),
    Footprint { id: "c1", kind: Kind::Church, sim: (-16.0, -130.0, 16.0, -96.0) },
    Footprint { id: "w1", kind: Kind::Wall, sim: (-220.0, -20.0, -160.0, -16.0) },
    Footprint { id: "w2", kind: Kind::Wall, sim: (150.0, 20.0, 220.0, 24.0) },
    Footprint { id: "w3", kind: Kind::Wall, sim: (-24.0, -150.0, -16.0, -132.0) },
];

pub fn footprint(id: &str) -> Option<&'static Footprint> {
    FOOTPRINTS.iter().find(|f| f.id == id)
}

/// Playable ground in sim metres.
pub const PLAY_X: f32 = 240.0;
pub const PLAY_Y: f32 = 180.0;

/// Sim position to viewer ground position.
pub fn to_viewer(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, -y)
}

/// Spawns from the contract, in viewer coordinates.
pub fn spawn_west() -> Vec2 {
    to_viewer(-200.0, 0.0)
}
pub fn spawn_east() -> Vec2 {
    to_viewer(200.0, 40.0)
}

/// Roads and lanes as sim polylines. The first is the main road; the rest are
/// farm lanes between the clusters.
pub const ROADS: &[&[(f32, f32)]] = &[
    &[(-1000.0, 10.0), (-200.0, 10.0), (-100.0, 12.0), (0.0, 12.0), (110.0, 12.0), (1000.0, 12.0)],
    &[(0.0, 8.0), (0.0, -90.0)],
    &[(5.0, 12.0), (5.0, 75.0), (-200.0, 75.0)],
    &[(105.0, 12.0), (105.0, -120.0), (30.0, -150.0), (-60.0, -160.0)],
    &[(130.0, 12.0), (135.0, 110.0), (135.0, 175.0)],
    &[(-120.0, 10.0), (-120.0, -170.0)],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crop {
    Wheat,
    Plowed,
    Orchard,
    Corn,
}

/// Fields as sim rectangles, kept clear of footprints and roads.
pub const FIELDS: &[(Crop, (f32, f32, f32, f32))] = &[
    (Crop::Wheat, (-235.0, -170.0, -128.0, -28.0)),
    (Crop::Plowed, (-112.0, -170.0, -30.0, -90.0)),
    (Crop::Orchard, (42.0, -80.0, 98.0, -28.0)),
    (Crop::Corn, (112.0, -170.0, 240.0, -72.0)),
    (Crop::Wheat, (142.0, 104.0, 240.0, 175.0)),
    (Crop::Plowed, (-235.0, 110.0, -80.0, 175.0)),
    (Crop::Orchard, (-42.0, 84.0, -2.0, 170.0)),
    (Crop::Wheat, (14.0, 84.0, 125.0, 175.0)),
];

pub fn field_rect(r: (f32, f32, f32, f32)) -> (Vec2, Vec2) {
    (Vec2::new(r.0, -r.3), Vec2::new(r.2, -r.1))
}

/// Viewer-space road polylines.
pub fn roads_viewer() -> Vec<Vec<Vec2>> {
    ROADS.iter().map(|r| r.iter().map(|&(x, y)| to_viewer(x, y)).collect()).collect()
}

fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t - p).length()
}

/// Distance from a viewer point to the nearest road centre line.
pub fn road_distance(p: Vec2) -> f32 {
    let mut d = f32::MAX;
    for r in ROADS {
        for w in r.windows(2) {
            d = d.min(seg_dist(p, to_viewer(w[0].0, w[0].1), to_viewer(w[1].0, w[1].1)));
        }
    }
    d
}

/// Distance from a viewer point to a footprint rectangle (0 inside).
pub fn rect_distance(p: Vec2, (a, b): (Vec2, Vec2)) -> f32 {
    let dx = (a.x - p.x).max(0.0).max(p.x - b.x);
    let dz = (a.y - p.y).max(0.0).max(p.y - b.y);
    (dx * dx + dz * dz).sqrt()
}

/// First parameter t in 0..1 where segment a->b enters the rectangle.
pub fn segment_hits_rect(a: Vec2, b: Vec2, (lo, hi): (Vec2, Vec2)) -> Option<f32> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, dp, l, h) in [(a.x, d.x, lo.x, hi.x), (a.y, d.y, lo.y, hi.y)] {
        if dp.abs() < 1e-6 {
            if p < l || p > h {
                return None;
            }
        } else {
            let (mut u, mut v) = ((l - p) / dp, (h - p) / dp);
            if u > v {
                std::mem::swap(&mut u, &mut v);
            }
            t0 = t0.max(u);
            t1 = t1.min(v);
            if t0 > t1 {
                return None;
            }
        }
    }
    Some(t0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_z_is_minus_sim_y() {
        let h1 = footprint("h1").unwrap();
        let (a, b) = h1.viewer_rect();
        assert_eq!((a, b), (Vec2::new(-40.0, 50.0), Vec2::new(-22.0, 70.0)));
        assert!(h1.contains(to_viewer(-30.0, -60.0)));
        assert!(!h1.contains(to_viewer(-30.0, 60.0)));
    }

    #[test]
    fn roads_and_fields_avoid_footprints() {
        for f in FOOTPRINTS.iter().filter(|f| f.kind != Kind::Wall) {
            let r = f.viewer_rect();
            for road in roads_viewer() {
                for w in road.windows(2) {
                    // Sample the centre line every metre.
                    let n = (w[1] - w[0]).length().ceil() as usize;
                    for i in 0..=n {
                        let p = w[0].lerp(w[1], i as f32 / n.max(1) as f32);
                        assert!(rect_distance(p, r) > 2.0, "road crosses {}", f.id);
                    }
                }
            }
            for &(_, fr) in FIELDS {
                let (a, b) = field_rect(fr);
                let overlap = a.x < r.1.x && b.x > r.0.x && a.y < r.1.y && b.y > r.0.y;
                assert!(!overlap, "field overlaps {}", f.id);
            }
        }
    }

    #[test]
    fn segment_rect_entry() {
        let r = (Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        let t = segment_hits_rect(Vec2::new(-10.0, 5.0), Vec2::new(10.0, 5.0), r).unwrap();
        assert!((t - 0.5).abs() < 1e-5);
        assert!(segment_hits_rect(Vec2::new(-10.0, 20.0), Vec2::new(10.0, 20.0), r).is_none());
    }
}
