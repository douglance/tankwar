//! Spectator interface: the match directory, and the battlefield HUD shared
//! by live and replay. Every string that came from an agent or the feed is
//! escaped before display and rendered as plain text, never as markup.

use crate::camera::{CamMode, CamState, MainCam, Station};
use crate::feed::{sanitize_command_text as clean, EventKind, Match, Round, Status, CREW_ROLES, PARTS, TICK_HZ};
use crate::tanks::{side_color, tank_pose};
use crate::{Feeds, Playback, Screen, Viewing};
use bevy::prelude::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiHovered>()
            .add_systems(OnEnter(Screen::Directory), directory)
            .add_systems(OnEnter(Screen::Battlefield), hud)
            .add_systems(Update, (hover, button_tint))
            .add_systems(Update, directory_buttons.run_if(in_state(Screen::Directory)))
            .add_systems(
                Update,
                (hud_buttons.before(Playback), (top_bar, chips, tank_panel, timeline, transport, labels, overlays).after(Playback))
                    .run_if(in_state(Screen::Battlefield)),
            );
    }
}

/// True while the pointer is over interface, so camera drags stay out of it.
#[derive(Resource, Default)]
pub struct UiHovered(pub bool);

pub const SPEEDS: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];

pub fn next_speed(cur: f32, up: bool) -> f32 {
    let i = SPEEDS.iter().position(|s| (*s - cur).abs() < 1e-3).unwrap_or(2);
    if up { SPEEDS[(i + 1).min(SPEEDS.len() - 1)] } else { SPEEDS[i.saturating_sub(1)] }
}

const INK: Color = Color::srgb(0.93, 0.91, 0.85);
const DIM: Color = Color::srgb(0.66, 0.64, 0.58);
const PANEL: Color = Color::srgba(0.10, 0.10, 0.09, 0.82);
const BTN: Color = Color::srgb(0.24, 0.23, 0.20);
const BTN_HOT: Color = Color::srgb(0.36, 0.34, 0.28);
const BTN_ON: Color = Color::srgb(0.55, 0.45, 0.24);
const RED: Color = Color::srgb(0.72, 0.22, 0.18);
const AMBER: Color = Color::srgb(0.80, 0.56, 0.16);
const GREEN: Color = Color::srgb(0.30, 0.52, 0.28);
const SLATE: Color = Color::srgb(0.32, 0.38, 0.44);

fn clock(s: f32) -> String {
    let s = s.max(0.0);
    format!("{:02}:{:04.1}", (s / 60.0) as u32, s % 60.0)
}

fn font(size: f32) -> TextFont {
    TextFont { font_size: size, ..default() }
}

fn hover(q: Query<&Interaction>, mut h: ResMut<UiHovered>) {
    h.0 = q.iter().any(|i| *i != Interaction::None);
}

/// Buttons that are toggled on keep their highlight.
#[derive(Component)]
struct On(bool);

fn button_tint(mut q: Query<(&Interaction, &mut BackgroundColor, Option<&On>), With<Button>>) {
    for (i, mut bg, on) in &mut q {
        let on = on.map(|o| o.0).unwrap_or(false);
        bg.0 = match (i, on) {
            (_, true) => BTN_ON,
            (Interaction::Hovered | Interaction::Pressed, _) => BTN_HOT,
            _ => BTN,
        };
    }
}

fn button(label: impl Into<String>, size: f32) -> impl Bundle {
    (
        Button,
        Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)), margin: UiRect::all(Val::Px(2.0)), ..default() },
        BackgroundColor(BTN),
        BorderRadius::all(Val::Px(3.0)),
        children![(Text::new(label), font(size), TextColor(INK))],
    )
}

// ---------------------------------------------------------------- directory

#[derive(Component)]
struct Watch {
    idx: usize,
    live: bool,
}

