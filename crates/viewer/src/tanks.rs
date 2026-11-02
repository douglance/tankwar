//! Tanks on the battlefield. Every pose comes from the feed's snapshots; the
//! viewer only interpolates between them and never invents outcomes.

use crate::feed::{EventKind, Side, TankSnap, TICK_HZ};
use crate::world;
use crate::{Feeds, Playback, Screen, Viewing};
use bevy::prelude::*;

pub struct TanksPlugin;

impl Plugin for TanksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Battlefield), spawn_tanks).add_systems(
            Update,
            (bind_turrets, pose_tanks, effects, smoke).chain().after(Playback).run_if(in_state(Screen::Battlefield)),
        );
    }
}

#[derive(Component)]
pub struct TankView(pub usize);

/// The turret node inside the tank model, found once the model loads.
#[derive(Component)]
pub struct TurretBind {
    pub node: Entity,
    pub base: Quat,
}

#[derive(Component)]
struct Smoke {
    age: f32,
    vel: Vec3,
}

#[derive(Component)]
struct FireLight(usize);

#[derive(Resource)]
struct SmokeAssets {
    mesh: Handle<Mesh>,
    mat: Handle<StandardMaterial>,
    spawn_t: f32,
}

pub fn side_color(s: Side) -> Color {
    match s {
        Side::West => Color::srgb(0.85, 0.72, 0.38),
        Side::East => Color::srgb(0.55, 0.70, 0.86),
    }
}

/// World transform of a tank hull: on the ground, tilted to the slope.
pub fn tank_pose(s: &TankSnap) -> Transform {
    let (x, z) = (s.pos.x, s.pos.y);
    let up = world::normal(x, z);
    let yaw = Quat::from_rotation_y(s.heading);
    let tilt = Quat::from_rotation_arc(Vec3::Y, up);
    Transform::from_xyz(x, world::height(x, z), z).with_rotation(tilt * yaw)
}

/// World rotation of the turret (hull rotation times turret traverse).
pub fn turret_rot(s: &TankSnap) -> Quat {
    tank_pose(s).rotation * Quat::from_rotation_y(s.turret)
}

fn spawn_tanks(
    mut commands: Commands,
    assets: Res<AssetServer>,
    feeds: Res<Feeds>,
    v: Res<Viewing>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let m = &feeds.0[v.idx];
    for (i, seat) in m.seats.iter().enumerate() {
        commands.spawn((
            Name::new(format!("Tank {}", seat.agent)),
            TankView(i),
            SceneRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(seat.vehicle.model()))),
            Transform::default(),
            Visibility::Hidden,
            StateScoped(Screen::Battlefield),
        ));
        commands.spawn((
            FireLight(i),
            PointLight { color: Color::srgb(1.0, 0.55, 0.2), intensity: 0.0, range: 25.0, shadows_enabled: false, ..default() },
            Transform::default(),
            StateScoped(Screen::Battlefield),
        ));
    }
    commands.insert_resource(SmokeAssets {
        mesh: meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
        mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.22, 0.21, 0.20, 0.55),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            ..default()
        }),
        spawn_t: 0.0,
    });
}

fn bind_turrets(
    mut commands: Commands,
    tanks: Query<(Entity, &Children), (With<TankView>, Without<TurretBind>)>,
    kids: Query<&Children>,
    names: Query<(&Name, &Transform)>,
) {
    for (e, _) in &tanks {
        for d in kids.iter_descendants(e) {
            if let Ok((n, t)) = names.get(d) {
                if n.as_str() == "Turret" {
                    commands.entity(e).insert(TurretBind { node: d, base: t.rotation });
                    break;
                }
            }
        }
    }
}

