//! The arena: the fictional village of Lesmont, a rural European crossroads.
//!
//! Layout (x east, z south, meters):
//! - village center at the origin, a small square where the road meets a lane
//! - one west-east road through the center, the only paved line on the map
//! - farm fields north and south of the road, edged with hedges
//! - a tree line along the north edge
//! - open west and east approaches where the two sides enter
//!
//! Houses, church, walls and barns are flat-shaded procedural boxes. Trees,
//! bushes, crops, rocks, fences and the road surface come from the Kenney CC0
//! kits. Ground texture is Poly Haven's CC0 brown mud with leaves.

use bevy::asset::RenderAssetUsages;
use bevy::gltf::Gltf;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::math::primitives::{Cone, Cuboid, Extrusion, Triangle2d};
use bevy::pbr::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub const SKY: Color = Color::srgb(0.70, 0.73, 0.74);
pub const HALF_X: f32 = 380.0;
pub const HALF_Z: f32 = 280.0;
const CELL: f32 = 4.0;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (light, terrain, village, nature, road))
            .add_systems(Update, restyle_kits);
    }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Ground height in meters. Flat along the road and in the village, rolling
/// gently elsewhere, rising into a low ridge behind the north tree line.
pub fn height(x: f32, z: f32) -> f32 {
    let roll = 2.0 * (x * 0.011 + 0.7).sin() * (z * 0.013).cos()
        + 1.2 * (x * 0.027 + z * 0.019).sin()
        + 0.5 * (x * 0.061 - z * 0.047).sin();
    let road = smoothstep(8.0, 30.0, z.abs());
    let village = smoothstep(70.0, 120.0, (x * x + z * z * 1.6).sqrt());
    let ridge = (-z - 140.0).max(0.0) * 0.07;
    roll * road.min(village) + ridge
}

pub fn normal(x: f32, z: f32) -> Vec3 {
    let e = 1.5;
    let dx = height(x + e, z) - height(x - e, z);
    let dz = height(x, z + e) - height(x, z - e);
    Vec3::new(-dx, 2.0 * e, -dz).normalize()
}

#[derive(Clone, Copy, PartialEq)]
enum Ground {
    Meadow,
    Wheat,
    Plowed,
    Corn,
}

/// Field rectangles: x0, z0, x1, z1, crop.
const FIELDS: [(f32, f32, f32, f32, Ground); 4] = [
    (-210.0, -115.0, -70.0, -42.0, Ground::Wheat),
    (70.0, -115.0, 215.0, -42.0, Ground::Plowed),
    (-225.0, 42.0, -75.0, 120.0, Ground::Corn),
    (75.0, 42.0, 210.0, 112.0, Ground::Wheat),
];

fn ground_at(x: f32, z: f32) -> Ground {
    for (x0, z0, x1, z1, g) in FIELDS {
        if x >= x0 && x <= x1 && z >= z0 && z <= z1 {
            return g;
        }
    }
    Ground::Meadow
}

fn tint(x: f32, z: f32) -> [f32; 4] {
    let n = (hash2(x as i32, z as i32) % 1000) as f32 / 1000.0 * 0.08 - 0.04;
    let mut c = match ground_at(x, z) {
        Ground::Meadow => [0.80, 0.88, 0.58],
        Ground::Wheat => [1.30, 1.15, 0.72],
        Ground::Plowed => [0.95, 0.78, 0.62],
        Ground::Corn => [0.92, 1.0, 0.60],
    };
    let lane = x.abs() < 3.5 && z.abs() < 70.0;
    let square = x * x + z * z < 22.0 * 22.0;
    let yards = x.abs() < 75.0 && z.abs() < 32.0;
    if lane || square {
        c = [1.15, 1.02, 0.86];
    } else if yards {
        c = [0.92, 0.94, 0.70];
    }
    [c[0] + n, c[1] + n, c[2] + n * 0.5, 1.0]
}

fn hash2(a: i32, b: i32) -> u32 {
    let mut x = (a as u32).wrapping_mul(0x9E37_79B9) ^ (b as u32).wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE35);
    x ^ (x >> 16)
}