fn directory(mut commands: Commands, feeds: Res<Feeds>) {
    let root = commands
        .spawn((
            StateScoped(Screen::Directory),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(36.0)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .id();
    commands.entity(root).with_children(|r| {
        r.spawn((Text::new("TANKWAR"), font(40.0), TextColor(INK)));
        r.spawn((Text::new("Spectator - match directory"), font(18.0), TextColor(DIM), Node { margin: UiRect::bottom(Val::Px(18.0)), ..default() }));
        for (idx, m) in feeds.0.iter().enumerate() {
            let (chip, col) = match m.status {
                Status::Live => ("LIVE", RED),
                Status::Finished => ("RESULT", GREEN),
                Status::Aborted => ("ABORTED", SLATE),
            };
            r.spawn((
                Interaction::default(),
                Node {
                    width: Val::Px(560.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(14.0)),
                    margin: UiRect::bottom(Val::Px(12.0)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderRadius::all(Val::Px(4.0)),
            ))
            .with_children(|c| {
                c.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, ..default() }).with_children(|row| {
                    row.spawn((
                        Node { padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)), margin: UiRect::right(Val::Px(10.0)), ..default() },
                        BackgroundColor(col),
                        children![(Text::new(chip), font(13.0), TextColor(INK))],
                    ));
                    row.spawn((Text::new(format!("{}  {}  {}", clean(&m.id), m.mode, m.map)), font(17.0), TextColor(INK)));
                });
                let west: Vec<String> = m.seats.iter().filter(|s| s.side == crate::feed::Side::West).map(|s| format!("{} ({})", clean(&s.agent), s.vehicle.label())).collect();
                let east: Vec<String> = m.seats.iter().filter(|s| s.side == crate::feed::Side::East).map(|s| format!("{} ({})", clean(&s.agent), s.vehicle.label())).collect();
                c.spawn((Text::new(format!("West: {}\nEast: {}", west.join(", "), east.join(", "))), font(14.0), TextColor(DIM), Node { margin: UiRect::vertical(Val::Px(6.0)), ..default() }));
                let outcome = match m.status {
                    Status::Live => format!("In progress - {} recorded", clock(m.duration_s())),
                    _ => format!("{} - {}", clock(m.duration_s()), clean(m.result_text().unwrap_or("no result"))),
                };
                c.spawn((Text::new(outcome), font(14.0), TextColor(DIM)));
                c.spawn(Node { flex_direction: FlexDirection::Row, margin: UiRect::top(Val::Px(8.0)), ..default() }).with_children(|row| {
                    if m.status == Status::Live {
                        row.spawn((button("Watch live", 15.0), Watch { idx, live: true }));
                    }
                    row.spawn((button("Replay", 15.0), Watch { idx, live: false }));
                });
            });
        }
        r.spawn((
            Text::new("Demo feed: these matches come from a local generator until the server's spectator stream is wired in.\nSpectator sound is ambience for people watching; agents never receive it."),
            font(13.0),
            TextColor(DIM),
            Node { margin: UiRect::top(Val::Px(8.0)), max_width: Val::Px(560.0), ..default() },
        ));
    });
}

fn directory_buttons(
    q: Query<(&Interaction, &Watch), Changed<Interaction>>,
    feeds: Res<Feeds>,
    mut v: ResMut<Viewing>,
    mut cam: ResMut<CamState>,
    mut next: ResMut<NextState<Screen>>,
) {
    for (i, w) in &q {
        if *i == Interaction::Pressed {
            *v = Viewing::open(w.idx, w.live, &feeds.0[w.idx]);
            cam.focus = Vec3::ZERO;
            next.set(Screen::Battlefield);
        }
    }
}

// ---------------------------------------------------------------- battlefield HUD