fn pose_tanks(
    feeds: Res<Feeds>,
    v: Res<Viewing>,
    mut tanks: Query<(&TankView, &mut Transform, &mut Visibility, Option<&TurretBind>)>,
    mut nodes: Query<&mut Transform, (Without<TankView>, Without<FireLight>)>,
    mut lights: Query<(&FireLight, &mut PointLight, &mut Transform), (Without<TankView>,)>,
    time: Res<Time>,
) {
    let m = &feeds.0[v.idx];
    for (tv, mut t, mut vis, bind) in &mut tanks {
        let Some(s) = m.tank_at(tv.0, v.cursor) else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Inherited;
        *t = tank_pose(&s);
        if let Some(b) = bind {
            if let Ok(mut nt) = nodes.get_mut(b.node) {
                nt.rotation = b.base * Quat::from_rotation_y(s.turret);
            }
        }
    }
    let flicker = 0.75 + 0.25 * (time.elapsed_secs() * 13.0).sin();
    for (fl, mut pl, mut lt) in &mut lights {
        let s = m.tank_at(fl.0, v.cursor);
        match s {
            Some(s) if s.on_fire => {
                pl.intensity = 250_000.0 * flicker;
                *lt = Transform::from_translation(tank_pose(&s).translation + Vec3::Y * 2.5);
            }
            _ => pl.intensity = 0.0,
        }
    }
}

/// Shots and impacts within the last second and a half, drawn as tracers,
/// muzzle flashes and ground bursts. Side rings mark each tank's allegiance.
fn effects(feeds: Res<Feeds>, v: Res<Viewing>, mut g: Gizmos) {
    let m = &feeds.0[v.idx];
    for (i, seat) in m.seats.iter().enumerate() {
        let Some(s) = m.tank_at(i, v.cursor) else { continue };
        let p = tank_pose(&s).translation + Vec3::Y * 0.15;
        let mut c = side_color(seat.side);
        if s.knocked_out {
            c = Color::srgb(0.35, 0.33, 0.30);
        }
        let ring = Isometry3d::new(p, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
        g.circle(ring, 4.6, c);
        if i == v.tank {
            g.circle(ring, 5.4, Color::srgb(0.95, 0.93, 0.85));
        }
    }
    let window = 1.5 * TICK_HZ;
    let end = m.events_until(v.cursor);
    let start = m.events.partition_point(|e| (e.tick as f32) < v.cursor - window);
    for e in &m.events[start..end] {
        let age = (v.cursor - e.tick as f32) / TICK_HZ;
        match &e.kind {
            EventKind::Shot { from, to, .. } => {
                if age < 0.35 {
                    let k = age / 0.35;
                    let head = from.lerp(*to, k.min(1.0));
                    let tail = from.lerp(*to, (k - 0.25).max(0.0));
                    g.line(tail, head, Color::srgb(1.0, 0.85, 0.45));
                }
                if age < 0.15 {
                    g.sphere(Isometry3d::from_translation(*from), 0.9, Color::srgb(1.0, 0.8, 0.4));
                }
            }
            EventKind::Impact { at, .. } => {
                let r = 0.5 + age * 6.0;
                let a = (1.0 - age / 1.5).max(0.0);
                g.sphere(Isometry3d::from_translation(*at), r, Color::srgba(0.45, 0.38, 0.30, a));
            }
            _ => {}
        }
    }
}

/// Smoke drifting from burning tanks.
fn smoke(
    mut commands: Commands,
    time: Res<Time>,
    feeds: Res<Feeds>,
    v: Res<Viewing>,
    sa: Option<ResMut<SmokeAssets>>,
    mut puffs: Query<(Entity, &mut Smoke, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let Some(mut sa) = sa else { return };
    sa.spawn_t -= dt;
    if sa.spawn_t <= 0.0 {
        sa.spawn_t = 0.18;
        let m = &feeds.0[v.idx];
        for i in 0..m.seats.len() {
            if let Some(s) = m.tank_at(i, v.cursor) {
                if s.on_fire || s.knocked_out {
                    let p = tank_pose(&s).translation + Vec3::Y * 2.2;
                    let j = (time.elapsed_secs() * 7.3 + i as f32).sin();
                    commands.spawn((
                        Smoke { age: 0.0, vel: Vec3::new(0.8 + j * 0.4, 2.2, 0.3 * j) },
                        Mesh3d(sa.mesh.clone()),
                        MeshMaterial3d(sa.mat.clone()),
                        Transform::from_translation(p).with_scale(Vec3::splat(0.8)),
                        StateScoped(Screen::Battlefield),
                    ));
                }
            }
        }
    }
    for (e, mut s, mut t) in &mut puffs {
        s.age += dt;
        t.translation += s.vel * dt;
        t.scale = Vec3::splat(0.8 + s.age * 1.6);
        if s.age > 4.0 {
            commands.entity(e).despawn();
        }
    }
}
