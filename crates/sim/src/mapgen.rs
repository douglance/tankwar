//! Seeded wave function collapse for a 480 x 360 metre village grid.
//! Layouts are prototypes; geometry and vehicle clearance are not historical claims.
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const WIDTH: usize = 12;
pub const HEIGHT: usize = 9;
pub const TILE_METRES: u32 = 40;
// Four bits in east, north, west, south order. Tile 0..2 have no road.
const ROADS: [u8; 14] = [0, 0, 0, 5, 10, 3, 6, 12, 9, 7, 14, 13, 11, 15];
const ALL: u16 = (1 << ROADS.len()) - 1;
const DIRECTIONS: [(isize, isize, u8, u8); 4] = [(1,0,1,4),(0,1,2,8),(-1,0,4,1),(0,-1,8,2)];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Map { pub seed: u64, pub tiles: Vec<Tile> }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Tile { Ground, Trees, Home, Road { connections: u8 } }

fn compatible(a: usize, b: usize, exit: u8, entrance: u8) -> bool {
    (ROADS[a] & exit != 0) == (ROADS[b] & entrance != 0) && !(a == 2 && b == 2)
}
fn neighbor(index: usize, dx: isize, dy: isize) -> Option<usize> {
    let x = (index % WIDTH) as isize + dx;
    let y = (index / WIDTH) as isize + dy;
    (x >= 0 && x < WIDTH as isize && y >= 0 && y < HEIGHT as isize).then(|| y as usize * WIDTH + x as usize)
}
fn propagate(domains: &mut [u16], changes: impl IntoIterator<Item=usize>) -> bool {
    let mut queue: VecDeque<usize> = changes.into_iter().collect();
    while let Some(i) = queue.pop_front() {
        for (dx,dy,exit,entrance) in DIRECTIONS {
            let Some(j) = neighbor(i, dx, dy) else { continue };
            let mut keep = 0;
            for b in 0..ROADS.len() {
                if domains[j] & (1 << b) != 0 && (0..ROADS.len()).any(|a| domains[i] & (1 << a) != 0 && compatible(a,b,exit,entrance)) { keep |= 1 << b; }
            }
            if keep == 0 { return false; }
            if keep != domains[j] { domains[j] = keep; queue.push_back(j); }
        }
    }
    true
}
fn random(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}
fn collapse(domains: Vec<u16>, rng: &mut u64) -> Option<Vec<u16>> {
    let min = domains.iter().map(|d| d.count_ones()).filter(|n| *n > 1).min();
    let Some(min) = min else { return Some(domains) };
    let candidates: Vec<_> = domains.iter().enumerate().filter_map(|(i,d)| (d.count_ones() == min).then_some(i)).collect();
    let index = candidates[(random(rng) % candidates.len() as u64) as usize];
    let mut options: Vec<_> = (0..ROADS.len()).filter(|i| domains[index] & (1 << i) != 0).collect();
    for i in (1..options.len()).rev() { let j = (random(rng) % (i as u64 + 1)) as usize; options.swap(i,j); }
    for option in options {
        let mut next = domains.clone(); next[index] = 1 << option;
        if propagate(&mut next, [index]) { if let Some(result) = collapse(next, rng) { return Some(result); } }
    }
    None
}

/// Collapse domains with edge constraints and a guaranteed east-west central road.
/// The result is deterministic for a seed; callers must persist the seed and layout.
pub fn generate(seed: u64) -> Map {
    let mut domains = vec![ALL; WIDTH * HEIGHT];
    for (i, domain) in domains.iter_mut().enumerate() {
        let (x,y) = (i % WIDTH, i / WIDTH);
        for (_,_,exit,_) in DIRECTIONS {
            let boundary = match exit { 1 => x == WIDTH-1, 2 => y == HEIGHT-1, 4 => x == 0, _ => y == 0 };
            if boundary && !(y == 4 && (exit == 1 || exit == 4)) {
                for (tile,road) in ROADS.iter().enumerate() { if road & exit != 0 { *domain &= !(1 << tile); } }
            }
        }
        if y == 4 { for (tile, road) in ROADS.iter().enumerate() { if road & 5 != 5 { *domain &= !(1 << tile); } } }
        // Keep the east spawn and its teammate clear of obstacles.
        if y == 5 && x == 11 { *domain &= !((1 << 1) | (1 << 2)); }
    }
    assert!(propagate(&mut domains, 0..WIDTH*HEIGHT));
    let mut rng = seed;
    let domains = collapse(domains, &mut rng).expect("central-road constraints have a solution");
    let tiles = domains.into_iter().map(|domain| match domain.trailing_zeros() as usize {
        0 => Tile::Ground, 1 => Tile::Trees, 2 => Tile::Home, i => Tile::Road { connections: ROADS[i] },
    }).collect();
    Map { seed, tiles }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mask(tile: Tile) -> u8 { if let Tile::Road { connections } = tile { connections } else { 0 } }
    #[test]
    fn seeds_are_reproducible_varied_and_obey_constraints() {
        let first = generate(0);
        assert_eq!(first, generate(0));
        assert_ne!(first.tiles, generate(1).tiles);
        for seed in 0..40 {
            let map = generate(seed);
            assert_eq!(map.tiles.len(), WIDTH*HEIGHT);
            for (i, &tile) in map.tiles.iter().enumerate() {
                let (x,y) = (i % WIDTH, i / WIDTH);
                if y == 4 { assert_eq!(mask(tile) & 5, 5); }
                if x == 11 && y == 5 { assert!(!matches!(tile, Tile::Home | Tile::Trees)); }
                for (dx,dy,exit,entrance) in DIRECTIONS {
                    if let Some(j) = neighbor(i,dx,dy) {
                        assert_eq!(mask(tile) & exit != 0, mask(map.tiles[j]) & entrance != 0, "seed={seed} cell={i}");
                        assert!(!(tile == Tile::Home && map.tiles[j] == Tile::Home));
                    } else if !(y == 4 && (exit == 1 || exit == 4)) { assert_eq!(mask(tile) & exit, 0); }
                }
            }
        }
    }
}
