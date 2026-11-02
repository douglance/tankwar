//! The arena: Lesmont, a fictional rural European village.
//!
//! Layout comes from the shared footprints in docs/CONTRACT.md (see
//! `lesmont`). Viewer axes: x east, z south, y up; viewer z = -sim y.
//!
//! - homes h1..h12 and the church c1 fill their footprints; walls w1..w3 are
//!   permanent stone walls
//! - one west-east road, farm lanes between the clusters, a cobbled square
//! - wheat, plowed and corn fields edged with hedgerows, two orchards
//! - a tree line around the playable ground, wooded hills beyond it, a sky
//!   dome whose horizon matches the fog so the ground runs to the edges
//!
//! Homes and the church have intact, damaged and rubble states. Which one is
//! shown comes only from the snapshot's structure list; the viewer never
//! decides that a structure was hit.
//!
//! Buildings, hedges, haystacks and the distant forest are flat-shaded
//! procedural meshes. Trees, bushes, crops, rocks, fences, logs and the
//! square's paving come from the Kenney CC0 kits. Ground texture is Poly
//! Haven's CC0 brown mud with leaves.

use crate::lesmont::{self, Crop, Footprint, Kind, FOOTPRINTS, PLAY_X, PLAY_Y};
use crate::{Feeds, Screen, Viewing};
use bevy::asset::RenderAssetUsages;
use bevy::gltf::Gltf;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::math::primitives::{Cone, Cuboid, Extrusion, Triangle2d};
use bevy::pbr::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::view::NoFrustumCulling;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Horizon haze. The fog and the bottom of the sky dome use the same color,
/// so distant ground fades into the sky without a visible edge.
pub const SKY: Color = Color::srgb(0.74, 0.77, 0.78);
const ZENITH: [f32; 3] = [0.36, 0.52, 0.72];

/// Fine terrain covers the playable ground plus a margin; the coarse skirt
/// carries the hills out past the fog.
const FINE_X: f32 = 312.0;
const FINE_Z: f32 = 240.0;
const FINE_CELL: f32 = 4.0;
const OUTER: f32 = 1416.0;
const OUTER_CELL: f32 = 24.0;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (light, sky, terrain, roads, village, nature, distant_forest))
            .add_systems(Update, (restyle_kits, follow_sky, show_structures));
    }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Distance outside the playable rectangle (0 inside).
fn outside_play(x: f32, z: f32) -> f32 {
    let dx = (x.abs() - PLAY_X).max(0.0);
    let dz = (z.abs() - PLAY_Y).max(0.0);
    (dx * dx + dz * dz).sqrt()
}

fn footprint_distance(p: Vec2) -> f32 {
    FOOTPRINTS.iter().map(|f| lesmont::rect_distance(p, f.viewer_rect())).fold(f32::MAX, f32::min)
}

/// Ground height in meters. Gently rolling on the playable ground, flat on
/// the roads and under the buildings, rising into wooded hills outside.
pub fn height(x: f32, z: f32) -> f32 {
    let p = Vec2::new(x, z);
    let d = outside_play(x, z);
    let roll = 1.6 * (x * 0.011 + 0.7).sin() * (z * 0.013).cos()
        + 1.0 * (x * 0.027 + z * 0.019).sin()
        + 0.4 * (x * 0.061 - z * 0.047).sin();
    let hills = smoothstep(0.0, 520.0, d)
        * (46.0 + 22.0 * (x * 0.0041 + 1.3).sin() * (z * 0.0053 - 0.4).cos() + 9.0 * (x * 0.013 - z * 0.011).sin());
    let road = smoothstep(10.0, 26.0 + d * 0.18, lesmont::road_distance(p));
    let yards = if d > 0.0 { 1.0 } else { smoothstep(6.0, 20.0, footprint_distance(p)) };
    (roll + hills) * road.min(yards)
}

pub fn normal(x: f32, z: f32) -> Vec3 {
    let e = 1.5;
    let dx = height(x + e, z) - height(x - e, z);
    let dz = height(x, z + e) - height(x, z - e);
    Vec3::new(-dx, 2.0 * e, -dz).normalize()
}

fn crop_at(p: Vec2) -> Option<Crop> {
    lesmont::FIELDS.iter().find_map(|&(c, r)| {
        let (a, b) = lesmont::field_rect(r);
        (p.x >= a.x && p.x <= b.x && p.y >= a.y && p.y <= b.y).then_some(c)
    })
}

fn hash2(a: i32, b: i32) -> u32 {
    let mut x = (a as u32).wrapping_mul(0x9E37_79B9) ^ (b as u32).wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE35);
    x ^ (x >> 16)
}

