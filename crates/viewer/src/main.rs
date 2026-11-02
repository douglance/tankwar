//! Tankwar human spectator.
//!
//! Screens: match directory, live battlefield, and replay. The viewer renders
//! the spectator event stream; it never computes outcomes or talks to agents.

mod audio;
mod camera;
mod contract;
mod feed;
mod tanks;
mod ui;
mod world;

use bevy::asset::AssetMetaCheck;
use bevy::audio::{AudioPlugin, SpatialScale};
use bevy::prelude::*;
use feed::{Match, TICK_HZ};

#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Screen {
    #[default]
    Directory,
    Battlefield,
}

#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Playback;

#[derive(Resource)]
pub struct Feeds(pub Vec<Match>);

/// What the spectator is watching and where the playback cursor sits.
#[derive(Resource, Default)]
pub struct Viewing {
    pub idx: usize,
    pub live: bool,
    /// Live only: seconds of match time the feed would have delivered by now.
    pub wall_s: f32,
    /// Playback position in ticks.
    pub cursor: f32,
    pub prev_cursor: f32,
    pub paused: bool,
    pub speed: f32,
    /// Selected tank seat.
    pub tank: usize,
    pub lag_s: Option<f32>,
}

/// Live spectators trail the authoritative head by this many ticks.
pub const LIVE_DELAY_TICKS: f32 = 6.0;

impl Viewing {
    pub fn open(idx: usize, live: bool, m: &Match) -> Self {
        let start = if live { 62.0 } else { 0.0 };
        Viewing {
            idx,
            live,
            wall_s: start,
            cursor: start * TICK_HZ,
            prev_cursor: start * TICK_HZ,
            paused: false,
            speed: 1.0,
            tank: 0,
            lag_s: None,
        }
        .clamped(m)
    }
    fn clamped(mut self, m: &Match) -> Self {
        self.cursor = self.cursor.clamp(0.0, m.end_tick as f32);
        self
    }
    /// Jump the live clock, used by capture mode and tests of the lag label.
    pub fn set_wall(&mut self, m: &Match, wall_s: f32) {
        self.wall_s = wall_s;
        let (head, _) = live_head_s(m, wall_s);
        self.cursor = (head * TICK_HZ - LIVE_DELAY_TICKS).clamp(0.0, m.end_tick as f32);
        self.prev_cursor = self.cursor;
    }
}

/// Where the delivered feed has reached at a wall-clock time, and how far it
/// is behind if delivery is stalled.
pub fn live_head_s(m: &Match, wall_s: f32) -> (f32, Option<f32>) {
    for st in &m.stalls {
        if wall_s >= st.at_s && wall_s < st.at_s + st.len_s {
            return (st.at_s, Some(wall_s - st.at_s));
        }
    }
    (wall_s.min(m.duration_s()), None)
}

fn advance_playback(time: Res<Time>, feeds: Res<Feeds>, mut v: ResMut<Viewing>) {
    let m = &feeds.0[v.idx];
    v.prev_cursor = v.cursor;
    let dt = time.delta_secs().min(0.1);
    if v.live {
        v.wall_s += dt;
        let (head, lag) = live_head_s(m, v.wall_s);
        v.lag_s = lag;
        let target = (head * TICK_HZ - LIVE_DELAY_TICKS).clamp(0.0, m.end_tick as f32);
        // Ease toward the head after a stall rather than teleporting.
        let catch_up = if target - v.cursor > 4.0 { 3.0 } else { 1.0 };
        v.cursor = (v.cursor + dt * TICK_HZ * catch_up).min(target);
        if target < v.cursor {
            v.cursor = target;
        }
    } else {
        v.lag_s = None;
        if !v.paused {
            v.cursor += dt * TICK_HZ * v.speed;
            if v.cursor >= m.end_tick as f32 {
                v.cursor = m.end_tick as f32;
                v.paused = true;
            }
        }
    }
}

fn playback_keys(
    keys: Res<ButtonInput<KeyCode>>,
    feeds: Res<Feeds>,
    mut v: ResMut<Viewing>,
    mut next: ResMut<NextState<Screen>>,
) {
    let m = &feeds.0[v.idx];
    if keys.just_pressed(KeyCode::Escape) {
        next.set(Screen::Directory);
    }
    if keys.just_pressed(KeyCode::Tab) {
        v.tank = (v.tank + 1) % m.seats.len();
    }
    if v.live {
        return;
    }
    if keys.just_pressed(KeyCode::Space) {
        v.paused = !v.paused;
    }
    let step = 10.0 * TICK_HZ;
    if keys.just_pressed(KeyCode::ArrowRight) {
        v.cursor = (v.cursor + step).min(m.end_tick as f32);
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        v.cursor = (v.cursor - step).max(0.0);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        v.speed = ui::next_speed(v.speed, true);
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        v.speed = ui::next_speed(v.speed, false);
    }
}

fn main() {
    let asset_root = if cfg!(target_arch = "wasm32") { "assets" } else { "../../assets/derived" };
    let mut app = App::new();
    app.insert_resource(ClearColor(world::SKY))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Tankwar spectator".into(),
                        canvas: Some("#bevy".into()),
                        fit_canvas_to_parent: true,
                        resolution: (1600.0_f32, 900.0_f32).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin { file_path: asset_root.into(), meta_check: AssetMetaCheck::Never, ..default() })
                .set(AudioPlugin { default_spatial_scale: SpatialScale::new(1.0 / 14.0), ..default() }),
        )
        .init_state::<Screen>()
        .enable_state_scoped_entities::<Screen>()
        .insert_resource(Feeds(all_matches()))
        .init_resource::<Viewing>()
        .add_plugins((world::WorldPlugin, tanks::TanksPlugin, camera::CameraPlugin, ui::UiPlugin, audio::AudioFxPlugin))
        .add_systems(
            Update,
            (playback_keys, advance_playback).chain().in_set(Playback).run_if(in_state(Screen::Battlefield)),
        );
    #[cfg(not(target_arch = "wasm32"))]
    capture::add(&mut app);
    app.run();
}