/// Small deterministic generator so the village is identical on every load.
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407))
    }
    pub fn f(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32) / (1u64 << 31) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[((self.f() * xs.len() as f32) as usize).min(xs.len() - 1)]
    }
}

fn light(mut commands: Commands) {
    commands.insert_resource(AmbientLight { color: Color::srgb(0.85, 0.87, 0.92), brightness: 450.0, ..default() });
    commands.spawn((
        DirectionalLight { illuminance: 11000.0, color: Color::srgb(1.0, 0.95, 0.86), shadows_enabled: true, ..default() },
        Transform::from_xyz(-160.0, 190.0, 120.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder { maximum_distance: 420.0, first_cascade_far_bound: 45.0, ..default() }.build(),
    ));
}

fn terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    let nx = (2.0 * HALF_X / CELL) as i32;
    let nz = (2.0 * HALF_Z / CELL) as i32;
    let mut pos = Vec::new();
    let mut nor = Vec::new();
    let mut uv = Vec::new();
    let mut col = Vec::new();
    for iz in 0..nz {
        for ix in 0..nx {
            let x0 = -HALF_X + ix as f32 * CELL;
            let z0 = -HALF_Z + iz as f32 * CELL;
            let p = |dx: f32, dz: f32| Vec3::new(x0 + dx, height(x0 + dx, z0 + dz), z0 + dz);
            let (a, b, c, d) = (p(0.0, 0.0), p(CELL, 0.0), p(CELL, CELL), p(0.0, CELL));
            for tri in [[a, d, c], [a, c, b]] {
                let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize();
                let center = (tri[0] + tri[1] + tri[2]) / 3.0;
                let t = tint(center.x, center.z);
                for v in tri {
                    pos.push(v.to_array());
                    nor.push(n.to_array());
                    uv.push([v.x / 9.0, v.z / 9.0]);
                    col.push(t);
                }
            }
        }
    }
    let count = pos.len() as u32;
    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32((0..count).collect()));
    let tex: Handle<Image> = assets.load_with_settings(
        "ground/brown_mud_leaves_01_diff_1k.jpg",
        |s: &mut ImageLoaderSettings| {
            s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                ..ImageSamplerDescriptor::linear()
            });
        },
    );
    commands.spawn((
        Name::new("Terrain"),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.78, 0.74, 0.66),
            base_color_texture: Some(tex),
            perceptual_roughness: 0.97,
            reflectance: 0.2,
            ..default()
        })),
    ));
}

struct Kit {
    wall: Handle<Mesh>,
    roof: Handle<Mesh>,
    spire: Handle<Mesh>,
    plaster: Vec<Handle<StandardMaterial>>,
    roofs: Vec<Handle<StandardMaterial>>,
    dark: Handle<StandardMaterial>,
    timber: Handle<StandardMaterial>,
    stone: Handle<StandardMaterial>,
}

fn mat(materials: &mut Assets<StandardMaterial>, r: f32, g: f32, b: f32) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial { base_color: Color::srgb(r, g, b), perceptual_roughness: 0.92, ..default() })
}