/// Vertex tint, multiplied with the mud texture. Values above 1 brighten it.
fn tint(x: f32, z: f32, y: f32) -> [f32; 4] {
    let p = Vec2::new(x, z);
    let n = (hash2((x * 0.5) as i32, (z * 0.5) as i32) % 1000) as f32 / 1000.0 * 0.08 - 0.04;
    let d = outside_play(x, z);
    let mut c = match crop_at(p) {
        Some(Crop::Wheat) => {
            let s = 0.93 + 0.07 * (x * 1.4).sin();
            [1.34 * s, 1.14 * s, 0.66 * s]
        }
        Some(Crop::Plowed) => {
            let s = 0.80 + 0.2 * (z * 2.1).sin().abs();
            [0.92 * s, 0.74 * s, 0.58 * s]
        }
        Some(Crop::Corn) => {
            let s = 0.92 + 0.08 * (z * 1.2).sin();
            [0.90 * s, 1.02 * s, 0.56 * s]
        }
        Some(Crop::Orchard) => [0.86, 0.96, 0.60],
        None => [0.76, 0.88, 0.54],
    };
    if d == 0.0 && footprint_distance(p) < 9.0 {
        c = [0.98, 0.92, 0.74];
    }
    if d > 0.0 {
        // Pasture on the hills, browner where the ground climbs.
        let k = (y / 70.0).clamp(0.0, 1.0);
        let m = 0.5 + 0.5 * (x * 0.009 + z * 0.012).sin();
        c = [0.70 + 0.18 * k + 0.06 * m, 0.84 - 0.06 * k + 0.05 * m, 0.50 - 0.04 * k];
    }
    [c[0] + n, c[1] + n, c[2] + n * 0.5, 1.0]
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
    commands.insert_resource(AmbientLight { color: Color::srgb(0.80, 0.85, 0.95), brightness: 520.0, ..default() });
    // Late-afternoon sun from the south-west: long readable shadows.
    commands.spawn((
        DirectionalLight { illuminance: 12000.0, color: Color::srgb(1.0, 0.93, 0.80), shadows_enabled: true, ..default() },
        Transform::from_xyz(-150.0, 120.0, 170.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder { maximum_distance: 480.0, first_cascade_far_bound: 50.0, ..default() }.build(),
    ));
}

#[derive(Component)]
struct SkyDome;

/// An inverted dome with a vertical gradient, unlit and outside the fog. It
/// follows the camera so it always sits at the horizon.
fn sky(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut mesh = Sphere::new(1500.0).mesh().uv(40, 20);
    let horizon = SKY.to_srgba();
    let cols: Vec<[f32; 4]> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(ps)) => ps
            .iter()
            .map(|p| {
                let t = (p[1] / 1500.0).clamp(0.0, 1.0).powf(0.55);
                let l = |h: f32, z: f32| h + (z - h) * t;
                [l(horizon.red, ZENITH[0]), l(horizon.green, ZENITH[1]), l(horizon.blue, ZENITH[2]), 1.0]
            })
            .collect(),
        _ => vec![],
    };
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, cols);
    let mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        fog_enabled: false,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    commands.spawn((Name::new("Sky"), SkyDome, Mesh3d(meshes.add(mesh)), MeshMaterial3d(mat), NotShadowCaster, NoFrustumCulling, Transform::default()));

    // A few flat low-poly clouds high over the hills.
    let cloud = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
    let cmat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.97, 0.97, 0.96, 0.9),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        fog_enabled: false,
        ..default()
    });
    let mut rng = Rng::new(77);
    for _ in 0..14 {
        let a = rng.range(0.0, TAU);
        let r = rng.range(500.0, 1150.0);
        let c = Vec3::new(a.cos() * r, rng.range(330.0, 520.0), a.sin() * r);
        for j in 0..4 {
            let off = Vec3::new(rng.range(-60.0, 60.0), rng.range(-6.0, 10.0), rng.range(-25.0, 25.0)) * (j as f32 / 3.0);
            let s = Vec3::new(rng.range(50.0, 90.0), rng.range(12.0, 20.0), rng.range(28.0, 45.0));
            commands.spawn((Mesh3d(cloud.clone()), MeshMaterial3d(cmat.clone()), NotShadowCaster, Transform::from_translation(c + off).with_scale(s)));
        }
    }
}

fn follow_sky(cams: Query<&GlobalTransform, With<Camera3d>>, mut dome: Query<&mut Transform, With<SkyDome>>) {
    let Some(c) = cams.iter().next() else { return };
    for mut t in &mut dome {
        t.translation = c.translation();
    }
}

/// Flat-shaded triangle soup builder with per-face colors.
#[derive(Default)]
struct Soup {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
}

impl Soup {
    fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, col: [f32; 4]) {
        let n = (b - a).cross(c - a).normalize_or_zero();
        for v in [a, b, c] {
            self.pos.push(v.to_array());
            self.nor.push(n.to_array());
            self.uv.push([v.x / 9.0, v.z / 9.0]);
            self.col.push(col);
        }
    }
    fn mesh(self) -> Mesh {
        let count = self.pos.len() as u32;
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nor)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
            .with_inserted_indices(Indices::U32((0..count).collect()))
    }
}

fn grid(soup: &mut Soup, x0: f32, z0: f32, nx: usize, nz: usize, cell: f32, h: impl Fn(f32, f32) -> f32) {
    let mut hs = vec![0.0f32; (nx + 1) * (nz + 1)];
    for iz in 0..=nz {
        for ix in 0..=nx {
            hs[iz * (nx + 1) + ix] = h(x0 + ix as f32 * cell, z0 + iz as f32 * cell);
        }
    }
    let v = |ix: usize, iz: usize| {
        let (x, z) = (x0 + ix as f32 * cell, z0 + iz as f32 * cell);
        Vec3::new(x, hs[iz * (nx + 1) + ix], z)
    };
    for iz in 0..nz {
        for ix in 0..nx {
            let (a, b, c, d) = (v(ix, iz), v(ix + 1, iz), v(ix + 1, iz + 1), v(ix, iz + 1));
            for tri in [[a, d, c], [a, c, b]] {
                let m = (tri[0] + tri[1] + tri[2]) / 3.0;
                soup.tri(tri[0], tri[1], tri[2], tint(m.x, m.z, m.y));
            }
        }
    }
}

fn mud_texture(assets: &AssetServer) -> Handle<Image> {
    assets.load_with_settings("ground/brown_mud_leaves_01_diff_1k.jpg", |s: &mut ImageLoaderSettings| {
        s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::linear()
        });
    })
}

fn terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    let mut fine = Soup::default();
    grid(&mut fine, -FINE_X, -FINE_Z, (2.0 * FINE_X / FINE_CELL) as usize, (2.0 * FINE_Z / FINE_CELL) as usize, FINE_CELL, height);
    // The coarse skirt runs under the fine patch, sunk there, so the seam
    // between the two never opens a gap to the sky.
    let mut outer = Soup::default();
    let n = (2.0 * OUTER / OUTER_CELL) as usize;
    grid(&mut outer, -OUTER, -OUTER, n, n, OUTER_CELL, |x, z| {
        let inside = x.abs() < FINE_X - 0.5 && z.abs() < FINE_Z - 0.5;
        height(x, z) - if inside { 6.0 } else { 0.0 }
    });
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.80, 0.76, 0.68),
        base_color_texture: Some(mud_texture(&assets)),
        perceptual_roughness: 0.97,
        reflectance: 0.15,
        ..default()
    });
    commands.spawn((Name::new("Terrain"), Mesh3d(meshes.add(fine.mesh())), MeshMaterial3d(mat.clone())));
    commands.spawn((Name::new("Hills"), Mesh3d(meshes.add(outer.mesh())), MeshMaterial3d(mat), NotShadowCaster));
}

