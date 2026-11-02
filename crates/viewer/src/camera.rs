//! Spectator cameras: orbit, follow, and crew-station views. Station views
//! are reconstructions from the spectator feed, not what an agent sees.

use crate::tanks::{tank_pose, turret_rot};
use crate::ui::UiHovered;
use crate::world;
use crate::{Feeds, Playback, Screen, Viewing};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::camera::Projection;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CamState>()
            .add_systems(Startup, spawn)
            .add_systems(Update, directory_orbit.run_if(in_state(Screen::Directory)))
            .add_systems(Update, (cam_input, place).chain().after(Playback).run_if(in_state(Screen::Battlefield)));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Station {
    Gunner,
    Commander,
    Driver,
}

impl Station {
    pub fn label(self) -> &'static str {
        match self {
            Station::Gunner => "Gunner sight",
            Station::Commander => "Commander hatch",
            Station::Driver => "Driver visor",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CamMode {
    Orbit,
    Follow,
    Station(Station),
    /// The sight report the seat's agent is given, in place of a picture.
    Agent,
}

impl CamMode {
    pub const ALL: [CamMode; 6] = [
        CamMode::Orbit,
        CamMode::Follow,
        CamMode::Station(Station::Gunner),
        CamMode::Station(Station::Commander),
        CamMode::Station(Station::Driver),
        CamMode::Agent,
    ];
    pub fn label(self) -> &'static str {
        match self {
            CamMode::Orbit => "Orbit",
            CamMode::Follow => "Follow",
            CamMode::Station(s) => s.label(),
            CamMode::Agent => "Agent's view",
        }
    }
}

#[derive(Resource)]
pub struct CamState {
    pub mode: CamMode,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub focus: Vec3,
    pub follow_pos: Vec3,
    pub follow_look: Vec3,
}

impl Default for CamState {
    fn default() -> Self {
        CamState { mode: CamMode::Orbit, yaw: 2.4, pitch: 0.5, dist: 130.0, focus: Vec3::ZERO, follow_pos: Vec3::ZERO, follow_look: Vec3::ZERO }
    }
}

#[derive(Component)]
pub struct MainCam;

fn spawn(mut commands: Commands) {
    commands.spawn((
        MainCam,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection { fov: 50f32.to_radians(), near: 0.1, far: 2600.0, ..default() }),
        Transform::from_xyz(-120.0, 70.0, 140.0).looking_at(Vec3::ZERO, Vec3::Y),
        DistanceFog { color: world::SKY, falloff: FogFalloff::Linear { start: 320.0, end: 1150.0 }, ..default() },
        SpatialListener::new(0.3),
    ));
}

/// Behind the match directory the village slowly turns, as a backdrop.
fn directory_orbit(time: Res<Time>, mut cam: Query<(&mut Transform, &mut Projection), With<MainCam>>) {
    let Ok((mut t, mut p)) = cam.single_mut() else { return };
    let a = time.elapsed_secs() * 0.04 + 2.2;
    let pos = Vec3::new(a.cos() * 210.0, 70.0, -20.0 + a.sin() * 170.0);
    *t = Transform::from_translation(pos).looking_at(Vec3::new(0.0, 4.0, -20.0), Vec3::Y);
    if let Projection::Perspective(pp) = p.as_mut() {
        pp.fov = 50f32.to_radians();
    }
}

fn cam_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    hovered: Res<UiHovered>,
    mut cs: ResMut<CamState>,
) {
    for (k, m) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6].into_iter().zip(CamMode::ALL) {
        if keys.just_pressed(k) {
            cs.mode = m;
        }
    }
    if hovered.0 {
        return;
    }
    if mouse.pressed(MouseButton::Left) || mouse.pressed(MouseButton::Right) {
        cs.yaw -= motion.delta.x * 0.005;
        cs.pitch = (cs.pitch + motion.delta.y * 0.004).clamp(0.08, 1.45);
    }
    if scroll.delta.y != 0.0 {
        cs.dist = (cs.dist * (1.0 - scroll.delta.y * 0.08)).clamp(15.0, 400.0);
    }
}

fn place(
    feeds: Res<Feeds>,
    v: Res<Viewing>,
    mut cs: ResMut<CamState>,
    time: Res<Time>,
    mut cam: Query<(&mut Transform, &mut Projection), With<MainCam>>,
) {
    let Ok((mut t, mut p)) = cam.single_mut() else { return };
    let m = &feeds.0[v.idx];
    let Some(s) = m.tank_at(v.tank, v.cursor) else { return };
    let hull = tank_pose(&s);
    let mut fov = 50.0f32;
    match cs.mode {
        CamMode::Orbit => {
            // Orbit the midpoint of the battle, eased so jumps stay readable.
            let mut c = Vec3::ZERO;
            let mut n = 0.0;
            for i in 0..m.seats.len() {
                if let Some(o) = m.tank_at(i, v.cursor) {
                    c += tank_pose(&o).translation;
                    n += 1.0;
                }
            }
            let target = if n > 0.0 { c / n } else { Vec3::ZERO };
            let k = 1.0 - (-time.delta_secs() * 3.0).exp();
            cs.focus = if cs.focus == Vec3::ZERO { target } else { cs.focus.lerp(target, k) };
            let dir = Vec3::new(cs.yaw.sin() * cs.pitch.cos(), cs.pitch.sin(), cs.yaw.cos() * cs.pitch.cos());
            let mut pos = cs.focus + dir * cs.dist;
            pos.y = pos.y.max(world::height(pos.x, pos.z) + 3.0);
            *t = Transform::from_translation(pos).looking_at(cs.focus + Vec3::Y * 2.0, Vec3::Y);
        }
        // The agent's view hides the scene; keep the chase camera behind it.
        CamMode::Follow | CamMode::Agent => {
            let fwd = hull.rotation * Vec3::Z;
            let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero();
            // Ease toward the chase point so turns and snapshot steps stay readable.
            let mut want = hull.translation - flat * 18.0 + Vec3::Y * 7.0;
            want.y = want.y.max(world::height(want.x, want.z) + 2.0);
            let look = hull.translation + flat * 10.0 + Vec3::Y * 2.0;
            let k = 1.0 - (-time.delta_secs() * 4.0).exp();
            let near = cs.follow_pos.distance(want) < 60.0;
            cs.follow_pos = if near { cs.follow_pos.lerp(want, k) } else { want };
            cs.follow_look = if near { cs.follow_look.lerp(look, k) } else { look };
            let pos = cs.follow_pos;
            *t = Transform::from_translation(pos).looking_at(cs.follow_look, Vec3::Y);
        }
        CamMode::Station(st) => {
            let (eye, look, f) = match st {
                Station::Gunner => {
                    let r = turret_rot(&s) * Quat::from_rotation_x(-s.gun_elev_deg.to_radians());
                    (hull.translation + turret_rot(&s) * Vec3::new(0.45, 2.15, 1.6), r * Vec3::Z, 9.0)
                }
                Station::Commander => {
                    let r = turret_rot(&s);
                    (hull.translation + r * Vec3::new(0.0, 3.1, -0.9), r * Quat::from_rotation_x(0.06) * Vec3::Z, 50.0)
                }
                Station::Driver => (hull.translation + hull.rotation * Vec3::new(0.6, 1.45, 2.3), hull.rotation * Vec3::Z, 62.0),
            };
            *t = Transform::from_translation(eye).looking_to(look, Vec3::Y);
            fov = f;
        }
    }
    if let Projection::Perspective(pp) = p.as_mut() {
        pp.fov = fov.to_radians();
    }
}