/// One house: plaster walls, a gable roof, a door, windows and a chimney.
fn house(commands: &mut Commands, k: &Kit, rng: &mut Rng, at: Vec2, rot: f32, w: f32, d: f32, h: f32) {
    let y = height(at.x, at.y);
    let plaster = rng.pick(&k.plaster).clone();
    let roof = rng.pick(&k.roofs).clone();
    let rh = w * rng.range(0.45, 0.6);
    commands
        .spawn((Name::new("House"), Transform::from_xyz(at.x, y, at.y).with_rotation(Quat::from_rotation_y(rot)), Visibility::default()))
        .with_children(|p| {
            p.spawn((Mesh3d(k.wall.clone()), MeshMaterial3d(plaster), Transform::from_xyz(0.0, h / 2.0 - 0.3, 0.0).with_scale(Vec3::new(w, h + 0.6, d))));
            p.spawn((
                Mesh3d(k.roof.clone()),
                MeshMaterial3d(roof),
                Transform::from_xyz(0.0, h, 0.0).with_rotation(Quat::from_rotation_y(FRAC_PI_2)).with_scale(Vec3::new(w + 0.8, rh, d + 0.8)),
            ));
            // The roof mesh is extruded along its local z; rotate so the ridge runs along the house's length.
            let face = w / 2.0 + 0.04;
            p.spawn((Mesh3d(k.wall.clone()), MeshMaterial3d(k.timber.clone()), Transform::from_xyz(face, 1.1, 0.0).with_scale(Vec3::new(0.1, 2.2, 1.2))));
            let n = ((d - 2.0) / 3.0).floor().max(1.0) as i32;
            for i in 0..n {
                let zz = -d / 2.0 + (i as f32 + 0.5) * d / n as f32;
                if (zz).abs() < 0.9 {
                    continue;
                }
                for side in [-1.0, 1.0] {
                    p.spawn((Mesh3d(k.wall.clone()), MeshMaterial3d(k.dark.clone()), Transform::from_xyz(side * face, h * 0.62, zz).with_scale(Vec3::new(0.1, 1.1, 0.8))));
                }
            }
            p.spawn((Mesh3d(k.wall.clone()), MeshMaterial3d(k.stone.clone()), Transform::from_xyz(-w * 0.22, h + rh * 0.75, d * 0.25).with_scale(Vec3::new(0.7, rh * 0.9, 0.7))));
        });
}

fn wall_run(commands: &mut Commands, k: &Kit, a: Vec2, b: Vec2) {
    let mid = (a + b) / 2.0;
    let len = a.distance(b);
    let ang = (b.x - a.x).atan2(b.y - a.y);
    commands.spawn((
        Name::new("Stone wall"),
        Mesh3d(k.wall.clone()),
        MeshMaterial3d(k.stone.clone()),
        Transform::from_xyz(mid.x, height(mid.x, mid.y) + 0.45, mid.y).with_rotation(Quat::from_rotation_y(ang)).with_scale(Vec3::new(0.6, 1.2, len)),
    ));
}