/// Earth ribbons that follow the ground: the main road is packed gravel,
/// lanes are two wheel ruts with grass between.
fn roads(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    let mut pos = Vec::new();
    let mut nor = Vec::new();
    let mut uv = Vec::new();
    let mut col = Vec::new();
    let mut idx = Vec::new();
    for (ri, line) in lesmont::roads_viewer().iter().enumerate() {
        let main = ri == 0;
        let half = if main { 3.6 } else { 2.3 };
        // Across the ribbon: edge, rut, centre, rut, edge.
        let across: [(f32, [f32; 4]); 5] = if main {
            [(-1.0, [0.82, 0.72, 0.58, 1.0]), (-0.5, [0.98, 0.88, 0.72, 1.0]), (0.0, [1.02, 0.92, 0.76, 1.0]), (0.5, [0.98, 0.88, 0.72, 1.0]), (1.0, [0.82, 0.72, 0.58, 1.0])]
        } else {
            [(-1.0, [0.80, 0.80, 0.56, 1.0]), (-0.55, [0.92, 0.78, 0.60, 1.0]), (0.0, [0.78, 0.86, 0.54, 1.0]), (0.55, [0.92, 0.78, 0.60, 1.0]), (1.0, [0.80, 0.80, 0.56, 1.0])]
        };
        let mut pts = Vec::new();
        for w in line.windows(2) {
            let n = ((w[1] - w[0]).length() / 3.0).ceil().max(1.0) as usize;
            for i in 0..n {
                pts.push(w[0].lerp(w[1], i as f32 / n as f32));
            }
        }
        pts.push(*line.last().unwrap());
        for (i, p) in pts.iter().enumerate() {
            let a = pts[i.saturating_sub(1)];
            let b = pts[(i + 1).min(pts.len() - 1)];
            let t = (b - a).normalize_or_zero();
            let side = Vec2::new(-t.y, t.x);
            let base = pos.len() as u32;
            for (o, c) in across {
                let q = *p + side * o * half;
                pos.push([q.x, height(q.x, q.y) + 0.07 + if main { 0.0 } else { 0.01 }, q.y]);
                nor.push(normal(q.x, q.y).to_array());
                uv.push([q.x / 6.0, q.y / 6.0]);
                col.push(c);
            }
            if i + 1 < pts.len() {
                for k in 0..4u32 {
                    let (a, b, c, d) = (base + k, base + k + 1, base + 5 + k, base + 5 + k + 1);
                    idx.extend_from_slice(&[a, c, b, b, c, d]);
                }
            }
        }
    }
    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx));
    commands.spawn((
        Name::new("Roads"),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.66, 0.58, 0.47),
            base_color_texture: Some(mud_texture(&assets)),
            perceptual_roughness: 1.0,
            reflectance: 0.1,
            ..default()
        })),
        NotShadowCaster,
    ));
    // The cobbled square where the lanes meet the road, from the Kenney kit.
    let square = lesmont::to_viewer(0.0, 12.0);
    for (dx, dz) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (0, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
        let (x, z) = (square.x + dx as f32 * 8.0, square.y + dz as f32 * 8.0);
        commands.spawn((
            Name::new("Square"),
            kit_scene(&assets, "roads/road-square.glb"),
            Transform::from_xyz(x, height(x, z) + 0.1, z).with_scale(Vec3::new(8.0, 1.0, 8.0)),
        ));
    }
}

/// Shared meshes and palette for buildings.
struct Kit {
    cube: Handle<Mesh>,
    roof: Handle<Mesh>,
    spire: Handle<Mesh>,
    plaster: Vec<Handle<StandardMaterial>>,
    roofs: Vec<Handle<StandardMaterial>>,
    shutters: Vec<Handle<StandardMaterial>>,
    dark: Handle<StandardMaterial>,
    timber: Handle<StandardMaterial>,
    stone: Handle<StandardMaterial>,
    char: Handle<StandardMaterial>,
}

fn mat(materials: &mut Assets<StandardMaterial>, r: f32, g: f32, b: f32) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial { base_color: Color::srgb(r, g, b), perceptual_roughness: 0.92, ..default() })
}

/// One of a structure's three looks; only the one the snapshot calls for is
/// visible.
#[derive(Component)]
struct StructureLook {
    id: &'static str,
    look: Look,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    Intact,
    Damaged,
    Rubble,
}

/// What a structure should look like given the snapshot's entry for it.
/// Walls are permanent; missing data means intact.
pub fn look_for(f: &Footprint, entry: Option<(u32, bool)>) -> Look {
    if f.kind == Kind::Wall {
        return Look::Intact;
    }
    match entry {
        None => Look::Intact,
        Some((i, destroyed)) if destroyed || i == 0 => Look::Rubble,
        Some((i, _)) if i < f.max_integrity() => Look::Damaged,
        _ => Look::Intact,
    }
}

