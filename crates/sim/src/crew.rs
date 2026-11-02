//! Deterministic prototype crew. Navigation and vehicle characteristics are approximations.
use crate::{geometry::{Bounds, Camera, Vec3, first_contact}, world::World};
use tankwar_protocol::Order;

fn expanded(b: Bounds, r: f64) -> Bounds {
    Bounds { min:Vec3 { x:b.min.x-r,y:b.min.y-r,z:b.min.z-r }, max:Vec3 { x:b.max.x+r,y:b.max.y+r,z:b.max.z+r } }
}
fn distance(a: Vec3, b: Vec3) -> f64 { (a.x-b.x).hypot(a.y-b.y) }
fn clear(a: Vec3, b: Vec3, obstacles: &[Bounds]) -> bool { obstacles.iter().all(|o| o.intersection(a,b).is_none()) }
fn angle(value: f64) -> f64 { (value + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI }

/// Generate crew orders using the authoritative world; the caller records them as commands.
pub fn orders(world: &World, seat: &str) -> Vec<Order> {
    let Some(own) = world.vehicles.get(seat) else { return vec![] };
    if own.hit_points == 0 { return vec![]; }
    let Some((enemy_id,enemy)) = world.vehicles.iter().filter(|(_,v)| v.team != own.team && v.hit_points > 0)
        .min_by(|a,b| distance(a.1.position,own.position).total_cmp(&distance(b.1.position,own.position))) else { return vec![]; };
    let solid: Vec<_> = world.obstacles.iter().copied().chain(world.structures.iter().filter(|s| s.integrity > 0).map(|s| s.bounds)).collect();
    let camera = Camera { position:enemy.position, forward:Vec3{x:enemy.heading_radians.cos(),y:enemy.heading_radians.sin(),z:0.}, horizontal_half_angle_radians:30_f64.to_radians(), max_range:1000. };
    let exposed = camera.sees_point(own.position, &solid);
    let mut destination = if own.position.x < -100. { Vec3{x:-105.,y:120.,z:0.} }
        else if own.position.x > 100. { Vec3{x:105.,y:120.,z:0.} }
        else { Vec3{x:enemy.position.x, y:120.,z:0.} };
    if exposed {
        let mut candidates = Vec::new();
        for s in world.structures.iter().filter(|s| s.integrity > 0) {
            let b = s.bounds; let r = own.half_extent_metres + 2.;
            for p in [Vec3{x:b.min.x-r,y:(b.min.y+b.max.y)/2.,z:0.}, Vec3{x:b.max.x+r,y:(b.min.y+b.max.y)/2.,z:0.}, Vec3{x:(b.min.x+b.max.x)/2.,y:b.min.y-r,z:0.}, Vec3{x:(b.min.x+b.max.x)/2.,y:b.max.y+r,z:0.}] {
                if b.intersection(enemy.position,p).is_some() { candidates.push(p); }
            }
        }
        if let Some(p) = candidates.into_iter().min_by(|a,b| distance(own.position,*a).total_cmp(&distance(own.position,*b))) { destination = p; }
    } else {
        let camera = Camera { position:own.position, forward:Vec3{x:own.heading_radians.cos(),y:own.heading_radians.sin(),z:0.}, horizontal_half_angle_radians:30_f64.to_radians(), max_range:1000. };
        if camera.sees_point(enemy.position, &solid) { destination = enemy.position; }
    }
    // Route through a visible expanded-obstacle corner when the direct path is blocked.
    let expanded: Vec<_> = solid.iter().map(|b| expanded(*b,own.half_extent_metres+0.5)).collect();
    if !clear(own.position,destination,&expanded) {
        let mut candidates = Vec::new();
        for b in &expanded {
            for p in [Vec3{x:b.min.x-1.,y:b.min.y-1.,z:0.}, Vec3{x:b.min.x-1.,y:b.max.y+1.,z:0.}, Vec3{x:b.max.x+1.,y:b.min.y-1.,z:0.}, Vec3{x:b.max.x+1.,y:b.max.y+1.,z:0.}] {
                if clear(own.position,p,&expanded) { candidates.push(p); }
            }
        }
        if let Some(p) = candidates.into_iter().min_by(|a,b| (distance(own.position,*a)+distance(*a,destination)).total_cmp(&(distance(own.position,*b)+distance(*b,destination)))) { destination=p; }
    }
    let desired=(destination.y-own.position.y).atan2(destination.x-own.position.x);
    let error=angle(desired-own.heading_radians);
    let mut result=Vec::new();
    if !own.tank.engine_running && own.tank.fuel_ml > 0 { result.push(Order::Engine { running:true }); }
    result.push(Order::Drive { throttle:if error.abs()<0.35 { 1000 } else { 0 }, steering:if error.abs()<0.03 { 0 } else if error>0. { 1000 } else { -1000 }, ttl_ticks:12 });
    if own.tank.chamber.is_none() && own.tank.loading.is_none() {
        if let Some((kind,_))=own.tank.ammunition.iter().find(|(_,n)| **n>0) { result.push(Order::Load { ammunition:kind.clone() }); }
    }
    if own.tank.chamber.is_some() {
        let start=own.position;
        let end=Vec3{x:start.x+own.heading_radians.cos()*2000.,y:start.y+own.heading_radians.sin()*2000.,z:start.z};
        let mut bounds=solid;
        let solid_count=bounds.len();
        let targets:Vec<_>=world.vehicles.iter().filter(|(id,v)| id.as_str()!=seat && v.hit_points>0).collect();
        for (_,v) in &targets {
            let r=v.half_extent_metres;
            bounds.push(Bounds { min:Vec3{x:v.position.x-r,y:v.position.y-r,z:v.position.z-r},max:Vec3{x:v.position.x+r,y:v.position.y+r,z:v.position.z+r} });
        }
        if first_contact(start,end,&bounds).and_then(|(i,_)| i.checked_sub(solid_count)).and_then(|i| targets.get(i)).is_some_and(|(id,_)| *id==enemy_id) { result.push(Order::Fire); }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Tank, world::{Structure, Vehicle}};
    use std::collections::BTreeMap;
    fn vehicle(team:u8,x:f64) -> Vehicle {
        Vehicle { tank:Tank { fuel_ml:100, idle_fuel_ml_per_tick:0, engine_running:true, ammunition:BTreeMap::new(), chamber:Some("ap".into()), reload_ticks:1, loading:None, throttle:0, steering:0, drive_expires_at:None }, team, position:Vec3{x,y:0.,z:0.}, heading_radians:0., speed_metres_per_second:10., turn_radians_per_second:1., half_extent_metres:1., hit_points:1 }
    }
    #[test] fn fires_only_when_the_enemy_is_the_first_contact() {
        let mut w=World::new(BTreeMap::from([("own".into(),vehicle(0,0.)),("enemy".into(),vehicle(1,50.))]),vec![]);
        assert!(orders(&w,"own").contains(&Order::Fire));
        w.structures.push(Structure { id:"home".into(), bounds:Bounds { min:Vec3{x:20.,y:-2.,z:-2.},max:Vec3{x:25.,y:2.,z:2.} }, integrity:3 });
        assert!(!orders(&w,"own").contains(&Order::Fire));
        w.structures[0].integrity=0;
        w.vehicles.insert("ally".into(),vehicle(0,30.));
        assert!(!orders(&w,"own").contains(&Order::Fire));
        w.vehicles.remove("ally");
        assert!(orders(&w,"own").contains(&Order::Fire));
    }
}