fn village(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let roof_mesh = Extrusion::new(Triangle2d::new(Vec2::new(-0.5, 0.0), Vec2::new(0.5, 0.0), Vec2::new(0.0, 1.0)), 1.0);
    let k = Kit {
        wall: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        roof: meshes.add(roof_mesh),
        spire: meshes.add(Cone::new(0.72, 1.0).mesh().resolution(4)),
        plaster: vec![
            mat(&mut materials, 0.84, 0.79, 0.68),
            mat(&mut materials, 0.78, 0.68, 0.50),
            mat(&mut materials, 0.80, 0.78, 0.73),
            mat(&mut materials, 0.72, 0.66, 0.58),
        ],
        roofs: vec![mat(&mut materials, 0.52, 0.27, 0.19), mat(&mut materials, 0.44, 0.24, 0.18), mat(&mut materials, 0.30, 0.30, 0.32)],
        dark: mat(&mut materials, 0.16, 0.14, 0.12),
        timber: mat(&mut materials, 0.33, 0.24, 0.17),
        stone: mat(&mut materials, 0.58, 0.56, 0.51),
    };
    let mut rng = Rng::new(1918);

    // Houses face the road on both sides, leaving the square open.
    for (x, side) in [(-62.0, -1.0), (-46.0, -1.0), (-30.0, -1.0), (22.0, -1.0), (38.0, -1.0), (56.0, -1.0), (-58.0, 1.0), (-40.0, 1.0), (-22.0, 1.0), (24.0, 1.0), (42.0, 1.0), (60.0, 1.0)] {
        let w = rng.range(7.0, 9.0);
        let d = rng.range(10.0, 14.0);
        let h = rng.range(4.5, 6.5);
        let z = side * (13.0 + w / 2.0 + rng.range(0.0, 2.0));
        // Long side faces the road: the house's local x axis points across the road.
        house(&mut commands, &k, &mut rng, Vec2::new(x, z), if side < 0.0 { FRAC_PI_2 } else { -FRAC_PI_2 }, w, d, h);
    }
    // Second row and outlying farms.
    for (x, z, rot) in [(-50.0, -42.0, 0.2), (46.0, -44.0, -0.1), (-34.0, 46.0, 0.05), (36.0, 48.0, 1.5), (-84.0, 30.0, 0.0), (86.0, -30.0, 1.57)] {
        let barn = (x as f32).abs() > 80.0;
        let (w, d, h) = if barn { (11.0, 18.0, 6.5) } else { (rng.range(7.0, 9.0), rng.range(10.0, 13.0), rng.range(4.5, 6.0)) };
        house(&mut commands, &k, &mut rng, Vec2::new(x, z), rot, w, d, h);
    }

    // Church north of the square: nave, tower toward the square, a pyramid spire.
    let church = Vec2::new(0.0, -44.0);
    let y = height(church.x, church.y);
    commands
        .spawn((Name::new("Church"), Transform::from_xyz(church.x, y, church.y), Visibility::default()))
        .with_children(|p| {
            p.spawn((Mesh3d(k.wall.clone()), MeshMaterial3d(k.stone.clone()), Transform::from_xyz(0.0, 4.0, -4.0).with_scale(Vec3::new(10.0, 8.6, 22.0))));
            p.spawn((
                Mesh3d(k.roof.clone()),
                MeshMaterial3d(k.roofs[2].clone()),
                Transform::from_xyz(0.0, 8.3, -4.0).with_scale(Vec3::new(11.0, 5.0, 23.0)),
            ));
            p.spawn((Mesh3d(k.wall.clone()), MeshMaterial3d(k.stone.clone()), Transform::from_xyz(0.0, 9.0, 9.0).with_scale(Vec3::new(5.5, 18.0, 5.5))));
            p.spawn((Mesh3d(k.spire.clone()), MeshMaterial3d(k.roofs[2].clone()), Transform::from_xyz(0.0, 18.0 + 4.0, 9.0).with_rotation(Quat::from_rotation_y(PI / 4.0)).with_scale(Vec3::new(5.4, 8.0, 5.4))));
            for zz in [-12.0, -7.0, -2.0, 3.0] {
                for side in [-1.0, 1.0] {
                    p.spawn((Mesh3d(k.wall.clone()), MeshMaterial3d(k.dark.clone()), Transform::from_xyz(side * 5.04, 5.0, zz).with_scale(Vec3::new(0.1, 3.2, 1.1))));
                }
            }
            p.spawn((Mesh3d(k.wall.clone()), MeshMaterial3d(k.dark.clone()), Transform::from_xyz(0.0, 1.4, 11.8).with_scale(Vec3::new(1.6, 2.8, 0.1))));
            p.spawn((Mesh3d(k.wall.clone()), MeshMaterial3d(k.dark.clone()), Transform::from_xyz(0.0, 14.0, 11.8).with_scale(Vec3::new(1.0, 1.6, 0.1))));
        });
    // Churchyard wall with a gap for the lane.
    let (x0, x1, z0, z1) = (-12.0, 12.0, -64.0, -28.0);
    wall_run(&mut commands, &k, Vec2::new(x0, z0), Vec2::new(x1, z0));
    wall_run(&mut commands, &k, Vec2::new(x0, z0), Vec2::new(x0, z1));
    wall_run(&mut commands, &k, Vec2::new(x1, z0), Vec2::new(x1, z1));
    wall_run(&mut commands, &k, Vec2::new(x0, z1), Vec2::new(-3.0, z1));
    wall_run(&mut commands, &k, Vec2::new(3.0, z1), Vec2::new(x1, z1));
    // Field walls along the approaches give cover on both flanks.
    wall_run(&mut commands, &k, Vec2::new(-150.0, -30.0), Vec2::new(-110.0, -30.0));
    wall_run(&mut commands, &k, Vec2::new(115.0, 30.0), Vec2::new(160.0, 30.0));
    // A village well on the square.
    commands.spawn((Name::new("Well"), Mesh3d(k.wall.clone()), MeshMaterial3d(k.stone.clone()), Transform::from_xyz(-9.0, 0.5, -8.0).with_scale(Vec3::new(2.2, 1.0, 2.2))));
}

fn kit_scene(assets: &AssetServer, path: &str) -> SceneRoot {
    SceneRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path.to_string())))
}