fn show_structures(
    screen: Res<State<Screen>>,
    feeds: Res<Feeds>,
    v: Res<Viewing>,
    mut looks: Query<(&StructureLook, &mut Visibility)>,
) {
    let entries = if *screen.get() == Screen::Battlefield { feeds.0.get(v.idx).and_then(|m| m.structures_at(v.cursor)) } else { None };
    for (sl, mut vis) in &mut looks {
        let Some(f) = lesmont::footprint(sl.id) else { continue };
        let entry = entries.and_then(|es| es.iter().find(|e| e.0 == sl.id)).map(|e| (e.1, e.2));
        let want = if look_for(f, entry) == sl.look { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

/// A block with plaster walls, a gable roof, windows with shutters, a door
/// and a chimney. `ridge_x` runs the ridge east-west. A damaged block has a
/// torn roof, scorched walls and debris at its foot.
#[allow(clippy::too_many_arguments)]
fn block(p: &mut ChildSpawnerCommands, k: &Kit, rng: &mut Rng, at: Vec3, w: f32, d: f32, h: f32, ridge_x: bool, plaster: &Handle<StandardMaterial>, roof: &Handle<StandardMaterial>, damaged: bool) {
    let (len, span) = if ridge_x { (w, d) } else { (d, w) };
    let rot = if ridge_x { Quat::from_rotation_y(FRAC_PI_2) } else { Quat::IDENTITY };
    let rh = span * rng.range(0.42, 0.55);
    p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(plaster.clone()), Transform::from_translation(at + Vec3::Y * (h / 2.0 - 0.4)).with_scale(Vec3::new(w, h + 0.8, d))));
    // Roof: local z runs along the ridge.
    let (roof_len, roof_off) = if damaged { (len * 0.5, len * 0.25 + 0.4) } else { (len + 0.9, 0.0) };
    p.spawn((
        Mesh3d(k.roof.clone()),
        MeshMaterial3d(roof.clone()),
        Transform::from_translation(at + Vec3::Y * h + rot * Vec3::Z * roof_off).with_rotation(rot).with_scale(Vec3::new(span + 0.9, rh, roof_len)),
    ));
    if damaged {
        // Torn half open to the sky, scorch on the walls, debris below.
        // The torn half is open to the sky: a charred attic floor sits just
        // inside the wall tops, so nothing stands above or outside the walls.
        p.spawn((
            Mesh3d(k.cube.clone()),
            MeshMaterial3d(k.char.clone()),
            Transform::from_translation(at + Vec3::Y * (h - 0.25) + rot * Vec3::Z * (-len * 0.25))
                .with_rotation(rot)
                .with_scale(Vec3::new(span - 0.3, 0.2, len * 0.5 - 0.3)),
        ));
        for s in [-1.0, 1.0] {
            p.spawn((
                Mesh3d(k.cube.clone()),
                MeshMaterial3d(k.char.clone()),
                Transform::from_translation(at + Vec3::Y * (h * 0.7) + rot * Vec3::new(s * (span / 2.0 + 0.06), 0.0, -len * 0.25)).with_rotation(rot).with_scale(Vec3::new(0.1, h * 0.55, len * 0.35)),
            ));
        }
        for _ in 0..6 {
            let o = rot * Vec3::new(rng.range(-1.0, 1.0) * (span / 2.0 + 1.5), 0.3, rng.range(-0.5, 0.0) * len);
            let m = if rng.f() < 0.5 { plaster.clone() } else { k.stone.clone() };
            p.spawn((
                Mesh3d(k.cube.clone()),
                MeshMaterial3d(m),
                Transform::from_translation(at + o).with_rotation(Quat::from_euler(EulerRot::YXZ, rng.range(0.0, TAU), rng.range(-0.3, 0.3), rng.range(-0.3, 0.3))).with_scale(Vec3::new(rng.range(0.8, 2.2), rng.range(0.4, 1.0), rng.range(0.8, 2.0))),
            ));
        }
    } else {
        p.spawn((
            Mesh3d(k.cube.clone()),
            MeshMaterial3d(k.stone.clone()),
            Transform::from_translation(at + Vec3::Y * (h + rh * 0.7) + rot * Vec3::new(span * 0.22, 0.0, len * 0.3)).with_scale(Vec3::new(0.9, rh * 0.9, 0.9)),
        ));
    }
    // Windows and shutters along both long faces, a door on one.
    let shutter = rng.pick(&k.shutters).clone();
    let n = ((len - 3.0) / 3.4).floor().max(1.0) as i32;
    let face = span / 2.0 + 0.05;
    for side in [-1.0f32, 1.0] {
        for i in 0..n {
            let z = -len / 2.0 + (i as f32 + 0.5) * len / n as f32;
            let door = side > 0.0 && i == n / 2;
            let (wy, wh) = if door { (1.15, 2.3) } else { (h * 0.58, 1.3) };
            p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(k.dark.clone()), Transform::from_translation(at + Vec3::Y * wy + rot * Vec3::new(side * face, 0.0, z)).with_rotation(rot).with_scale(Vec3::new(0.1, wh, if door { 1.3 } else { 0.95 }))));
            if !door && !damaged {
                for s in [-1.0, 1.0] {
                    p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(shutter.clone()), Transform::from_translation(at + Vec3::Y * wy + rot * Vec3::new(side * (face + 0.04), 0.0, z + s * 0.75)).with_rotation(rot).with_scale(Vec3::new(0.08, 1.35, 0.5))));
                }
            }
        }
    }
    // Timber lintel on the gable end.
    p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(k.timber.clone()), Transform::from_translation(at + Vec3::Y * (h - 0.2) + rot * Vec3::new(0.0, 0.0, len / 2.0 + 0.05)).with_rotation(rot).with_scale(Vec3::new(span, 0.25, 0.1))));
}

/// A low pile of broken masonry, roof tiles and charred beams that fills the
/// footprint but no longer stands taller than a tank hull.
fn rubble(p: &mut ChildSpawnerCommands, k: &Kit, rng: &mut Rng, size: Vec2, plaster: &Handle<StandardMaterial>, roof: &Handle<StandardMaterial>) {
    let (hx, hz) = (size.x / 2.0 - 1.0, size.y / 2.0 - 1.0);
    for i in 0..26 {
        let m = match i % 4 {
            0 => plaster.clone(),
            1 => k.stone.clone(),
            2 => roof.clone(),
            _ => k.char.clone(),
        };
        let c = Vec3::new(rng.range(-hx, hx) * 0.9, 0.0, rng.range(-hz, hz) * 0.9);
        let mound = 1.0 - (c.x.abs() / hx).max(c.z.abs() / hz) * 0.6;
        let s = Vec3::new(rng.range(1.5, 4.5), rng.range(0.5, 1.4) * mound, rng.range(1.2, 3.5));
        p.spawn((
            Mesh3d(k.cube.clone()),
            MeshMaterial3d(m),
            Transform::from_translation(c + Vec3::Y * s.y * 0.4).with_rotation(Quat::from_euler(EulerRot::YXZ, rng.range(0.0, TAU), rng.range(-0.35, 0.35), rng.range(-0.35, 0.35))).with_scale(s),
        ));
    }
    // Two broken wall stubs and a few beams poking out.
    for (x, z, w, d) in [(-hx, rng.range(-hz, 0.0), 0.6, size.y * 0.35), (rng.range(0.0, hx), hz, size.x * 0.3, 0.6)] {
        p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(plaster.clone()), Transform::from_xyz(x, 1.1, z).with_scale(Vec3::new(w, rng.range(1.6, 2.4), d))));
    }
    for _ in 0..4 {
        p.spawn((
            Mesh3d(k.cube.clone()),
            MeshMaterial3d(k.char.clone()),
            Transform::from_xyz(rng.range(-hx, hx) * 0.6, 1.2, rng.range(-hz, hz) * 0.6)
                .with_rotation(Quat::from_euler(EulerRot::YXZ, rng.range(0.0, TAU), 0.0, rng.range(0.3, 0.7)))
                .with_scale(Vec3::new(rng.range(4.0, 6.5), 0.3, 0.3)),
        ));
    }
}

/// Home layouts, chosen per footprint so no two neighbours repeat.
#[derive(Clone, Copy)]
enum Layout {
    Farmhouse,
    LShape,
    HouseAndBarn,
}