#[derive(Component)]
struct TopInfo;
#[derive(Component, Clone, Copy, PartialEq)]
enum Chip {
    Live,
    Lag,
    Disconnect,
    Recovery,
    Missing,
    Result,
    Aborted,
    Replay,
    Paused,
    Demo,
}
#[derive(Component)]
struct ChipText(Chip);
#[derive(Component)]
struct Banner;
#[derive(Component)]
struct CamBtn(CamMode);
#[derive(Component)]
struct Back;
#[derive(Component)]
struct TankBtn(usize);
#[derive(Component)]
struct PanelText;
#[derive(Component)]
struct FuelFill;
#[derive(Component)]
struct TimelineText;
#[derive(Component)]
struct TransportBar;
#[derive(Component)]
struct PlayBtn;
#[derive(Component)]
struct SpeedBtn(f32);
#[derive(Component)]
struct SeekSeg(usize);
#[derive(Component)]
struct TimeText;
#[derive(Component)]
struct NameLabel(usize);
#[derive(Component)]
struct Overlay(Station);
#[derive(Component)]
struct StationNote;

const SEGS: usize = 100;

fn hud(mut commands: Commands, feeds: Res<Feeds>, v: Res<Viewing>) {
    let m = &feeds.0[v.idx];
    let scoped = StateScoped(Screen::Battlefield);
    let full = Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };

    // Station overlays sit underneath the panels.
    commands.spawn((scoped.clone(), full.clone(), Overlay(Station::Gunner), Visibility::Hidden)).with_children(|o| {
        let mask = Color::srgba(0.02, 0.02, 0.02, 0.92);
        for (l, t, w, h) in [(0.0, 0.0, 100.0, 22.0), (0.0, 78.0, 100.0, 22.0), (0.0, 22.0, 33.0, 56.0), (67.0, 22.0, 33.0, 56.0)] {
            o.spawn((Node { position_type: PositionType::Absolute, left: Val::Percent(l), top: Val::Percent(t), width: Val::Percent(w), height: Val::Percent(h), ..default() }, BackgroundColor(mask)));
        }
        let ink = Color::srgba(0.05, 0.05, 0.04, 0.9);
        o.spawn((Node { position_type: PositionType::Absolute, left: Val::Percent(33.0), top: Val::Percent(50.0), width: Val::Percent(14.0), height: Val::Px(2.0), ..default() }, BackgroundColor(ink)));
        o.spawn((Node { position_type: PositionType::Absolute, left: Val::Percent(53.0), top: Val::Percent(50.0), width: Val::Percent(14.0), height: Val::Px(2.0), ..default() }, BackgroundColor(ink)));
        o.spawn((Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), top: Val::Percent(53.0), width: Val::Px(2.0), height: Val::Percent(25.0), ..default() }, BackgroundColor(ink)));
        for k in 0..4 {
            let x = 50.0 - 1.4 + (k as f32 - 1.5) * 1.8;
            o.spawn((Node { position_type: PositionType::Absolute, left: Val::Percent(x + 1.4), top: Val::Percent(49.0 - k as f32 * 0.0), width: Val::Px(9.0), height: Val::Px(9.0), border: UiRect::all(Val::Px(2.0)), ..default() }, BorderColor(ink)));
        }
    });
    commands.spawn((scoped.clone(), full.clone(), Overlay(Station::Driver), Visibility::Hidden)).with_children(|o| {
        let mask = Color::srgb(0.07, 0.07, 0.06);
        o.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(36.0), ..default() }, BackgroundColor(mask)));
        o.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Percent(60.0), width: Val::Percent(100.0), height: Val::Percent(40.0), ..default() }, BackgroundColor(mask)));
    });
    commands.spawn((scoped.clone(), full.clone(), Overlay(Station::Commander), Visibility::Hidden));
    commands.spawn((
        scoped.clone(),
        StationNote,
        Visibility::Hidden,
        Text::new(""),
        font(14.0),
        TextColor(Color::srgb(0.86, 0.82, 0.70)),
        Node { position_type: PositionType::Absolute, left: Val::Percent(38.0), top: Val::Percent(23.0), ..default() },
    ));

    for i in 0..m.seats.len() {
        commands.spawn((
            scoped.clone(),
            NameLabel(i),
            Text::new(clean(&m.seats[i].agent)),
            font(13.0),
            TextColor(side_color(m.seats[i].side)),
            Node { position_type: PositionType::Absolute, ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
        ));
    }

    // Top bar.
    commands
        .spawn((
            scoped.clone(),
            Node { position_type: PositionType::Absolute, top: Val::Px(0.0), left: Val::Px(0.0), width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
        ))
        .with_children(|top| {
            top.spawn((
                Interaction::default(),
                Node { width: Val::Percent(100.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, ..default() },
                BackgroundColor(PANEL),
            ))
            .with_children(|bar| {
                bar.spawn((button("< Matches", 14.0), Back));
                bar.spawn((TopInfo, Text::new(""), font(16.0), TextColor(INK), Node { margin: UiRect::horizontal(Val::Px(12.0)), ..default() }));
                for c in [Chip::Live, Chip::Replay, Chip::Paused, Chip::Lag, Chip::Disconnect, Chip::Recovery, Chip::Missing, Chip::Result, Chip::Aborted, Chip::Demo] {
                    let bg = match c {
                        Chip::Live | Chip::Disconnect => RED,
                        Chip::Lag | Chip::Recovery | Chip::Missing => AMBER,
                        Chip::Result => GREEN,
                        _ => SLATE,
                    };
                    bar.spawn((c, Node { padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)), margin: UiRect::right(Val::Px(6.0)), display: Display::None, ..default() }, BackgroundColor(bg)))
                        .with_children(|ch| {
                            ch.spawn((ChipText(c), Text::new(""), font(13.0), TextColor(INK)));
                        });
                }
                bar.spawn(Node { flex_grow: 1.0, ..default() });
                for mode in CamMode::ALL {
                    bar.spawn((button(mode.label(), 13.0), CamBtn(mode), On(false)));
                }
            });
            top.spawn((
                Banner,
                Text::new(""),
                font(22.0),
                TextColor(INK),
                Node { align_self: AlignSelf::Center, margin: UiRect::top(Val::Px(10.0)), padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)), display: Display::None, ..default() },
                BackgroundColor(Color::srgba(0.08, 0.08, 0.07, 0.85)),
            ));
        });

    // Left: selected tank.
    commands
        .spawn((
            scoped.clone(),
            Interaction::default(),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                top: Val::Px(56.0),
                width: Val::Px(270.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|p| {
            p.spawn(Node { flex_wrap: FlexWrap::Wrap, margin: UiRect::bottom(Val::Px(6.0)), ..default() }).with_children(|r| {
                for (i, s) in m.seats.iter().enumerate() {
                    r.spawn((button(clean(&s.agent), 12.0), TankBtn(i), On(false)));
                }
            });
            p.spawn((Text::new("Fuel"), font(13.0), TextColor(DIM)));
            p.spawn((Node { width: Val::Percent(100.0), height: Val::Px(8.0), margin: UiRect::vertical(Val::Px(3.0)), ..default() }, BackgroundColor(Color::srgb(0.2, 0.2, 0.18))))
                .with_children(|b| {
                    b.spawn((FuelFill, Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(AMBER)));
                });
            p.spawn((PanelText, Text::new(""), font(13.0), TextColor(INK)));
        });

    // Right: command timeline.
    commands
        .spawn((
            scoped.clone(),
            Interaction::default(),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                top: Val::Px(56.0),
                width: Val::Px(360.0),
                max_height: Val::Percent(72.0),
                overflow: Overflow::clip(),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|p| {
            p.spawn((Text::new("Command timeline (newest first)"), font(14.0), TextColor(DIM)));
            p.spawn((TimelineText, Text::new(""), font(12.5), TextColor(INK), Node { margin: UiRect::top(Val::Px(6.0)), ..default() }));
        });

    // Bottom: transport (replay) or live status.
    commands
        .spawn((
            scoped,
            TransportBar,
            Interaction::default(),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(10.0),
                left: Val::Percent(18.0),
                width: Val::Percent(64.0),
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|b| {
            if !v.live {
                b.spawn((button("Pause", 14.0), PlayBtn));
                for s in SPEEDS {
                    let l = if s < 1.0 { format!("{s}x") } else { format!("{}x", s as u32) };
                    b.spawn((button(l, 12.0), SpeedBtn(s), On(false)));
                }
            }
            b.spawn(Node { flex_grow: 1.0, height: Val::Px(16.0), margin: UiRect::horizontal(Val::Px(8.0)), ..default() }).with_children(|bar| {
                for i in 0..SEGS {
                    let mut e = bar.spawn((SeekSeg(i), Node { flex_grow: 1.0, height: Val::Percent(100.0), margin: UiRect::right(Val::Px(1.0)), ..default() }, BackgroundColor(BTN)));
                    if !v.live {
                        e.insert(Interaction::default());
                    }
                }
            });
            b.spawn((TimeText, Text::new(""), font(14.0), TextColor(INK)));
        });
}

fn hud_buttons(
    q: Query<(&Interaction, Option<&CamBtn>, Option<&TankBtn>, Option<&PlayBtn>, Option<&SpeedBtn>, Option<&SeekSeg>, Option<&Back>), Changed<Interaction>>,
    feeds: Res<Feeds>,
    mut v: ResMut<Viewing>,
    mut cam: ResMut<CamState>,
    mut next: ResMut<NextState<Screen>>,
) {
    let end = feeds.0[v.idx].end_tick as f32;
    for (i, c, t, p, s, seg, back) in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        if let Some(c) = c {
            cam.mode = c.0;
        }
        if let Some(t) = t {
            v.tank = t.0;
        }
        if p.is_some() {
            if v.cursor >= end {
                v.cursor = 0.0;
                v.prev_cursor = 0.0;
            }
            v.paused = !v.paused;
        }
        if let Some(s) = s {
            v.speed = s.0;
        }
        if let Some(seg) = seg {
            if !v.live {
                v.cursor = (seg.0 as f32 + 0.5) / SEGS as f32 * end;
                v.prev_cursor = v.cursor;
            }
        }
        if back.is_some() {
            next.set(Screen::Directory);
        }
    }
}

/// Feed facts at the cursor that the labels need.
struct Now {
    disconnected: Vec<usize>,
    recovery: Option<u32>,
    missing: Option<(u32, u32)>,
    result: Option<String>,
    aborted: Option<String>,
}

fn now(m: &Match, cursor: f32) -> Now {
    let mut n = Now { disconnected: Vec::new(), recovery: None, missing: None, result: None, aborted: None };
    let mut out = vec![false; m.seats.len()];
    for e in &m.events[..m.events_until(cursor)] {
        match &e.kind {
            EventKind::AgentDisconnected { tank } => out[*tank] = true,
            EventKind::AgentReconnected { tank } => out[*tank] = false,
            EventKind::ServerRecovery { paused_ticks } if cursor - (e.tick as f32) < 8.0 * TICK_HZ => n.recovery = Some(*paused_ticks),
            EventKind::Result { text } => n.result = Some(clean(text)),
            EventKind::Aborted { reason } => n.aborted = Some(clean(reason)),
            _ => {}
        }
    }
    n.disconnected = (0..out.len()).filter(|i| out[*i]).collect();
    n.missing = m.missing_intervals().find(|(a, b)| cursor >= *a as f32 && cursor < *b as f32);
    n
}

fn top_bar(feeds: Res<Feeds>, v: Res<Viewing>, mut q: Query<&mut Text, With<TopInfo>>) {
    let m = &feeds.0[v.idx];
    for mut t in &mut q {
        t.0 = format!("{}  {}  {}", clean(&m.id), m.mode, m.map);
    }
}

fn chips(
    feeds: Res<Feeds>,
    v: Res<Viewing>,
    mut nodes: Query<(&Chip, &mut Node)>,
    mut texts: Query<(&ChipText, &mut Text), Without<Banner>>,
    mut banner: Query<(&mut Text, &mut Node), (With<Banner>, Without<Chip>, Without<ChipText>)>,
) {
    let m = &feeds.0[v.idx];
    let n = now(m, v.cursor);
    let names: Vec<String> = n.disconnected.iter().map(|i| clean(&m.seats[*i].agent)).collect();
    let label = |c: Chip| -> Option<String> {
        match c {
            Chip::Live => v.live.then(|| "LIVE".into()),
            Chip::Replay => (!v.live).then(|| "REPLAY".into()),
            Chip::Paused => (!v.live && v.paused).then(|| "PAUSED".into()),
            Chip::Lag => v.lag_s.filter(|l| *l > 0.5).map(|l| format!("LAG {l:.1}s - feed stalled")),
            Chip::Disconnect => (!names.is_empty()).then(|| format!("DISCONNECTED: {}", names.join(", "))),
            Chip::Recovery => n.recovery.map(|p| format!("SERVER RECOVERY - paused {:.1}s", p as f32 / TICK_HZ)),
            Chip::Missing => n.missing.map(|(a, b)| format!("MISSING INTERVAL {}-{} - no data", clock(a as f32 / TICK_HZ), clock(b as f32 / TICK_HZ))),
            Chip::Result => n.result.as_ref().map(|_| "RESULT".into()),
            Chip::Aborted => n.aborted.as_ref().map(|_| "ABORTED".into()),
            Chip::Demo => m.demo.then(|| "DEMO FEED".into()),
        }
    };
    for (c, mut node) in &mut nodes {
        node.display = if label(*c).is_some() { Display::Flex } else { Display::None };
    }
    for (c, mut t) in &mut texts {
        if let Some(l) = label(c.0) {
            t.0 = l;
        }
    }
    let msg = if let Some(r) = &n.aborted {
        Some(format!("Match aborted: {r}"))
    } else if let Some(r) = &n.result {
        Some(format!("Result: {r}"))
    } else if let Some((a, b)) = n.missing {
        Some(format!("Missing interval: no spectator data from {} to {}", clock(a as f32 / TICK_HZ), clock(b as f32 / TICK_HZ)))
    } else if let Some(l) = v.lag_s.filter(|l| *l > 0.5) {
        Some(format!("Feed delayed {l:.1}s - holding last known state"))
    } else if !names.is_empty() {
        Some(format!("{} disconnected - tank holds its last orders", names.join(", ")))
    } else if let Some(p) = n.recovery {
        Some(format!("Server recovered - simulation paused {:.1}s", p as f32 / TICK_HZ))
    } else {
        None
    };
    for (mut t, mut node) in &mut banner {
        match &msg {
            Some(s) => {
                t.0 = s.clone();
                node.display = Display::Flex;
            }
            None => node.display = Display::None,
        }
    }
}

fn tank_panel(
    feeds: Res<Feeds>,
    v: Res<Viewing>,
    cam: Res<CamState>,
    mut text: Query<&mut Text, With<PanelText>>,
    mut fuel: Query<(&mut Node, &mut BackgroundColor), With<FuelFill>>,
    mut toggles: Query<(&mut On, Option<&TankBtn>, Option<&CamBtn>, Option<&SpeedBtn>)>,
) {
    let m = &feeds.0[v.idx];
    for (mut on, t, c, s) in &mut toggles {
        on.0 = t.map(|t| t.0 == v.tank).unwrap_or(false) || c.map(|c| c.0 == cam.mode).unwrap_or(false) || s.map(|s| (s.0 - v.speed).abs() < 1e-3).unwrap_or(false);
    }
    let seat = &m.seats[v.tank];
    let Some(s) = m.tank_at(v.tank, v.cursor) else { return };
    let cap = seat.vehicle.demo_fuel_capacity_l();
    let frac = (s.fuel_l / cap).clamp(0.0, 1.0);
    for (mut n, mut bg) in &mut fuel {
        n.width = Val::Percent(frac * 100.0);
        bg.0 = if frac < 0.2 { RED } else { AMBER };
    }
    let mut out = format!(
        "{} - {} side\n{}\n{:.0} / {:.0} L   {:.1} km/h   engine {}\n\nAmmunition\n",
        clean(&seat.agent),
        seat.side.label(),
        seat.vehicle.label(),
        s.fuel_l,
        cap,
        s.speed.abs() * 3.6,
        if s.engine_on { "running" } else { "off" },
    );
    for r in Round::ALL {
        out += &format!("  {:<6} {:>3}\n", r.label(), s.ammo[r.index()]);
    }
    out += &format!(
        "Loaded: {}\n",
        match s.loaded {
            Some(r) if s.reload <= 0.0 => format!("{} - ready", r.label()),
            Some(r) => format!("{} - loading {:.0}%", r.label(), s.reload * 100.0),
            None if s.reload > 0.0 => format!("loading {:.0}%", s.reload * 100.0),
            None => "empty".into(),
        }
    );
    out += "\nCrew\n";
    for (i, c) in s.crew.iter().enumerate() {
        out += &format!("  {:<10} {}\n", CREW_ROLES[i], c.label());
    }
    out += "\nDamage\n";
    for (i, p) in s.parts.iter().enumerate() {
        out += &format!("  {:<11} {}\n", PARTS[i], p.label());
    }
    if s.on_fire {
        out += "  ON FIRE\n";
    }
    if s.knocked_out {
        out += "  KNOCKED OUT\n";
    }
    for mut t in &mut text {
        t.0 = out.clone();
    }
}

fn timeline(feeds: Res<Feeds>, v: Res<Viewing>, mut q: Query<&mut Text, With<TimelineText>>) {
    let m = &feeds.0[v.idx];
    let end = m.events_until(v.cursor);
    let name = |i: usize| clean(&m.seats[i].agent);
    let mut lines = Vec::new();
    for e in m.events[..end].iter().rev() {
        let t = clock(e.tick as f32 / TICK_HZ);
        let line = match &e.kind {
            EventKind::CommandSubmitted { cmd, tank, text } => format!("{t} {} #{cmd} sent: {}", name(*tank), clean(text)),
            EventKind::CommandDecision { cmd, tank, accepted, reason } => {
                if *accepted {
                    format!("{t} {} #{cmd} accepted", name(*tank))
                } else {
                    format!("{t} {} #{cmd} REJECTED: {}", name(*tank), clean(reason))
                }
            }
            EventKind::CommandPhase { cmd, tank, phase } => format!("{t} {} #{cmd} {}", name(*tank), phase.label()),
            EventKind::AgentDisconnected { tank } => format!("{t} -- {} disconnected", name(*tank)),
            EventKind::AgentReconnected { tank } => format!("{t} -- {} reconnected", name(*tank)),
            EventKind::ServerRecovery { paused_ticks } => format!("{t} -- server recovery ({:.1}s)", *paused_ticks as f32 / TICK_HZ),
            EventKind::MissingInterval { from_tick, to_tick } => {
                format!("{t} -- missing interval {}-{}", clock(*from_tick as f32 / TICK_HZ), clock(*to_tick as f32 / TICK_HZ))
            }
            EventKind::Result { text } => format!("{t} -- result: {}", clean(text)),
            EventKind::Aborted { reason } => format!("{t} -- aborted: {}", clean(reason)),
            EventKind::MatchStarted => format!("{t} -- match started"),
            _ => continue,
        };
        lines.push(line);
        if lines.len() >= 26 {
            break;
        }
    }
    for mut tx in &mut q {
        tx.0 = lines.join("\n");
    }
}

fn transport(
    feeds: Res<Feeds>,
    v: Res<Viewing>,
    mut segs: Query<(&SeekSeg, &mut BackgroundColor, Option<&Interaction>)>,
    mut time: Query<&mut Text, With<TimeText>>,
    play: Query<&Children, With<PlayBtn>>,
    mut texts: Query<&mut Text, Without<TimeText>>,
) {
    let m = &feeds.0[v.idx];
    let end = m.end_tick as f32;
    let gaps: Vec<(u32, u32)> = m.missing_intervals().collect();
    for (s, mut bg, i) in &mut segs {
        let a = s.0 as f32 / SEGS as f32 * end;
        let b = (s.0 + 1) as f32 / SEGS as f32 * end;
        let gap = gaps.iter().any(|(x, y)| (*x as f32) < b && (*y as f32) > a);
        let played = a <= v.cursor;
        let mut c = match (gap, played) {
            (true, _) => Color::srgb(0.55, 0.18, 0.16),
            (false, true) => Color::srgb(0.70, 0.60, 0.40),
            (false, false) => Color::srgb(0.28, 0.27, 0.24),
        };
        if v.cursor >= a && v.cursor < b {
            c = INK;
        }
        if matches!(i, Some(Interaction::Hovered)) {
            c = Color::srgb(0.85, 0.80, 0.65);
        }
        bg.0 = c;
    }
    for mut t in &mut time {
        t.0 = if v.live {
            format!("{} live (delay {:.1}s)", clock(v.cursor / TICK_HZ), crate::LIVE_DELAY_TICKS / TICK_HZ)
        } else {
            format!("{} / {}  {}x", clock(v.cursor / TICK_HZ), clock(end / TICK_HZ), v.speed)
        };
    }
    for kids in &play {
        for k in kids.iter() {
            if let Ok(mut t) = texts.get_mut(k) {
                t.0 = if v.paused { "Play".into() } else { "Pause".into() };
            }
        }
    }
}

fn labels(
    feeds: Res<Feeds>,
    v: Res<Viewing>,
    cs: Res<CamState>,
    cam: Query<(&Camera, &GlobalTransform), With<MainCam>>,
    mut q: Query<(&NameLabel, &mut Node, &mut Visibility)>,
) {
    let Ok((camera, ct)) = cam.single() else { return };
    let m = &feeds.0[v.idx];
    for (l, mut n, mut vis) in &mut q {
        let hide = matches!(cs.mode, CamMode::Station(_)) && l.0 == v.tank;
        let pos = m.tank_at(l.0, v.cursor).map(|s| tank_pose(&s).translation + Vec3::Y * 4.5);
        match pos.and_then(|p| camera.world_to_viewport(ct, p).ok()) {
            Some(p) if !hide => {
                n.left = Val::Px(p.x - 30.0);
                n.top = Val::Px(p.y - 10.0);
                *vis = Visibility::Inherited;
            }
            _ => *vis = Visibility::Hidden,
        }
    }
}

fn overlays(
    cs: Res<CamState>,
    feeds: Res<Feeds>,
    v: Res<Viewing>,
    mut q: Query<(&Overlay, &mut Visibility), Without<StationNote>>,
    mut note: Query<(&mut Text, &mut Visibility), With<StationNote>>,
) {
    let st = match cs.mode {
        CamMode::Station(s) => Some(s),
        _ => None,
    };
    for (o, mut vis) in &mut q {
        *vis = if Some(o.0) == st { Visibility::Inherited } else { Visibility::Hidden };
    }
    for (mut t, mut vis) in &mut note {
        match st {
            Some(s) => {
                let m = &feeds.0[v.idx];
                t.0 = format!("{} - {} - spectator reconstruction, not the agent's view", s.label(), clean(&m.seats[v.tank].agent));
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}