fn place(commands: &mut Commands, assets: &AssetServer, path: &str, x: f32, z: f32, scale: f32, rot: f32) {
    commands.spawn((
        kit_scene(assets, path),
        Transform::from_xyz(x, height(x, z) - 0.05, z).with_rotation(Quat::from_rotation_y(rot)).with_scale(Vec3::splat(scale)),
    ));
}

const BROADLEAF: [&str; 4] = ["nature/tree_oak.glb", "nature/tree_default.glb", "nature/tree_detailed.glb", "nature/tree_fat.glb"];
const PINES: [&str; 2] = ["nature/tree_pineTallA.glb", "nature/tree_pineRoundC.glb"];
const BUSHES: [&str; 3] = ["nature/plant_bush.glb", "nature/plant_bushLarge.glb", "nature/plant_bushDetailed.glb"];
const ROCKS: [&str; 4] = ["nature/rock_largeA.glb", "nature/rock_smallA.glb", "nature/rock_smallC.glb", "nature/stone_largeB.glb"];

fn nature(mut commands: Commands, assets: Res<AssetServer>) {
    let mut rng = Rng::new(44);
    let c = &mut commands;

    // North tree line: two staggered rows of broadleaf, pines behind.
    let mut x = -HALF_X;
    while x < HALF_X {
        let z = -150.0 - rng.range(0.0, 14.0);
        let m = *rng.pick(&BROADLEAF);
        place(c, &assets, m, x, z, rng.range(6.5, 9.0), rng.range(0.0, TAU));
        if rng.f() < 0.7 {
            let m = *rng.pick(&PINES);
            place(c, &assets, m, x + rng.range(-3.0, 3.0), z - rng.range(14.0, 40.0), rng.range(7.0, 10.0), rng.range(0.0, TAU));
        }
        x += rng.range(6.0, 10.0);
    }
    // Copses on the approaches and behind the southern fields.
    for (cx, cz, n) in [(-250.0, -70.0, 9), (245.0, 75.0, 9), (180.0, -140.0, 6), (-150.0, 160.0, 10), (40.0, 170.0, 8), (-300.0, 120.0, 7)] {
        for _ in 0..n {
            let m = *rng.pick(&BROADLEAF);
            place(c, &assets, m, cx + rng.range(-22.0, 22.0), cz + rng.range(-16.0, 16.0), rng.range(6.0, 8.5), rng.range(0.0, TAU));
        }
    }
    // Village trees: a few in yards and on the square.
    for (x, z) in [(-14.0, 10.0), (14.0, -12.0), (-70.0, -24.0), (70.0, 26.0), (-20.0, -30.0), (30.0, 32.0)] {
        place(c, &assets, "nature/tree_oak.glb", x, z, rng.range(5.5, 7.0), rng.range(0.0, TAU));
    }

    // Hedges along the road-facing edge of each field, and crops inside.
    for (x0, z0, x1, z1, g) in FIELDS {
        let edge = if z0 < 0.0 { z1 + 2.0 } else { z0 - 2.0 };
        let mut x = x0;
        while x <= x1 {
            let m = *rng.pick(&BUSHES);
            place(c, &assets, m, x, edge + rng.range(-0.8, 0.8), rng.range(3.0, 4.2), rng.range(0.0, TAU));
            x += rng.range(3.5, 5.5);
        }
        let (crop, step, scale) = match g {
            Ground::Wheat => ("nature/crops_wheatStageB.glb", 6.0, 3.2),
            Ground::Corn => ("nature/crops_cornStageB.glb", 6.5, 3.4),
            Ground::Plowed => ("nature/crops_dirtRow.glb", 8.0, 4.0),
            Ground::Meadow => continue,
        };
        let mut z = z0 + 4.0;
        while z < z1 - 2.0 {
            let mut x = x0 + 4.0;
            while x < x1 - 2.0 {
                place(c, &assets, crop, x, z, scale, 0.0);
                x += step;
            }
            z += step;
        }
    }

    // Rocks and stumps scattered across the approaches.
    for _ in 0..34 {
        let side = if rng.f() < 0.5 { -1.0 } else { 1.0 };
        let x = side * rng.range(130.0, 340.0);
        let z = rng.range(-130.0, 140.0);
        if z.abs() < 30.0 && x.abs() > 230.0 {
            continue; // keep spawn lanes open
        }
        if ground_at(x, z) != Ground::Meadow {
            continue;
        }
        let m = *rng.pick(&ROCKS);
        place(c, &assets, m, x, z, rng.range(3.0, 6.0), rng.range(0.0, TAU));
    }
    for (x, z) in [(-120.0, -140.0), (-60.0, -138.0), (90.0, -142.0)] {
        place(c, &assets, "nature/log_stack.glb", x, z, 4.0, rng.range(0.0, TAU));
        place(c, &assets, "nature/stump_round.glb", x + 6.0, z + 3.0, 4.0, 0.0);
    }
    // Garden fences behind the houses.
    for (x, z, rot) in [(-46.0, -28.0, 0.0), (38.0, -29.0, 0.0), (-40.0, 30.0, 0.0), (42.0, 31.0, 0.0)] {
        for i in -3..=3 {
            let f = if i == 0 { "nature/fence_gate.glb" } else { "nature/fence_planks.glb" };
            place(c, &assets, f, x + i as f32 * 2.2, z, 2.2, rot);
        }
    }
}