fn home(commands: &mut Commands, k: &Kit, f: &Footprint, seed: u64) {
    let c = f.center();
    let size = f.size();
    let y = height(c.x, c.y);
    let mut rng = Rng::new(seed);
    let plaster = rng.pick(&k.plaster).clone();
    let roof = rng.pick(&k.roofs).clone();
    let layout = [Layout::Farmhouse, Layout::LShape, Layout::HouseAndBarn][seed as usize % 3];
    let h = rng.range(5.5, 7.5);
    let ridge_x = rng.f() < 0.5;
    let (iw, id) = (size.x - 2.0, size.y - 2.0);
    let parts: Vec<(Vec3, f32, f32, f32, bool, bool)> = match layout {
        Layout::Farmhouse => vec![(Vec3::ZERO, iw, id * 0.7, h, true, false)],
        Layout::LShape => vec![
            (Vec3::new(0.0, 0.0, -id * 0.25), iw, id * 0.5, h, true, false),
            (Vec3::new(iw * 0.25, 0.0, id * 0.25), iw * 0.5, id * 0.5, h - 0.8, false, false),
        ],
        Layout::HouseAndBarn => vec![
            (Vec3::new(-iw * 0.22, 0.0, 0.0), iw * 0.56, id * 0.8, h, ridge_x, false),
            (Vec3::new(iw * 0.28, 0.0, 0.0), iw * 0.44, id, h + 1.2, false, true),
        ],
    };
    let barn = k.timber.clone();
    let root = commands.spawn((Name::new(format!("Home {}", f.id)), Transform::from_xyz(c.x, y, c.y), Visibility::default())).id();
    for look in [Look::Intact, Look::Damaged, Look::Rubble] {
        let mut r = Rng::new(seed * 31 + look as u64);
        let child = commands
            .spawn((StructureLook { id: f.id, look }, Transform::default(), if look == Look::Intact { Visibility::Inherited } else { Visibility::Hidden }))
            .with_children(|p| match look {
                Look::Rubble => rubble(p, k, &mut r, size, &plaster, &roof),
                _ => {
                    for (i, &(at, w, d, hh, rx, is_barn)) in parts.iter().enumerate() {
                        let pl = if is_barn { &barn } else { &plaster };
                        // Only the main block shows the hit; the annex is left standing.
                        block(p, k, &mut r, at, w, d, hh, rx, pl, &roof, look == Look::Damaged && i == 0);
                    }
                }
            })
            .id();
        commands.entity(root).add_child(child);
    }
}

fn church(commands: &mut Commands, k: &Kit, f: &Footprint) {
    let c = f.center();
    let size = f.size();
    let y = height(c.x, c.y);
    let roof = k.roofs[2].clone();
    let root = commands.spawn((Name::new("Church c1"), Transform::from_xyz(c.x, y, c.y), Visibility::default())).id();
    // The tower faces the village (north, toward -z), the nave runs south.
    let tower_z = -size.y / 2.0 + 4.5;
    let nave_z = 3.5;
    let nave_len = size.y - 9.0;
    for look in [Look::Intact, Look::Damaged, Look::Rubble] {
        let mut r = Rng::new(900 + look as u64);
        let child = commands
            .spawn((StructureLook { id: f.id, look }, Transform::default(), if look == Look::Intact { Visibility::Inherited } else { Visibility::Hidden }))
            .with_children(|p| {
                if look == Look::Rubble {
                    rubble(p, k, &mut r, size, &k.stone, &roof);
                    p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(k.stone.clone()), Transform::from_xyz(0.0, 3.5, tower_z).with_scale(Vec3::new(7.0, 7.0, 7.0))));
                    return;
                }
                let nave_w = size.x * 0.5;
                p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(k.stone.clone()), Transform::from_xyz(0.0, 4.6, nave_z).with_scale(Vec3::new(nave_w, 10.0, nave_len))));
                let rl = if look == Look::Damaged { nave_len * 0.55 } else { nave_len + 1.0 };
                let ro = if look == Look::Damaged { nave_len * 0.22 } else { 0.0 };
                p.spawn((Mesh3d(k.roof.clone()), MeshMaterial3d(roof.clone()), Transform::from_xyz(0.0, 9.6, nave_z + ro).with_scale(Vec3::new(nave_w + 1.0, 6.0, rl))));
                // Side aisles with lean-to roofs widen the silhouette.
                for s in [-1.0, 1.0] {
                    p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(k.stone.clone()), Transform::from_xyz(s * (nave_w / 2.0 + 3.0), 2.6, nave_z + 2.0).with_scale(Vec3::new(6.0, 6.0, nave_len * 0.7))));
                    p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(roof.clone()), Transform::from_xyz(s * (nave_w / 2.0 + 3.0), 5.8, nave_z + 2.0).with_rotation(Quat::from_rotation_z(s * -0.35)).with_scale(Vec3::new(6.8, 0.4, nave_len * 0.7 + 0.6))));
                    for i in 0..4 {
                        let z = nave_z - nave_len * 0.3 + i as f32 * nave_len * 0.2;
                        p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(k.dark.clone()), Transform::from_xyz(s * (nave_w / 2.0 + 0.05), 7.0, z).with_scale(Vec3::new(0.1, 3.2, 1.2))));
                    }
                }
                let th = if look == Look::Damaged { 16.0 } else { 22.0 };
                p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(k.stone.clone()), Transform::from_xyz(0.0, th / 2.0, tower_z).with_scale(Vec3::new(7.0, th, 7.0))));
                p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(k.dark.clone()), Transform::from_xyz(0.0, 1.6, tower_z - 3.55).with_scale(Vec3::new(2.0, 3.2, 0.1))));
                if look == Look::Damaged {
                    for _ in 0..8 {
                        p.spawn((
                            Mesh3d(k.cube.clone()),
                            MeshMaterial3d(if r.f() < 0.5 { k.stone.clone() } else { k.char.clone() }),
                            Transform::from_xyz(r.range(-10.0, 10.0), 0.4, r.range(4.0, size.y / 2.0 - 1.0))
                                .with_rotation(Quat::from_euler(EulerRot::YXZ, r.range(0.0, TAU), r.range(-0.3, 0.3), 0.0))
                                .with_scale(Vec3::new(r.range(1.0, 2.8), r.range(0.5, 1.1), r.range(1.0, 2.5))),
                        ));
                    }
                } else {
                    p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(k.dark.clone()), Transform::from_xyz(0.0, 17.5, tower_z - 3.55).with_scale(Vec3::new(1.2, 1.8, 0.1))));
                    p.spawn((Mesh3d(k.spire.clone()), MeshMaterial3d(roof.clone()), Transform::from_xyz(0.0, 22.0 + 5.0, tower_z).with_rotation(Quat::from_rotation_y(PI / 4.0)).with_scale(Vec3::new(6.9, 10.0, 6.9))));
                }
            })
            .id();
        commands.entity(root).add_child(child);
    }
}

fn stone_wall(commands: &mut Commands, k: &Kit, f: &Footprint) {
    let c = f.center();
    let s = f.size();
    let y = height(c.x, c.y);
    commands
        .spawn((Name::new(format!("Wall {}", f.id)), Transform::from_xyz(c.x, y, c.y), Visibility::default()))
        .with_children(|p| {
            p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(k.stone.clone()), Transform::from_xyz(0.0, 0.6, 0.0).with_scale(Vec3::new(s.x, 2.0, s.y))));
            p.spawn((Mesh3d(k.cube.clone()), MeshMaterial3d(k.dark.clone()), Transform::from_xyz(0.0, 1.65, 0.0).with_scale(Vec3::new(s.x + 0.3, 0.12, s.y + 0.3))));
        });
}