/// Native capture mode: `TANKWAR_CAPTURE=<dir>` walks every screen, saves a
/// screenshot of each, and exits. Used as visual evidence of the built scene.
#[cfg(not(target_arch = "wasm32"))]
mod capture {
    use super::*;
    use crate::camera::{CamMode, CamState};
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    use std::path::PathBuf;

    #[derive(Resource)]
    struct Cap {
        dir: PathBuf,
        frame: u32,
        step: usize,
    }

    pub fn add(app: &mut App) {
        let Ok(dir) = std::env::var("TANKWAR_CAPTURE") else { return };
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("capture dir");
        app.insert_resource(Cap { dir, frame: 0, step: 0 }).add_systems(Last, drive);
    }

    enum Act {
        Live { wall: f32, mode: CamMode, tank: usize, yaw: f32 },
        Replay { idx: usize, at_s: f32, mode: CamMode, tank: usize },
        Dir,
    }

    const SHOTS: &[(&str, u32)] = &[
        ("01-directory", 300),
        ("02-live-orbit", 200),
        ("03-live-follow", 90),
        ("04-live-gunner", 60),
        ("05-live-driver", 60),
        ("06-live-commander", 60),
        ("07-live-disconnect", 90),
        ("08-live-lag", 60),
        ("09-live-recovery", 60),
        ("10-replay", 120),
        ("11-replay-missing-interval", 120),
        ("12-replay-abort", 120),
        ("13-replay-result", 120),
    ];

    fn act(step: usize) -> Act {
        use CamMode::*;
        match step {
            0 => Act::Dir,
            1 => Act::Live { wall: 66.0, mode: Orbit, tank: 0, yaw: 2.6 },
            2 => Act::Live { wall: 68.0, mode: Follow, tank: 0, yaw: 0.0 },
            3 => Act::Live { wall: 69.0, mode: Station(crate::camera::Station::Gunner), tank: 0, yaw: 0.0 },
            4 => Act::Live { wall: 70.0, mode: Station(crate::camera::Station::Driver), tank: 1, yaw: 0.0 },
            5 => Act::Live { wall: 71.0, mode: Station(crate::camera::Station::Commander), tank: 0, yaw: 0.0 },
            6 => Act::Live { wall: 82.0, mode: Orbit, tank: 2, yaw: 3.4 },
            7 => Act::Live { wall: 117.5, mode: Follow, tank: 1, yaw: 0.0 },
            8 => Act::Live { wall: 43.0, mode: Orbit, tank: 0, yaw: 1.9 },
            9 => Act::Replay { idx: 1, at_s: 88.0, mode: Orbit, tank: 0 },
            10 => Act::Replay { idx: 2, at_s: 90.0, mode: Follow, tank: 1 },
            11 => Act::Replay { idx: 2, at_s: 9999.0, mode: Orbit, tank: 0 },
            _ => Act::Replay { idx: 1, at_s: 9999.0, mode: Follow, tank: 0 },
        }
    }

    fn drive(
        mut commands: Commands,
        mut cap: ResMut<Cap>,
        feeds: Res<Feeds>,
        mut v: ResMut<Viewing>,
        mut cam: ResMut<CamState>,
        mut next: ResMut<NextState<Screen>>,
        mut exit: EventWriter<AppExit>,
    ) {
        if cap.step >= SHOTS.len() {
            exit.write(AppExit::Success);
            return;
        }
        let (name, wait) = SHOTS[cap.step];
        if cap.frame == 0 {
            match act(cap.step) {
                Act::Dir => next.set(Screen::Directory),
                Act::Live { wall, mode, tank, yaw } => {
                    let m = &feeds.0[0];
                    if !v.live || v.idx != 0 {
                        *v = Viewing::open(0, true, m);
                    }
                    v.set_wall(m, wall);
                    v.tank = tank;
                    cam.mode = mode;
                    cam.yaw = yaw;
                    cam.pitch = 0.5;
                    cam.dist = 120.0;
                    next.set(Screen::Battlefield);
                }
                Act::Replay { idx, at_s, mode, tank } => {
                    let m = &feeds.0[idx];
                    *v = Viewing::open(idx, false, m);
                    v.cursor = (at_s * TICK_HZ).min(m.end_tick as f32);
                    v.prev_cursor = v.cursor;
                    v.paused = true;
                    v.tank = tank;
                    cam.mode = mode;
                    cam.yaw = 2.2;
                    cam.pitch = 0.55;
                    cam.dist = 140.0;
                    // Leave through the directory, as a spectator would, so
                    // the battlefield is rebuilt for the new match.
                    next.set(Screen::Directory);
                }
            }
        }
        if cap.frame == 2 && matches!(act(cap.step), Act::Replay { .. }) {
            next.set(Screen::Battlefield);
        }
        cap.frame += 1;
        if cap.frame == wait {
            let path = cap.dir.join(format!("{name}.png"));
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        }
        if cap.frame == wait + 15 {
            cap.step += 1;
            cap.frame = 0;
        }
    }
}

/// Demo feeds plus a sample recorded in the contract v1 schema. The sample is
/// not a real match, so it is labelled as a demo too.
fn all_matches() -> Vec<feed::Match> {
    let mut v = feed::demo_matches();
    if let Ok(mut m) = contract::from_json(include_str!("../fixtures/contract-v1-sample.json")) {
        m.demo = true;
        m.mode = "Contract v1 sample";
        v.push(m);
    }
    v
}
