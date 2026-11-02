//! Approximate three-dimensional visibility and segment collision primitives.
//! Axis-aligned boxes are placeholders, not historically validated tank geometry.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vec3 { pub x: f64, pub y: f64, pub z: f64 }
impl Vec3 {
    fn sub(self, other: Self) -> Self { Self { x: self.x - other.x, y: self.y - other.y, z: self.z - other.z } }
    fn dot(self, other: Self) -> f64 { self.x * other.x + self.y * other.y + self.z * other.z }
    fn finite(self) -> bool { self.x.is_finite() && self.y.is_finite() && self.z.is_finite() }
}
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Bounds { pub min: Vec3, pub max: Vec3 }
impl Bounds {
    fn valid(&self) -> bool { self.min.finite() && self.max.finite() && self.min.x <= self.max.x && self.min.y <= self.max.y && self.min.z <= self.max.z }
    /// Fraction of the segment at its first contact, including its endpoints.
    pub fn intersection(&self, start: Vec3, end: Vec3) -> Option<f64> {
        if !start.finite() || !end.finite() || !self.valid() { return None; }
        let mut near: f64 = 0.0; let mut far: f64 = 1.0;
        for (origin, direction, min, max) in [(start.x, end.x-start.x, self.min.x, self.max.x), (start.y, end.y-start.y, self.min.y, self.max.y), (start.z, end.z-start.z, self.min.z, self.max.z)] {
            if min > max { return None; }
            if direction.abs() < f64::EPSILON { if origin < min || origin > max { return None; } continue; }
            let a = (min-origin)/direction; let b = (max-origin)/direction;
            near = near.max(a.min(b)); far = far.min(a.max(b));
            if near > far { return None; }
        }
        Some(near)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Camera { pub position: Vec3, pub forward: Vec3, pub horizontal_half_angle_radians: f64, pub max_range: f64 }
impl Camera {
    /// Checks a point observation against the sight cone and intervening opaque boxes.
    pub fn sees_point(&self, target: Vec3, obstacles: &[Bounds]) -> bool {
        if !self.position.finite() || !self.forward.finite() || !target.finite() || !self.max_range.is_finite() || self.max_range <= 0.0 || !self.horizontal_half_angle_radians.is_finite() || !(0.0..=std::f64::consts::PI).contains(&self.horizontal_half_angle_radians) { return false; }
        if obstacles.iter().any(|b| !b.valid()) { return false; }
        let delta = target.sub(self.position); let distance2 = delta.dot(delta); let forward2 = self.forward.dot(self.forward);
        if distance2 == 0.0 || forward2 == 0.0 || distance2 > self.max_range * self.max_range { return false; }
        let cos = delta.dot(self.forward) / (distance2.sqrt() * forward2.sqrt());
        if cos < self.horizontal_half_angle_radians.cos() { return false; }
        !obstacles.iter().any(|b| b.intersection(self.position, target).is_some())
    }
}

/// Returns the nearest collidable object and its contact fraction.
pub fn first_contact(start: Vec3, end: Vec3, objects: &[Bounds]) -> Option<(usize, f64)> {
    objects.iter().enumerate().filter_map(|(i, bounds)| bounds.intersection(start, end).map(|t| (i, t))).min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn v(x: f64,y: f64,z: f64)->Vec3 { Vec3{x,y,z} }
    fn wall(x:f64)->Bounds { Bounds { min:v(x,-1.,-1.), max:v(x+1.,1.,1.) } }
    #[test] fn occlusion_range_and_sight_cone() {
        let c=Camera { position:v(0.,0.,0.),forward:v(1.,0.,0.),horizontal_half_angle_radians:0.3,max_range:20. };
        assert!(c.sees_point(v(10.,0.,0.),&[]));
        assert!(!c.sees_point(v(10.,0.,0.),&[wall(5.)]));
        assert!(!c.sees_point(v(-1.,0.,0.),&[]));
        assert!(!c.sees_point(v(21.,0.,0.),&[]));
        assert!(!c.sees_point(v(10.,10.,0.),&[]));
    }
    #[test] fn nearest_hit_and_miss() {
        let hit=first_contact(v(0.,0.,0.),v(10.,0.,0.),&[wall(8.),wall(3.)]).unwrap();
        assert_eq!(hit.0,1); assert!((hit.1-0.3).abs()<1e-12);
        assert!(first_contact(v(0.,2.,0.),v(10.,2.,0.),&[wall(3.)]).is_none());
    }
    #[test] fn invalid_numeric_input_is_not_visible() {
        let c=Camera { position:v(0.,0.,0.),forward:v(1.,0.,0.),horizontal_half_angle_radians:0.3,max_range:20. };
        assert!(!c.sees_point(v(f64::NAN,0.,0.),&[]));
        assert!(!c.sees_point(v(10.,0.,0.),&[Bounds { min:v(f64::NAN,0.,0.), max:v(1.,1.,1.) }]));
    }
}