fn village(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let roof_mesh = Extrusion::new(Triangle2d::new(Vec2::new(-0.5, 0.0), Vec2::new(0.5, 0.0), Vec2::new(0.0, 1.0)), 1.0);
    let k = Kit {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        roof: meshes.add(roof_mesh),
        spire: meshes.add(Cone::new(0.72, 1.0).mesh().resolution(4)),
        plaster: vec![
            mat(&mut materials, 0.86, 0.80, 0.68),
            mat(&mut materials, 0.80, 0.69, 0.50),
            mat(&mut materials, 0.82, 0.79, 0.73),
            mat(&mut materials, 0.73, 0.66, 0.57),
            mat(&mut materials, 0.66, 0.60, 0.52),
        ],
        roofs: vec![mat(&mut materials, 0.55, 0.27, 0.18), mat(&mut materials, 0.45, 0.24, 0.18), mat(&mut materials, 0.30, 0.30, 0.33), mat(&mut materials, 0.52, 0.35, 0.22)],
        shutters: vec![mat(&mut materials, 0.36, 0.44, 0.36), mat(&mut materials, 0.48, 0.22, 0.18), mat(&mut materials, 0.36, 0.40, 0.46)],
        dark: mat(&mut materials, 0.15, 0.13, 0.11),
        timber: mat(&mut materials, 0.42, 0.30, 0.20),
        stone: mat(&mut materials, 0.60, 0.58, 0.53),
        char: mat(&mut materials, 0.10, 0.09, 0.08),
    };
    for (i, f) in FOOTPRINTS.iter().enumerate() {
        match f.kind {
            Kind::Home => home(&mut commands, &k, f, 11 + i as u64),
            Kind::Church => church(&mut commands, &k, f),
            Kind::Wall => stone_wall(&mut commands, &k, f),
        }
    }
    // A well on the square.
    let sq = lesmont::to_viewer(-10.0, 22.0);
    commands.spawn((Name::new("Well"), Mesh3d(k.cube.clone()), MeshMaterial3d(k.stone.clone()), Transform::from_xyz(sq.x, height(sq.x, sq.y) + 0.5, sq.y).with_scale(Vec3::new(2.4, 1.1, 2.4))));
}

fn kit_scene(assets: &AssetServer, path: &str) -> SceneRoot {
    SceneRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path.to_string())))
}

fn place(commands: &mut Commands, assets: &AssetServer, path: &str, x: f32, z: f32, scale: Vec3, rot: f32) {
    commands.spawn((
        kit_scene(assets, path),
        Transform::from_xyz(x, height(x, z) - 0.05, z).with_rotation(Quat::from_rotation_y(rot)).with_scale(scale),
    ));
}

const BROADLEAF: [&str; 4] = ["nature/tree_oak.glb", "nature/tree_default.glb", "nature/tree_detailed.glb", "nature/tree_fat.glb"];
const PINES: [&str; 2] = ["nature/tree_pineTallA.glb", "nature/tree_pineRoundC.glb"];
const BUSHES: [&str; 3] = ["nature/plant_bush.glb", "nature/plant_bushLarge.glb", "nature/plant_bushDetailed.glb"];
const ROCKS: [&str; 4] = ["nature/rock_largeA.glb", "nature/rock_smallA.glb", "nature/rock_smallC.glb", "nature/stone_largeB.glb"];