#[derive(Resource)]
struct KitStyle {
    road: Vec<Handle<Gltf>>,
    nature: Vec<Handle<Gltf>>,
    done: Vec<bool>,
}

/// Road tiles run along the local z axis; turn them to run west to east.
fn road(mut commands: Commands, assets: Res<AssetServer>) {
    let tile = 8.0;
    let n = (HALF_X / tile) as i32;
    for i in -n..=n {
        let x = i as f32 * tile;
        let path = if i == 0 { "roads/road-crossroad-path.glb" } else { "roads/road-straight.glb" };
        commands.spawn((
            Name::new("Road"),
            kit_scene(&assets, path),
            Transform::from_xyz(x, 0.02, 0.0).with_rotation(Quat::from_rotation_y(FRAC_PI_2)).with_scale(Vec3::new(tile, 1.2, tile)),
        ));
    }
    let road = ["roads/road-straight.glb", "roads/road-crossroad-path.glb"].iter().map(|p| assets.load(*p)).collect::<Vec<_>>();
    let nature = [
        BROADLEAF.as_slice(),
        PINES.as_slice(),
        BUSHES.as_slice(),
        ROCKS.as_slice(),
        &["nature/crops_wheatStageB.glb", "nature/crops_cornStageB.glb", "nature/crops_dirtRow.glb", "nature/log_stack.glb", "nature/stump_round.glb", "nature/fence_planks.glb", "nature/fence_gate.glb"],
    ]
    .concat()
    .iter()
    .map(|p| assets.load(*p))
    .collect::<Vec<_>>();
    let done = vec![false; road.len() + nature.len()];
    commands.insert_resource(KitStyle { road, nature, done });
}

/// Art direction pass on the kit materials once they load: the road becomes
/// a packed-earth country road with no lane markings, and the bright kit
/// greens are pulled toward muted earth tones.
fn restyle_kits(mut style: ResMut<KitStyle>, gltfs: Res<Assets<Gltf>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let n_road = style.road.len();
    for i in 0..style.done.len() {
        if style.done[i] {
            continue;
        }
        let h = if i < n_road { style.road[i].clone() } else { style.nature[i - n_road].clone() };
        let Some(g) = gltfs.get(&h) else { continue };
        for mh in g.materials.iter() {
            let Some(m) = materials.get_mut(mh) else { continue };
            if i < n_road {
                m.base_color_texture = None;
                m.base_color = Color::srgb(0.50, 0.42, 0.32);
                m.perceptual_roughness = 1.0;
            } else {
                let c = m.base_color.to_srgba();
                let l = 0.3 * c.red + 0.59 * c.green + 0.11 * c.blue;
                let mix = |v: f32, w: f32| (v * 0.6 + l * 0.4) * w;
                m.base_color = Color::srgb(mix(c.red, 0.92), mix(c.green, 0.86), mix(c.blue, 0.78));
                m.perceptual_roughness = 0.95;
            }
        }
        style.done[i] = true;
    }
}