/// Distance to the paths the demo tanks drive, so props never stand in them.
fn route_distance(p: Vec2) -> f32 {
    let starts = [lesmont::spawn_west(), lesmont::spawn_east()];
    let mut d = f32::MAX;
    for (i, r) in crate::feed::ROUTES.iter().enumerate() {
        let mut prev = starts[i % 2] + Vec2::new(0.0, if i % 2 == 0 { -12.0 } else { 12.0 } * (i / 2) as f32);
        for &(_, (x, z)) in r.iter() {
            let q = Vec2::new(x, z);
            let ab = q - prev;
            let t = ((p - prev).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            d = d.min((prev + ab * t - p).length());
            prev = q;
        }
    }
    for s in starts {
        d = d.min((p - s).length() - 20.0);
    }
    d
}

/// Open ground for a prop: off roads, routes, footprints and (optionally) fields.
fn open(p: Vec2, allow_field: bool) -> bool {
    lesmont::road_distance(p) > 7.0 && route_distance(p) > 9.0 && footprint_distance(p) > 3.0 && (allow_field || crop_at(p).is_none())
}

fn hedge(soup: &mut Soup, rng: &mut Rng, a: Vec2, b: Vec2) {
    let len = a.distance(b);
    let n = (len / 3.0).ceil() as usize;
    let dir = (b - a) / len;
    let side = Vec2::new(-dir.y, dir.x);
    for i in 0..n {
        let t = (i as f32 + 0.5) / n as f32;
        let c = a.lerp(b, t);
        if (t - 0.5).abs() < 0.05 || !open(c, true) || lesmont::road_distance(c) < 8.0 {
            continue; // a gate gap mid-side, and none across a lane
        }
        let hw = rng.range(0.7, 1.0);
        let hh = rng.range(1.2, 1.8);
        let hl = len / n as f32 * 0.62;
        let g = rng.range(0.0, 0.08);
        let col = [0.24 + g, 0.33 + g, 0.17, 1.0];
        let y = height(c.x, c.y) - 0.1;
        let q = |u: f32, v: f32, h: f32| {
            let p = c + dir * u + side * v;
            Vec3::new(p.x, y + h, p.y)
        };
        // A rounded box: four sloped sides and a top.
        let (b0, b1, b2, b3) = (q(-hl, -hw, 0.0), q(hl, -hw, 0.0), q(hl, hw, 0.0), q(-hl, hw, 0.0));
        let (t0, t1, t2, t3) = (q(-hl * 0.8, -hw * 0.6, hh), q(hl * 0.8, -hw * 0.6, hh), q(hl * 0.8, hw * 0.6, hh), q(-hl * 0.8, hw * 0.6, hh));
        for (p0, p1, p2, p3) in [(b0, b1, t1, t0), (b1, b2, t2, t1), (b2, b3, t3, t2), (b3, b0, t0, t3), (t0, t1, t2, t3)] {
            soup.tri(p0, p2, p1, col);
            soup.tri(p0, p3, p2, col);
        }
    }
}

fn haystack(soup: &mut Soup, at: Vec2, r: f32) {
    let y = height(at.x, at.y);
    let n = 7;
    let col = [0.86, 0.72, 0.42, 1.0];
    let top = Vec3::new(at.x, y + r * 1.9, at.y);
    for i in 0..n {
        let a0 = i as f32 / n as f32 * TAU;
        let a1 = (i + 1) as f32 / n as f32 * TAU;
        let p = |a: f32, rr: f32, h: f32| Vec3::new(at.x + a.cos() * rr, y + h, at.y + a.sin() * rr);
        let (l0, l1, m0, m1) = (p(a0, r, 0.0), p(a1, r, 0.0), p(a0, r * 0.95, r), p(a1, r * 0.95, r));
        soup.tri(l0, m0, l1, col);
        soup.tri(l1, m0, m1, col);
        soup.tri(m0, top, m1, [0.80, 0.66, 0.38, 1.0]);
    }
}

fn nature(mut commands: Commands, assets: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut rng = Rng::new(44);
    let c = &mut commands;
    let s = Vec3::splat;

    // Tree line framing the playable ground, two to three rows deep, open
    // where the road leaves the map.
    let ring = |rng: &mut Rng, off: f32| -> Vec<Vec2> {
        let (hx, hz) = (PLAY_X + off, PLAY_Y + off);
        let mut v = Vec::new();
        let mut t = 0.0;
        let per = 4.0 * (hx + hz);
        while t < per {
            let p = if t < 2.0 * hx {
                Vec2::new(-hx + t, -hz)
            } else if t < 2.0 * hx + 2.0 * hz {
                Vec2::new(hx, -hz + (t - 2.0 * hx))
            } else if t < 4.0 * hx + 2.0 * hz {
                Vec2::new(hx - (t - 2.0 * hx - 2.0 * hz), hz)
            } else {
                Vec2::new(-hx, hz - (t - 4.0 * hx - 2.0 * hz))
            };
            v.push(p + Vec2::new(rng.range(-4.0, 4.0), rng.range(-4.0, 4.0)));
            t += rng.range(8.0, 12.0);
        }
        v
    };
    for (off, keep, pine) in [(8.0, 0.92, 0.1), (22.0, 0.8, 0.35), (38.0, 0.7, 0.6)] {
        for p in ring(&mut rng, off) {
            if rng.f() > keep || lesmont::road_distance(p) < 14.0 {
                continue;
            }
            let m = if rng.f() < pine { *rng.pick(&PINES) } else { *rng.pick(&BROADLEAF) };
            place(c, &assets, m, p.x, p.y, s(rng.range(7.0, 10.5)), rng.range(0.0, TAU));
        }
    }

    // Orchards: trees in straight rows.
    for &(crop, r) in lesmont::FIELDS {
        let (a, b) = lesmont::field_rect(r);
        match crop {
            Crop::Orchard => {
                let mut z = a.y + 5.0;
                while z < b.y - 3.0 {
                    let mut x = a.x + 5.0;
                    while x < b.x - 3.0 {
                        let p = Vec2::new(x, z);
                        if open(p, true) {
                            let m = if rng.f() < 0.6 { "nature/tree_fat.glb" } else { "nature/tree_default.glb" };
                            place(c, &assets, m, x, z, s(rng.range(4.0, 5.0)), rng.range(0.0, TAU));
                        }
                        x += 9.0;
                    }
                    z += 9.0;
                }
            }
            Crop::Wheat | Crop::Corn | Crop::Plowed => {
                // Sparse crop rows; the ground tint carries the field colour.
                let (model, step, sc) = match crop {
                    Crop::Wheat => ("nature/crops_wheatStageB.glb", 10.0, 3.2),
                    Crop::Corn => ("nature/crops_cornStageB.glb", 9.0, 3.4),
                    _ => ("nature/crops_dirtRow.glb", 14.0, 4.0),
                };
                let mut z = a.y + 6.0;
                while z < b.y - 4.0 {
                    let mut x = a.x + 6.0;
                    while x < b.x - 4.0 {
                        if open(Vec2::new(x, z), true) && rng.f() < 0.75 {
                            place(c, &assets, model, x, z, s(sc), 0.0);
                        }
                        x += step;
                    }
                    z += step * 1.6;
                }
            }
        }
    }

    // Hedgerows round the fields and haystacks in the wheat, as one mesh.
    let mut soup = Soup::default();
    for &(crop, r) in lesmont::FIELDS {
        let (a, b) = lesmont::field_rect(r);
        let (a, b) = (a - Vec2::splat(1.5), b + Vec2::splat(1.5));
        let corners = [a, Vec2::new(b.x, a.y), b, Vec2::new(a.x, b.y)];
        for i in 0..4 {
            hedge(&mut soup, &mut rng, corners[i], corners[(i + 1) % 4]);
        }
        if crop == Crop::Wheat {
            for _ in 0..5 {
                let p = Vec2::new(rng.range(a.x + 8.0, b.x - 8.0), rng.range(a.y + 8.0, b.y - 8.0));
                if open(p, true) {
                    haystack(&mut soup, p, rng.range(1.6, 2.2));
                }
            }
        }
    }
    c.spawn((
        Name::new("Hedgerows"),
        Mesh3d(meshes.add(soup.mesh())),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, ..default() })),
    ));

    // Yard trees and bushes beside the homes, on the side away from the lanes.
    for f in FOOTPRINTS.iter().filter(|f| f.kind != Kind::Wall) {
        let (a, b) = f.viewer_rect();
        for corner in [a, b, Vec2::new(a.x, b.y), Vec2::new(b.x, a.y)] {
            let out = (corner - f.center()).normalize() * 6.0;
            let p = corner + out;
            if open(p, false) && rng.f() < 0.7 {
                let m = *rng.pick(&BROADLEAF);
                place(c, &assets, m, p.x, p.y, s(rng.range(5.5, 7.5)), rng.range(0.0, TAU));
            } else if open(p, false) {
                let m = *rng.pick(&BUSHES);
                place(c, &assets, m, p.x, p.y, s(rng.range(3.0, 4.0)), rng.range(0.0, TAU));
            }
        }
        // A short garden fence with a gate on one side.
        let z = b.y + 4.0;
        for i in 0..6 {
            let x = a.x + 2.0 + i as f32 * 2.2;
            if x < b.x - 1.0 && open(Vec2::new(x, z), false) {
                let m = if i == 2 { "nature/fence_gate.glb" } else { "nature/fence_planks.glb" };
                place(c, &assets, m, x, z, s(2.2), 0.0);
            }
        }
    }

    // Poplars along the main road outside the village, a European touch.
    for side in [-1.0, 1.0] {
        let mut x = -235.0;
        while x < 235.0 {
            let p = lesmont::to_viewer(x, 11.0) + Vec2::new(0.0, side * 9.0);
            if x.abs() > 110.0 && open(p, false) {
                place(c, &assets, "nature/tree_pineTallA.glb", p.x, p.y, Vec3::new(4.0, rng.range(10.0, 12.0), 4.0), rng.range(0.0, TAU));
            }
            x += 13.0;
        }
    }

    // Rocks, stumps and log stacks on the open meadow.
    let mut placed = 0;
    while placed < 40 {
        let p = Vec2::new(rng.range(-PLAY_X, PLAY_X), rng.range(-PLAY_Y, PLAY_Y));
        if !open(p, false) {
            continue;
        }
        let m = *rng.pick(&ROCKS);
        place(c, &assets, m, p.x, p.y, s(rng.range(2.5, 5.0)), rng.range(0.0, TAU));
        placed += 1;
    }
    for (x, z) in [(-200.0, -186.0), (-60.0, -188.0), (90.0, -190.0), (200.0, 184.0), (-150.0, 186.0)] {
        place(c, &assets, "nature/log_stack.glb", x, z, s(4.0), rng.range(0.0, TAU));
        place(c, &assets, "nature/stump_round.glb", x + 6.0, z + 3.0, s(4.0), 0.0);
    }
    let _ = assets.load::<Gltf>("roads/road-square.glb");
}

/// Wooded hills beyond the tree line: thousands of simple cone and crown
/// trees merged into one mesh so they cost almost nothing to draw.
fn distant_forest(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut soup = Soup::default();
    let mut rng = Rng::new(5);
    let step = 17.0;
    let lim = 760.0;
    let mut z = -lim;
    while z < lim {
        let mut x = -lim;
        while x < lim {
            let p = Vec2::new(x + rng.range(-6.0, 6.0), z + rng.range(-6.0, 6.0));
            x += step;
            let d = outside_play(p.x, p.y);
            if d < 60.0 || lesmont::road_distance(p) < 40.0 + d * 0.1 {
                continue;
            }
            let patch = 0.5 + 0.5 * ((p.x * 0.010).sin() * (p.y * 0.012 + 0.6).cos() + 0.4 * (p.x * 0.031 + p.y * 0.027).sin());
            if patch < 0.42 || rng.f() > 0.85 {
                continue;
            }
            let y = height(p.x, p.y) - 0.5;
            let g = rng.range(-0.04, 0.04);
            let hgt = rng.range(11.0, 18.0);
            let r = rng.range(3.5, 5.5);
            let base = Vec3::new(p.x, y, p.y);
            let pine = rng.f() < 0.45;
            let col = if pine { [0.17 + g, 0.27 + g, 0.17, 1.0] } else { [0.27 + g, 0.36 + g, 0.18, 1.0] };
            let n = 5;
            let a0 = rng.range(0.0, TAU);
            for i in 0..n {
                let a = a0 + i as f32 / n as f32 * TAU;
                let b = a0 + (i + 1) as f32 / n as f32 * TAU;
                let ring = |ang: f32, rr: f32, h: f32| base + Vec3::new(ang.cos() * rr, h, ang.sin() * rr);
                if pine {
                    let top = base + Vec3::Y * hgt;
                    soup.tri(ring(a, r, hgt * 0.15), top, ring(b, r, hgt * 0.15), col);
                } else {
                    // A faceted crown: a bipyramid on a short trunk.
                    let top = base + Vec3::Y * hgt;
                    let bot = base + Vec3::Y * hgt * 0.25;
                    let (m0, m1) = (ring(a, r * 1.2, hgt * 0.6), ring(b, r * 1.2, hgt * 0.6));
                    soup.tri(m0, top, m1, col);
                    soup.tri(m0, m1, bot, [col[0] * 0.8, col[1] * 0.8, col[2] * 0.8, 1.0]);
                }
            }
        }
        z += step;
    }
    commands.spawn((
        Name::new("Distant forest"),
        Mesh3d(meshes.add(soup.mesh())),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, ..default() })),
        NotShadowCaster,
    ));
}

#[derive(Resource)]
struct KitStyle {
    road: Vec<Handle<Gltf>>,
    nature: Vec<Handle<Gltf>>,
    done: Vec<bool>,
}

impl FromWorld for KitStyle {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        let road = vec![assets.load("roads/road-square.glb")];
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
        KitStyle { road, nature, done }
    }
}

/// Art direction pass on the kit materials once they load: the square
/// becomes worn grey-brown cobbles, and the bright kit greens are pulled
/// toward muted earth tones.
fn restyle_kits(style: Option<ResMut<KitStyle>>, mut commands: Commands, gltfs: Res<Assets<Gltf>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let Some(mut style) = style else {
        commands.init_resource::<KitStyle>();
        return;
    };
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
                let c = m.base_color.to_srgba();
                let l = 0.3 * c.red + 0.59 * c.green + 0.11 * c.blue;
                m.base_color = Color::srgb(0.42 + l * 0.3, 0.38 + l * 0.28, 0.33 + l * 0.24);
                m.base_color_texture = None;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_look_follows_snapshot_only() {
        let h = lesmont::footprint("h5").unwrap();
        let c = lesmont::footprint("c1").unwrap();
        let w = lesmont::footprint("w1").unwrap();
        assert_eq!(look_for(h, None), Look::Intact);
        assert_eq!(look_for(h, Some((3, false))), Look::Intact);
        assert_eq!(look_for(h, Some((2, false))), Look::Damaged);
        assert_eq!(look_for(h, Some((1, false))), Look::Damaged);
        assert_eq!(look_for(h, Some((0, false))), Look::Rubble);
        assert_eq!(look_for(h, Some((2, true))), Look::Rubble);
        assert_eq!(look_for(c, Some((3, false))), Look::Damaged);
        assert_eq!(look_for(c, Some((5, false))), Look::Intact);
        assert_eq!(look_for(w, Some((0, true))), Look::Intact);
    }

    #[test]
    fn ground_is_flat_under_footprints_and_roads_and_rises_outside() {
        for f in FOOTPRINTS.iter() {
            let c = f.center();
            assert!(height(c.x, c.y).abs() < 1e-3, "{} not flat", f.id);
        }
        for road in lesmont::roads_viewer() {
            for p in road.iter().filter(|p| p.x.abs() < PLAY_X) {
                assert!(height(p.x, p.y).abs() < 1e-3);
            }
        }
        assert!(height(0.0, -700.0) > 25.0, "no hills beyond the tree line");
    }
}
