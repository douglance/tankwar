//! Spectator interface: the match directory, and the battlefield HUD shared
//! by live and replay. Every string that came from an agent or the feed is
//! escaped before display and rendered as plain text, never as markup.

use crate::camera::{CamMode, CamState, MainCam, Station};
use crate::feed::{sanitize_command_text as clean, EventKind, Match, Round, Status, CREW_ROLES, PARTS, TICK_HZ};
use crate::tanks::{side_color, tank_pose};
use crate::remote::{self, Catalog};
use crate::{Feeds, Playback, Screen, Viewing};
use bevy::prelude::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiHovered>()
            .init_resource::<DirFilter>()
            .init_resource::<TlState>()
            .init_resource::<KeyFocus>()
            .add_systems(OnEnter(Screen::Battlefield), (hud, reset_timeline))
            .add_systems(Update, (hover, focus_keys, button_tint, focus_ring, responsive).chain())
            .add_systems(
                Update,
                (
                    ensure_directory,
                    directory_typing,
                    (directory_buttons, directory_filter_buttons).after(focus_keys),
                    directory_filter,
                    directory_scroll,
                    ladder_text,
                    directory_live_badges,
                    sync_note,
                )
                    .run_if(in_state(Screen::Directory)),
            )
            .add_systems(
                Update,
                (
                    (hud_buttons, timeline_buttons).after(focus_keys).before(Playback),
                    (top_bar, chips, hud_toggles, tank_panel, timeline, transport, labels, overlays, agent_view, other_live_lines).after(Playback),
                )
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
/// Secondary text that sits straight on the battlefield backdrop.
const SOFT: Color = Color::srgb(0.84, 0.82, 0.76);
const FOCUS: Color = Color::srgb(1.0, 0.86, 0.36);
const PANEL: Color = Color::srgba(0.08, 0.08, 0.07, 0.95);
const BTN: Color = Color::srgb(0.24, 0.23, 0.20);
const BTN_HOT: Color = Color::srgb(0.36, 0.34, 0.28);
const BTN_ON: Color = Color::srgb(0.55, 0.45, 0.24);
const RED: Color = Color::srgb(0.72, 0.22, 0.18);
const AMBER: Color = Color::srgb(0.80, 0.56, 0.16);
const GREEN: Color = Color::srgb(0.30, 0.52, 0.28);
const SLATE: Color = Color::srgb(0.32, 0.38, 0.44);
const VIOLET: Color = Color::srgb(0.52, 0.26, 0.62);
const NEUTRAL: Color = Color::srgb(0.30, 0.30, 0.28);

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

fn button_tint(mut q: Query<(&Interaction, &mut BackgroundColor, &mut BorderColor, Option<&On>), With<Button>>) {
    for (i, mut bg, mut border, on) in &mut q {
        let on = on.map(|o| o.0).unwrap_or(false);
        border.0 = if on { INK } else { Color::NONE };
        bg.0 = match (i, on) {
            (_, true) => BTN_ON,
            (Interaction::Hovered | Interaction::Pressed, _) => BTN_HOT,
            _ => BTN,
        };
    }
}

/// Keyboard focus: Tab and Shift+Tab walk every visible button in reading
/// order, Enter presses the focused one, and a bright ring marks it.
#[derive(Resource, Default)]
pub struct KeyFocus {
    pub entity: Option<Entity>,
    release: Option<Entity>,
}

fn focus_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut focus: ResMut<KeyFocus>,
    buttons: Query<(Entity, &ComputedNode, &GlobalTransform, &InheritedVisibility), With<Button>>,
    mut inter: Query<&mut Interaction>,
) {
    if let Some(e) = focus.release.take() {
        if let Ok(mut i) = inter.get_mut(e) {
            if *i == Interaction::Pressed {
                *i = Interaction::None;
            }
        }
    }
    let mut list: Vec<(Entity, f32, f32)> = buttons
        .iter()
        .filter(|(_, n, _, v)| v.get() && n.size().x > 0.0 && n.size().y > 0.0)
        .map(|(e, _, t, _)| (e, (t.translation().y / 8.0).round(), t.translation().x))
        .collect();
    list.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)));
    if focus.entity.is_some_and(|f| !list.iter().any(|(e, ..)| *e == f)) {
        focus.entity = None;
    }
    if keys.just_pressed(KeyCode::Tab) && !list.is_empty() {
        let back = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let n = list.len();
        let at = focus.entity.and_then(|f| list.iter().position(|(e, ..)| *e == f));
        let next = match (at, back) {
            (None, false) => 0,
            (None, true) => n - 1,
            (Some(i), false) => (i + 1) % n,
            (Some(i), true) => (i + n - 1) % n,
        };
        focus.entity = Some(list[next].0);
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        if let Some(e) = focus.entity {
            if let Ok(mut i) = inter.get_mut(e) {
                *i = Interaction::Pressed;
                focus.release = Some(e);
            }
        }
    }
}

fn focus_ring(mut commands: Commands, focus: Res<KeyFocus>, rings: Query<Entity, With<Outline>>, buttons: Query<(), With<Button>>) {
    for e in &rings {
        if Some(e) != focus.entity && buttons.contains(e) {
            commands.entity(e).remove::<Outline>();
        }
    }
    if let Some(e) = focus.entity {
        if !rings.contains(e) {
            if let Ok(mut ec) = commands.get_entity(e) {
                ec.insert(Outline { width: Val::Px(3.0), offset: Val::Px(2.0), color: FOCUS });
            }
        }
    }
}

fn button(label: impl Into<String>, size: f32) -> impl Bundle {
    (
        Button,
        Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)), margin: UiRect::all(Val::Px(2.0)), border: UiRect::bottom(Val::Px(3.0)), ..default() },
        BackgroundColor(BTN),
        BorderColor(Color::NONE),
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

#[derive(Component)]
struct DirNote;

fn ensure_directory(mut commands: Commands, feeds: Res<Feeds>, catalog: Res<Catalog>, roots: Query<Entity, With<DirRoot>>, mut shown: Local<Option<u32>>) {
    let missing = roots.is_empty();
    let stale = shown.is_none_or(|g| g != catalog.generation);
    if !missing && !stale {
        return;
    }
    for e in &roots {
        commands.entity(e).despawn();
    }
    *shown = Some(catalog.generation);
    directory(&mut commands, &feeds, &catalog);
}

fn sync_note(catalog: Res<Catalog>, mut notes: Query<&mut Text, With<DirNote>>) {
    for mut t in &mut notes {
        if t.0 != catalog.note {
            t.0.clone_from(&catalog.note);
        }
    }
}

fn directory(commands: &mut Commands, feeds: &Feeds, catalog: &Catalog) {
    let root = commands
        .spawn((
            StateScoped(Screen::Directory),
            DirRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::new(Val::Px(36.0), Val::Px(36.0), Val::Px(20.0), Val::Px(20.0)),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition::default(),
        ))
        .id();
    commands.entity(root).with_children(|r| {
        r.spawn((Text::new("TANKWAR"), font(40.0), TextColor(INK)));
        r.spawn((
            Text::new("Spectator - match directory"),
            font(20.0),
            TextColor(SOFT),
            Node { margin: UiRect::bottom(Val::Px(10.0)), padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), align_self: AlignSelf::FlexStart, flex_shrink: 0.0, ..default() },
            BackgroundColor(PANEL),
        ));
        r.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_items: AlignItems::FlexStart, column_gap: Val::Px(24.0), flex_shrink: 0.0, ..default() }).with_children(|r| {
        r.spawn(Node { flex_direction: FlexDirection::Column, width: Val::Percent(100.0), max_width: Val::Px(560.0), flex_shrink: 0.0, ..default() }).with_children(|r| {
        r.spawn((
            Node { flex_direction: FlexDirection::Column, width: Val::Percent(100.0), padding: UiRect::all(Val::Px(8.0)), margin: UiRect::bottom(Val::Px(8.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(PANEL),
            BorderRadius::all(Val::Px(4.0)),
        ))
        .with_children(|f| {
            f.spawn((
                SearchText,
                Text::new(""),
                font(16.0),
                TextColor(INK),
                Node { width: Val::Percent(100.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)), ..default() },
                BorderColor(DIM),
                BackgroundColor(Color::srgb(0.05, 0.05, 0.045)),
            ));
            f.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_items: AlignItems::Center, margin: UiRect::top(Val::Px(6.0)), ..default() }).with_children(|row| {
                row.spawn((Text::new("Show"), font(14.0), TextColor(SOFT), Node { margin: UiRect::right(Val::Px(6.0)), ..default() }));
                for st in [None, Some(Status::Live), Some(Status::Finished), Some(Status::Aborted)] {
                    let l = match st { None => "All", Some(Status::Live) => "Live", Some(Status::Finished) => "Result", Some(Status::Aborted) => "Aborted" };
                    row.spawn((button(l, 14.0), StatusBtn(st), On(st.is_none())));
                }
                row.spawn((FilterCount, Text::new(""), font(14.0), TextColor(SOFT), Node { margin: UiRect::left(Val::Px(8.0)), ..default() }));
            });
        });
        for (idx, m) in feeds.0.iter().enumerate() {
            let (chip, col) = match m.status {
                Status::Live => ("LIVE", RED),
                Status::Finished => ("RESULT", GREEN),
                Status::Aborted => ("ABORTED", SLATE),
            };
            r.spawn((
                Interaction::default(),
                Card(idx),
                Node {
                    width: Val::Percent(100.0),
                    max_width: Val::Px(560.0),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    margin: UiRect::bottom(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderRadius::all(Val::Px(4.0)),
            ))
            .with_children(|c| {
                c.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_items: AlignItems::Center, ..default() }).with_children(|row| {
                    row.spawn((
                        Node { padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)), margin: UiRect::right(Val::Px(10.0)), align_items: AlignItems::Center, ..default() },
                        BackgroundColor(col),
                    ))
                    .with_children(|b| {
                        status_icon(b, m.status.icon());
                        b.spawn((Text::new(chip), font(13.0), TextColor(INK)));
                    });
                    if m.status == Status::Live {
                        // Feed health at the live head: lag and disconnects.
                        for (kind, col) in [(DirBadge::Lag, AMBER), (DirBadge::Disconnect, VIOLET)] {
                            row.spawn((
                                kind,
                                Node { padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)), margin: UiRect::right(Val::Px(8.0)), align_items: AlignItems::Center, display: Display::None, ..default() },
                                BackgroundColor(col),
                            ))
                            .with_children(|b| {
                                status_icon(b, if kind == DirBadge::Lag { Icon::Lag } else { Icon::Disconnect });
                                b.spawn((Text::new(""), font(13.0), TextColor(INK)));
                            });
                        }
                    }
                    row.spawn((Text::new(format!("{}  {}  {}", clean(&m.id), m.mode, m.map)), font(17.0), TextColor(INK)));
                });
                let west: Vec<String> = m.seats.iter().filter(|s| s.side == crate::feed::Side::West).map(|s| format!("{} ({})", clean(&s.agent), s.vehicle.label())).collect();
                let east: Vec<String> = m.seats.iter().filter(|s| s.side == crate::feed::Side::East).map(|s| format!("{} ({})", clean(&s.agent), s.vehicle.label())).collect();
                c.spawn((Text::new(format!("West: {}\nEast: {}", west.join(", "), east.join(", "))), font(14.0), TextColor(SOFT), Node { margin: UiRect::vertical(Val::Px(4.0)), ..default() }));
                let outcome = match m.status {
                    Status::Live => format!("In progress - {} recorded", clock(m.duration_s())),
                    _ => format!("{} - {}", clock(m.duration_s()), clean(m.result_text().unwrap_or("no result"))),
                };
                c.spawn((Text::new(outcome), font(14.0), TextColor(SOFT)));
                if let Some(line) = (m.status != Status::Live).then(|| last_rejected_order(m)).flatten() {
                    c.spawn((Text::new(line), font(13.0), TextColor(AMBER)));
                }
                c.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, flex_shrink: 0.0, margin: UiRect::top(Val::Px(4.0)), ..default() }).with_children(|row| {
                    if m.status == Status::Live {
                        row.spawn((button("Watch live", 15.0), Watch { idx, live: true }));
                    }
                    row.spawn((button("Replay", 15.0), Watch { idx, live: false }));
                });
            });
        }
        });
        r.spawn(Node { flex_direction: FlexDirection::Column, width: Val::Percent(100.0), max_width: Val::Px(380.0), flex_shrink: 0.0, ..default() }).with_children(|r| {
            ladder_panel(r);
            r.spawn((
                DirNote,
                Text::new(catalog.note.clone()),
                font(15.0),
                TextColor(SOFT),
                Node { padding: UiRect::all(Val::Px(10.0)), width: Val::Percent(100.0), flex_shrink: 0.0, ..default() },
                BackgroundColor(PANEL),
                BorderRadius::all(Val::Px(4.0)),
            ));
        });
        });
    });
}

#[derive(Component)]
struct LadderText(usize);

/// The public season ladder, 1v1 and 2v2 kept apart.
fn ladder_panel(r: &mut ChildSpawnerCommands) {
    r.spawn((
        Node {
            width: Val::Percent(100.0),
            max_width: Val::Px(380.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(14.0)),
            margin: UiRect::bottom(Val::Px(12.0)),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(PANEL),
        BorderRadius::all(Val::Px(4.0)),
    ))
    .with_children(|c| {
        c.spawn((Text::new("Public ladder"), font(20.0), TextColor(INK)));
        c.spawn((Text::new("Ratings as published by the server. Nothing here is computed by the viewer."), font(13.0), TextColor(SOFT), Node { margin: UiRect::bottom(Val::Px(8.0)), ..default() }));
        for (i, mode) in crate::ladder::MODES.iter().enumerate() {
            c.spawn((Text::new(mode.to_uppercase()), font(16.0), TextColor(INK), Node { margin: UiRect::top(Val::Px(8.0)), ..default() }));
            c.spawn((Text::new("Loading..."), font(14.0), TextColor(SOFT), LadderText(i)));
        }
    });
}

/// Board text for one mode. Agent IDs are escaped like every feed string.
pub fn board_text(b: &crate::ladder::Board) -> String {
    use crate::ladder::Board;
    match b {
        Board::Loading => "Loading...".into(),
        Board::Unavailable => "Ladder unavailable: the leaderboard could not be read.".into(),
        Board::Ready { entries, .. } if entries.is_empty() => "No ranked entries yet.".into(),
        Board::Ready { season, entries } => {
            let mut s = format!("{}\n", clean(season));
            for (n, e) in entries.iter().take(10).enumerate() {
                let id: String = clean(&e.agent_id).chars().take(24).collect();
                s.push_str(&format!("{:>2}. {}  {}  {} games{}\n", n + 1, id, e.rating, e.games, if e.provisional { "  provisional" } else { "" }));
            }
            if entries.len() > 10 {
                s.push_str(&format!("... and {} more", entries.len() - 10));
            }
            s.trim_end().to_string()
        }
    }
}

fn ladder_text(ladder: Res<crate::ladder::Ladder>, mut q: Query<(&LadderText, &mut Text)>) {
    for (lt, mut t) in &mut q {
        let want = board_text(&ladder.get(lt.0));
        if t.0 != want {
            t.0 = want;
        }
    }
}

/// The match list scrolls with the wheel or a one-finger drag, so every
/// match stays reachable on a short or narrow screen.
fn directory_scroll(
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    touches: Res<Touches>,
    mut root: Query<(&mut ScrollPosition, &ComputedNode), With<DirRoot>>,
) {
    let Ok((mut pos, node)) = root.single_mut() else { return };
    let mut dy = -wheel.delta.y * if wheel.unit == bevy::input::mouse::MouseScrollUnit::Line { 40.0 } else { 1.0 };
    for t in touches.iter() {
        dy -= t.delta().y;
    }
    let max = (node.content_size().y - node.size().y).max(0.0) * node.inverse_scale_factor();
    pos.offset_y = (pos.offset_y + dy).clamp(0.0, max);
}

#[derive(Component)]
struct Card(usize);
#[derive(Component)]
struct SearchText;
#[derive(Component)]
struct FilterCount;
#[derive(Component)]
struct StatusBtn(Option<Status>);

/// What the spectator typed into the match search and which status they picked.
#[derive(Resource, Default)]
pub struct DirFilter {
    pub text: String,
    pub status: Option<Status>,
}

/// Does a match pass the directory filter? Text matches the match id, mode,
/// map, any agent name, or the status word, ignoring case.
pub fn matches_filter(m: &Match, f: &DirFilter) -> bool {
    if f.status.is_some_and(|s| s != m.status) {
        return false;
    }
    let q = f.text.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let status = match m.status {
        Status::Live => "live",
        Status::Finished => "result finished",
        Status::Aborted => "aborted",
    };
    let mut hay = format!("{} {} {} {}", m.id, m.mode, m.map, status);
    for s in &m.seats {
        hay.push(' ');
        hay.push_str(&s.agent);
    }
    hay.to_lowercase().contains(&q)
}

/// Typing on the directory goes to the search box.
fn directory_typing(mut keys: EventReader<bevy::input::keyboard::KeyboardInput>, mut f: ResMut<DirFilter>) {
    use bevy::input::keyboard::Key;
    for k in keys.read() {
        if !k.state.is_pressed() {
            continue;
        }
        match &k.logical_key {
            Key::Character(c) => {
                for ch in c.chars().filter(|c| !c.is_control()) {
                    if f.text.chars().count() < 40 {
                        f.text.push(ch);
                    }
                }
            }
            Key::Space if !f.text.is_empty() => f.text.push(' '),
            Key::Backspace => {
                f.text.pop();
            }
            Key::Escape => f.text.clear(),
            _ => {}
        }
    }
}

fn directory_filter_buttons(q: Query<(&Interaction, &StatusBtn), Changed<Interaction>>, mut f: ResMut<DirFilter>) {
    for (i, b) in &q {
        if *i == Interaction::Pressed {
            f.status = b.0;
        }
    }
}

#[allow(clippy::type_complexity)]
fn directory_filter(
    f: Res<DirFilter>,
    feeds: Res<Feeds>,
    time: Res<Time>,
    mut cards: Query<(&Card, &mut Node)>,
    mut texts: ParamSet<(Query<&mut Text, With<SearchText>>, Query<&mut Text, With<FilterCount>>)>,
    mut btns: Query<(&StatusBtn, &mut On)>,
) {
    let mut shown = 0;
    for (c, mut n) in &mut cards {
        let ok = matches_filter(&feeds.0[c.0], &f);
        shown += ok as usize;
        let want = if ok { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
    }
    let caret = if (time.elapsed_secs() * 2.0) as u32 % 2 == 0 { "|" } else { " " };
    let search = if f.text.is_empty() { format!("Search matches or agents: type to filter{caret}") } else { format!("Search: {}{caret}", clean(&f.text)) };
    for mut t in &mut texts.p0() {
        t.0.clone_from(&search);
    }
    for mut t in &mut texts.p1() {
        t.0 = format!("{shown} of {} matches", feeds.0.len());
    }
    for (b, mut on) in &mut btns {
        on.0 = b.0 == f.status;
    }
}

fn directory_buttons(
    q: Query<(&Interaction, &Watch), Changed<Interaction>>,
    feeds: Res<Feeds>,
    clock: Res<crate::LiveClock>,
    bridge: Res<remote::Bridge>,
    mut catalog: ResMut<Catalog>,
    mut v: ResMut<Viewing>,
    mut cam: ResMut<CamState>,
    mut next: ResMut<NextState<Screen>>,
) {
    for (i, w) in &q {
        if *i == Interaction::Pressed {
            let m = &feeds.0[w.idx];
            if !remote::prepare_watch(&bridge, w.idx, &m.id, m.has_dense_track(), &mut catalog.note) {
                continue;
            }
            // Server recordings play from the first tick. The demo live clock
            // starts mid-feed and would skip a village fight.
            let live = w.live && m.demo;
            *v = Viewing::open(w.idx, live, m);
            if live {
                v.set_wall(m, clock.0);
            }
            cam.focus = Vec3::ZERO;
            next.set(Screen::Battlefield);
        }
    }
}

/// Status icons drawn from plain shapes (the UI font has no symbol glyphs).
/// Each one always sits beside its text label, never alone.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Icon {
    /// Solid dot: broadcasting now.
    Live,
    /// Two bars: feed stalled.
    Lag,
    /// Broken link: an agent's connection dropped.
    Disconnect,
    /// Flag: the match finished with a result.
    Result,
    /// Stop square: the match was aborted.
    Aborted,
}

impl Status {
    fn icon(self) -> Icon {
        match self {
            Status::Live => Icon::Live,
            Status::Finished => Icon::Result,
            Status::Aborted => Icon::Aborted,
        }
    }
}

fn status_icon(p: &mut ChildSpawnerCommands, icon: Icon) {
    let px = Val::Px;
    let bar = |w: f32, h: f32| (Node { width: px(w), height: px(h), ..default() }, BackgroundColor(INK));
    p.spawn(Node { width: px(12.0), height: px(12.0), margin: UiRect::right(px(5.0)), flex_shrink: 0.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: px(2.0), ..default() })
        .with_children(|i| match icon {
            Icon::Live => {
                i.spawn((Node { width: px(10.0), height: px(10.0), ..default() }, BackgroundColor(INK), BorderRadius::all(px(5.0))));
            }
            Icon::Lag => {
                i.spawn(bar(3.0, 10.0));
                i.spawn(bar(3.0, 10.0));
            }
            Icon::Disconnect => {
                i.spawn(bar(4.0, 3.0));
                i.spawn((Node { width: px(1.0), height: px(10.0), ..default() }, BackgroundColor(INK.with_alpha(0.5))));
                i.spawn(bar(4.0, 3.0));
            }
            Icon::Result => {
                i.spawn(Node { flex_direction: FlexDirection::Row, width: px(10.0), height: px(12.0), ..default() }).with_children(|f| {
                    f.spawn(bar(2.0, 12.0));
                    f.spawn(bar(7.0, 6.0));
                });
            }
            Icon::Aborted => {
                i.spawn(bar(9.0, 9.0));
            }
        });
}

#[derive(Component, Clone, Copy, PartialEq)]
enum DirBadge {
    Lag,
    Disconnect,
}

/// Directory badges for the live match, read at the delivered live head.
fn directory_live_badges(
    feeds: Res<Feeds>,
    clock: Res<crate::LiveClock>,
    cards: Query<(&Card, &Children)>,
    rows: Query<&Children>,
    mut badges: Query<(&DirBadge, &mut Node, &Children)>,
    mut texts: Query<&mut Text>,
) {
    for (card, kids) in &cards {
        let m = &feeds.0[card.0];
        if m.status != Status::Live {
            continue;
        }
        let (head, lag) = crate::live_head_s(m, clock.0);
        let n = now(m, (head * TICK_HZ - crate::LIVE_DELAY_TICKS).clamp(0.0, m.end_tick as f32));
        let names: Vec<String> = n.disconnected.iter().map(|i| clean(&m.seats[*i].agent)).collect();
        for row in kids.iter() {
            let Ok(row_kids) = rows.get(row) else { continue };
            for b in row_kids.iter() {
                let Ok((kind, mut node, bk)) = badges.get_mut(b) else { continue };
                let label = match kind {
                    DirBadge::Lag => lag.filter(|l| *l > 0.5).map(|l| format!("LAG {l:.1}s")),
                    DirBadge::Disconnect => (!names.is_empty()).then(|| format!("DISCONNECTED: {}", names.join(", "))),
                };
                node.display = if label.is_some() { Display::Flex } else { Display::None };
                if let (Some(l), Some(t)) = (label, bk.last()) {
                    if let Ok(mut t) = texts.get_mut(*t) {
                        t.0 = l;
                    }
                }
            }
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
struct TransportBar;

#[derive(Component)]
struct DirRoot;
#[derive(Component)]
struct TopBar;
#[derive(Component)]
struct TankPanel;
#[derive(Component)]
struct TimelinePanel;
#[derive(Component)]
struct SeekRow;

/// Below this logical width the HUD switches to the phone layout.
pub const NARROW_PX: f32 = 760.0;

/// Phone layout: tank panel and timeline split the width under the top bar
/// and are capped in height so the battlefield stays visible; the transport
/// spans the width and the seek bar takes its own row.
#[allow(clippy::type_complexity)]
fn responsive(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    top: Query<&ComputedNode, With<TopBar>>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<DirRoot>>,
        Query<&mut Node, With<TankPanel>>,
        Query<&mut Node, With<TimelinePanel>>,
        Query<&mut Node, With<TransportBar>>,
        Query<&mut Node, With<SeekRow>>,
    )>,
) {
    let Ok(w) = windows.single() else { return };
    let narrow = w.width() < NARROW_PX;
    let below_bar = top.single().map(|c| c.size().y * c.inverse_scale_factor() + 6.0).unwrap_or(56.0).max(56.0);
    for mut n in &mut nodes.p0() {
        n.padding = UiRect::all(Val::Px(if narrow { 12.0 } else { 36.0 }));
    }
    for mut n in &mut nodes.p1() {
        n.top = Val::Px(below_bar);
        if narrow {
            n.left = Val::Percent(1.5);
            n.width = Val::Percent(47.5);
            n.max_height = Val::Auto;
            n.overflow = Overflow::visible();
            n.padding = UiRect::all(Val::Px(6.0));
        } else {
            n.left = Val::Px(10.0);
            n.width = Val::Px(270.0);
            n.max_height = Val::Auto;
            n.overflow = Overflow::visible();
            n.padding = UiRect::all(Val::Px(10.0));
        }
    }
    for mut n in &mut nodes.p2() {
        n.top = Val::Px(below_bar);
        if narrow {
            n.right = Val::Percent(1.5);
            n.width = Val::Percent(47.5);
            n.max_height = Val::Percent(55.0);
            n.padding = UiRect::all(Val::Px(6.0));
        } else {
            n.right = Val::Px(10.0);
            n.width = Val::Px(360.0);
            n.max_height = Val::Percent(72.0);
            n.padding = UiRect::all(Val::Px(10.0));
        }
    }
    for mut n in &mut nodes.p3() {
        if narrow {
            n.left = Val::Percent(2.0);
            n.width = Val::Percent(96.0);
        } else {
            n.left = Val::Percent(18.0);
            n.width = Val::Percent(64.0);
        }
    }
    for mut n in &mut nodes.p4() {
        n.flex_basis = if narrow { Val::Percent(100.0) } else { Val::Auto };
        n.margin = if narrow { UiRect::vertical(Val::Px(6.0)) } else { UiRect::horizontal(Val::Px(8.0)) };
    }
}
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
#[derive(Component)]
struct AgentView;
#[derive(Component)]
struct AgentTitle;
#[derive(Component)]
struct AgentReport;
/// One contact mark on the agent's sight plot, by report order.
#[derive(Component)]
struct AgentBlip(usize);

/// Size of the agent's sight plot: bearing across, range up.
const SIGHT_W: f32 = 600.0;
const SIGHT_H: f32 = 330.0;
#[derive(Component)]
struct OtherMatches;

/// The text of one row in the Other matches strip, by match index.
#[derive(Component)]
struct OtherLine(usize);

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
    // Agent's view: the scene is replaced by the report the seat is given.
    commands.spawn((scoped.clone(), full.clone(), AgentView, Visibility::Hidden, BackgroundColor(Color::srgb(0.045, 0.05, 0.045)))).with_children(|o| {
        o.spawn(Node { position_type: PositionType::Absolute, left: Val::Px(300.0), right: Val::Px(380.0), top: Val::Px(88.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() })
            .with_children(|c| {
                c.spawn((AgentTitle, Text::new(""), font(18.0), TextColor(INK), Node { margin: UiRect::bottom(Val::Px(6.0)), ..default() }));
                c.spawn((
                    Text::new("The agent gets no picture. Each turn it is told only what its forward sight picks up: contacts within 30 deg either side of the hull's nose, out to 1000 m, unless a standing building, a wall or another tank is in the way. Bearings come to the nearest 5 deg and ranges to the nearest 50 m. It is never told which tank it is, or whose side."),
                    font(13.0),
                    TextColor(SOFT),
                    Node { max_width: Val::Px(SIGHT_W + 40.0), margin: UiRect::bottom(Val::Px(10.0)), ..default() },
                ));
                // The plot: bearing left to right, range bottom to top.
                c.spawn((
                    Node { width: Val::Px(SIGHT_W), height: Val::Px(SIGHT_H), border: UiRect::all(Val::Px(1.0)), margin: UiRect::top(Val::Px(16.0)), ..default() },
                    BorderColor(DIM),
                    BackgroundColor(Color::srgb(0.07, 0.09, 0.07)),
                ))
                .with_children(|g| {
                    let line = Color::srgba(0.66, 0.64, 0.58, 0.28);
                    for k in 1..6 {
                        let x = k as f32 / 6.0 * 100.0;
                        g.spawn((Node { position_type: PositionType::Absolute, left: Val::Percent(x), top: Val::Px(0.0), width: Val::Px(1.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(if k == 3 { DIM } else { line })));
                    }
                    for k in 1..5 {
                        let y = k as f32 / 5.0 * 100.0;
                        g.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Percent(y), width: Val::Percent(100.0), height: Val::Px(1.0), ..default() }, BackgroundColor(line)));
                        g.spawn((Text::new(format!("{} m", 1000 - k * 200)), font(12.0), TextColor(DIM), Node { position_type: PositionType::Absolute, left: Val::Px(4.0), top: Val::Percent(y - 5.5), ..default() }));
                    }
                    g.spawn((Text::new("1000 m"), font(12.0), TextColor(DIM), Node { position_type: PositionType::Absolute, left: Val::Px(4.0), top: Val::Px(2.0), ..default() }));
                    for (txt, left) in [("30 deg left", Some(0.0)), ("ahead", None), ("30 deg right", Some(100.0))] {
                        let mut n = Node { position_type: PositionType::Absolute, top: Val::Px(-18.0), ..default() };
                        match left {
                            Some(l) if l == 0.0 => n.left = Val::Px(0.0),
                            Some(_) => n.right = Val::Px(0.0),
                            None => n.left = Val::Percent(47.0),
                        }
                        g.spawn((Text::new(txt), font(12.0), TextColor(DIM), n));
                    }
                    // Own tank at the bottom centre.
                    g.spawn((Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), bottom: Val::Px(-6.0), width: Val::Px(12.0), height: Val::Px(12.0), margin: UiRect::left(Val::Px(-6.0)), ..default() }, BackgroundColor(INK)));
                    for i in 0..m.seats.len() {
                        g.spawn((AgentBlip(i), Visibility::Hidden, Node { position_type: PositionType::Absolute, width: Val::Px(12.0), height: Val::Px(12.0), margin: UiRect::new(Val::Px(-6.0), Val::Px(0.0), Val::Px(-6.0), Val::Px(0.0)), ..default() }, BackgroundColor(FOCUS)))
                            .with_children(|b| {
                                b.spawn((Text::new(""), font(12.0), TextColor(FOCUS), Node { position_type: PositionType::Absolute, left: Val::Px(16.0), top: Val::Px(-3.0), width: Val::Px(150.0), ..default() }));
                            });
                    }
                });
                c.spawn((Text::new("own tank"), font(12.0), TextColor(DIM), Node { margin: UiRect::top(Val::Px(8.0)), ..default() }));
                c.spawn((AgentReport, Text::new(""), font(14.0), TextColor(INK), Node { width: Val::Px(SIGHT_W), margin: UiRect::top(Val::Px(10.0)), padding: UiRect::all(Val::Px(8.0)), ..default() }, BackgroundColor(PANEL)));
                c.spawn((
                    Text::new("Rebuilt by this viewer from the replay with the server's sight rule. The replay does not record the reports the agent actually received."),
                    font(12.0),
                    TextColor(DIM),
                    Node { max_width: Val::Px(SIGHT_W), margin: UiRect::top(Val::Px(8.0)), ..default() },
                ));
            });
    });
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
            TopBar,
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
                        Chip::Live => RED,
                        Chip::Disconnect => VIOLET,
                        Chip::Lag | Chip::Recovery | Chip::Missing => AMBER,
                        Chip::Result => GREEN,
                        Chip::Aborted => SLATE,
                        _ => NEUTRAL,
                    };
                    bar.spawn((c, Node { padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)), margin: UiRect::right(Val::Px(6.0)), align_items: AlignItems::Center, display: Display::None, ..default() }, BackgroundColor(bg)))
                        .with_children(|ch| {
                            let icon = match c {
                                Chip::Live => Some(Icon::Live),
                                Chip::Lag => Some(Icon::Lag),
                                Chip::Disconnect => Some(Icon::Disconnect),
                                Chip::Result => Some(Icon::Result),
                                Chip::Aborted => Some(Icon::Aborted),
                                _ => None,
                            };
                            if let Some(i) = icon {
                                status_icon(ch, i);
                            }
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
            TankPanel,
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
            TimelinePanel,
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
            p.spawn((Text::new("Command timeline (newest first, server ticks)"), font(14.0), TextColor(SOFT)));
            p.spawn(Node { flex_wrap: FlexWrap::Wrap, margin: UiRect::vertical(Val::Px(4.0)), flex_shrink: 0.0, ..default() }).with_children(|r| {
                for f in TlFilter::ALL {
                    r.spawn((button(f.label(), 12.0), TlFilterBtn(f), On(f == TlFilter::All)));
                }
            });
            p.spawn((
                TlDetail,
                Text::new(""),
                font(12.5),
                TextColor(INK),
                Node { padding: UiRect::all(Val::Px(6.0)), margin: UiRect::bottom(Val::Px(4.0)), border: UiRect::left(Val::Px(3.0)), flex_shrink: 0.0, ..default() },
                BorderColor(FOCUS),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            ));
            if !v.live {
                p.spawn((button("Seek to selected tick", 12.0), TlSeek)).insert(Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)), margin: UiRect::bottom(Val::Px(4.0)), align_self: AlignSelf::FlexStart, display: Display::None, ..default() });
            }
            for i in 0..TL_ROWS {
                p.spawn((
                    Button,
                    TlRow(i),
                    On(false),
                    Node { width: Val::Percent(100.0), padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)), margin: UiRect::bottom(Val::Px(1.0)), flex_shrink: 0.0, display: Display::None, ..default() },
                    BackgroundColor(BTN),
                    children![(TlRowText(i), Text::new(""), font(12.5), TextColor(INK), TextLayout::default(), Node { max_width: Val::Percent(100.0), ..default() })],
                ));
            }
        });

    // Bottom left: the other matches' status, so a final result and an
    // aborted match read beside this one. Hidden on narrow screens.
    if feeds.0.len() > 1 {
        commands
            .spawn((
                scoped.clone(),
                OtherMatches,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(10.0),
                    bottom: Val::Px(10.0),
                    width: Val::Px(270.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(8.0)),
                    row_gap: Val::Px(5.0),
                    ..default()
                },
                BackgroundColor(PANEL),
            ))
            .with_children(|p| {
                p.spawn((Text::new("Other matches"), font(13.0), TextColor(SOFT)));
                for (i, o) in feeds.0.iter().enumerate().filter(|(i, _)| *i != v.idx) {
                    p.spawn(Node { align_items: AlignItems::FlexStart, ..default() }).with_children(|r| {
                        status_icon(r, o.status.icon());
                        r.spawn((OtherLine(i), Text::new(other_match_line(o, None, &[])), font(12.5), TextColor(INK)));
                    });
                }
            });
    }

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
                flex_wrap: FlexWrap::Wrap,
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
            b.spawn((SeekRow, Node { flex_grow: 1.0, height: Val::Px(16.0), margin: UiRect::horizontal(Val::Px(8.0)), ..default() })).with_children(|bar| {
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

fn top_bar(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    feeds: Res<Feeds>,
    v: Res<Viewing>,
    cam: Res<CamState>,
    mut q: Query<&mut Text, With<TopInfo>>,
    mut others: Query<&mut Node, With<OtherMatches>>,
) {
    let m = &feeds.0[v.idx];
    let view = match cam.mode {
        CamMode::Orbit => "Camera: Orbit".to_string(),
        mode => format!("Camera: {} - {}", mode.label(), clean(&m.seats[v.tank].agent)),
    };
    for mut t in &mut q {
        t.0 = format!("{}  {}  {}   {view}", clean(&m.id), m.mode, m.map);
    }
    let wide = windows.single().is_ok_and(|w| w.width() >= NARROW_PX * 1.6);
    for mut n in &mut others {
        n.display = if wide { Display::Flex } else { Display::None };
    }
}

/// One line for another match in the battlefield's side strip: its status
/// and, once over, the server's outcome and any interval with no data.
fn other_match_line(m: &Match, lag: Option<f32>, disconnected: &[String]) -> String {
    let mut out = format!("{} {}", clean(&m.id), m.mode);
    if m.status == Status::Live {
        // A live match shows only what has been delivered, never its future.
        out += "\nLIVE";
        if let Some(l) = lag.filter(|l| *l > 0.5) {
            out += &format!("\nLAG {l:.1}s - feed stalled");
        }
        if !disconnected.is_empty() {
            out += &format!("\nDISCONNECTED: {}", disconnected.join(", "));
        }
        return out;
    }
    for e in &m.events {
        match &e.kind {
            EventKind::Result { text } => out += &format!("\nRESULT - {}", clean(text)),
            EventKind::Aborted { reason } => out += &format!("\nABORTED - {}", clean(reason)),
            _ => {}
        }
    }
    for (a, b) in m.missing_intervals() {
        out += &format!("\nno data {}-{}", clock(a as f32 / TICK_HZ), clock(b as f32 / TICK_HZ));
    }
    out
}

/// Keeps the live match's row in the Other matches strip current: lag and
/// disconnects are read at the delivered live head, as on the directory.
fn other_live_lines(feeds: Res<Feeds>, clock: Res<crate::LiveClock>, mut q: Query<(&OtherLine, &mut Text)>) {
    for (l, mut t) in &mut q {
        let m = &feeds.0[l.0];
        if m.status != Status::Live {
            continue;
        }
        let (head, lag) = crate::live_head_s(m, clock.0);
        let n = now(m, (head * TICK_HZ - crate::LIVE_DELAY_TICKS).clamp(0.0, m.end_tick as f32));
        let names: Vec<String> = n.disconnected.iter().map(|i| clean(&m.seats[*i].agent)).collect();
        let line = other_match_line(m, lag, &names);
        if t.0 != line {
            t.0 = line;
        }
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

/// Highlight the tank, camera and speed buttons that match what is shown.
/// Only those: the timeline's filter and row highlights are its own.
#[allow(clippy::type_complexity)]
fn hud_toggles(
    v: Res<Viewing>,
    cam: Res<CamState>,
    mut toggles: Query<(&mut On, Option<&TankBtn>, Option<&CamBtn>, Option<&SpeedBtn>), Or<(With<TankBtn>, With<CamBtn>, With<SpeedBtn>)>>,
) {
    for (mut on, t, c, s) in &mut toggles {
        on.0 = t.map(|t| t.0 == v.tank).unwrap_or(false) || c.map(|c| c.0 == cam.mode).unwrap_or(false) || s.map(|s| (s.0 - v.speed).abs() < 1e-3).unwrap_or(false);
    }
}

fn tank_panel(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    feeds: Res<Feeds>,
    v: Res<Viewing>,
    mut text: Query<&mut Text, With<PanelText>>,
    mut fuel: Query<(&mut Node, &mut BackgroundColor), With<FuelFill>>,
) {
    let m = &feeds.0[v.idx];
    let seat = &m.seats[v.tank];
    let Some(s) = m.tank_at(v.tank, v.cursor) else { return };
    let cap = seat.vehicle.demo_fuel_capacity_l();
    let frac = (s.fuel_l / cap).clamp(0.0, 1.0);
    for (mut n, mut bg) in &mut fuel {
        n.width = Val::Percent(frac * 100.0);
        bg.0 = if frac < 0.2 { RED } else { AMBER };
    }
    if windows.single().is_ok_and(|w| w.width() < NARROW_PX) {
        let out = compact_panel(seat, &s, cap);
        for mut t in &mut text {
            t.0.clone_from(&out);
        }
        return;
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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TlFilter {
    #[default]
    All,
    Commands,
    Combat,
    System,
}

impl TlFilter {
    const ALL: [TlFilter; 4] = [TlFilter::All, TlFilter::Commands, TlFilter::Combat, TlFilter::System];
    fn label(self) -> &'static str {
        match self {
            TlFilter::All => "All",
            TlFilter::Commands => "Commands",
            TlFilter::Combat => "Shots & hits",
            TlFilter::System => "System",
        }
    }
}

/// Timeline filter and the selected event, held by feed sequence number so
/// the selection survives new events arriving above it.
#[derive(Resource, Default)]
pub struct TlState {
    pub filter: TlFilter,
    pub selected: Option<u64>,
    rows: Vec<u64>,
}

const TL_ROWS: usize = 12;

#[derive(Component)]
struct TlRow(usize);
#[derive(Component)]
struct TlRowText(usize);
#[derive(Component)]
struct TlFilterBtn(TlFilter);
#[derive(Component)]
struct TlDetail;
#[derive(Component)]
struct TlSeek;

fn reset_timeline(mut tl: ResMut<TlState>) {
    *tl = TlState::default();
}

/// One timeline line for an event, or nothing for events the timeline skips.
fn event_cmd(e: &crate::feed::Event) -> Option<(u32, usize)> {
    match &e.kind {
        EventKind::CommandSubmitted { cmd, tank, .. } | EventKind::CommandDecision { cmd, tank, .. } | EventKind::CommandPhase { cmd, tank, .. } => Some((*cmd, *tank)),
        _ => None,
    }
}

/// One directory line for a finished match: its latest rejected order, with
/// the order text escaped and cut to fit one line. The reason is in the
/// replay's command trace.
/// Cards whose feed never delivered the order text show no line.
fn last_rejected_order(m: &Match) -> Option<String> {
    let rej = m.events.iter().rev().find(|e| matches!(e.kind, EventKind::CommandDecision { accepted: false, .. }))?;
    let EventKind::CommandDecision { cmd, tank, .. } = &rej.kind else { return None };
    let text = m
        .events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::CommandSubmitted { cmd: c, tank: t, text } if c == cmd && t == tank => Some(clean(text)),
            _ => None,
        })
        ?;
    let cut = |s: String, n: usize| if s.chars().count() > n { format!("{}...", s.chars().take(n).collect::<String>()) } else { s };
    Some(format!("Rejected #{cmd} {}: {}", cut(clean(&m.seats[*tank].agent), 10), cut(text, 36)))
}

/// The whole life of the order behind a command event, as far as the feed
/// has delivered: what was submitted, what the server decided and why, and
/// what came of it. Only the server's own events are reported.
fn command_trace(m: &Match, delivered: &[crate::feed::Event], sel: &crate::feed::Event) -> Option<String> {
    let (cmd, tank) = event_cmd(sel)?;
    let mine = || delivered.iter().filter(move |e| event_cmd(e) == Some((cmd, tank)));
    let who = clean(&m.seats[tank].agent);
    let order = mine().find_map(|e| match &e.kind {
        EventKind::CommandSubmitted { text, .. } => Some(format!("Order #{cmd} from {who} (t{}): {}", e.tick, clean(text))),
        _ => None,
    });
    let decision = mine().find_map(|e| match &e.kind {
        EventKind::CommandDecision { accepted: true, .. } => Some((true, format!("Server decision (t{}): accepted", e.tick))),
        EventKind::CommandDecision { accepted: false, reason, .. } => Some((false, format!("Server decision (t{}): REJECTED\nReason: {}", e.tick, clean(reason)))),
        _ => None,
    });
    let phase = mine().filter_map(|e| match &e.kind {
        EventKind::CommandPhase { phase, .. } => Some((e.tick, phase.label())),
        _ => None,
    }).last();
    let effect = match (&decision, phase) {
        (Some((false, _)), _) => "Effect: not carried out; the tank keeps its previous orders".to_string(),
        (Some((true, _)), Some((t, p))) => format!("Effect: order {p} (t{t})"),
        (Some((true, _)), None) => "Effect: accepted, not started yet".to_string(),
        (None, _) => "Effect: waiting for the server's decision".to_string(),
    };
    Some([order.unwrap_or_else(|| format!("Order #{cmd} from {who}: text not in the delivered feed")), decision.map(|d| d.1).unwrap_or_else(|| "Server decision: pending".into()), effect].join("\n"))
}

fn event_line(m: &Match, e: &crate::feed::Event) -> Option<(TlFilter, String)> {
    let name = |i: usize| clean(&m.seats[i].agent);
    let t = format!("t{} {}", e.tick, clock(e.tick as f32 / TICK_HZ));
    Some(match &e.kind {
        EventKind::CommandSubmitted { cmd, tank, text } => (TlFilter::Commands, format!("{t} {} #{cmd} sent: {}", name(*tank), clean(text))),
        EventKind::CommandDecision { cmd, tank, accepted, reason } => (
            TlFilter::Commands,
            if *accepted { format!("{t} {} #{cmd} accepted", name(*tank)) } else { format!("{t} {} #{cmd} REJECTED: {}", name(*tank), clean(reason)) },
        ),
        EventKind::CommandPhase { cmd, tank, phase } => (TlFilter::Commands, format!("{t} {} #{cmd} {}", name(*tank), phase.label())),
        EventKind::Shot { tank, round, .. } => (TlFilter::Combat, format!("{t} {} fired {}", name(*tank), round.label())),
        EventKind::Impact { target, effect, .. } => (
            TlFilter::Combat,
            match target {
                Some(i) => format!("{t} hit on {}: {}", name(*i), clean(effect)),
                None => format!("{t} impact: {}", clean(effect)),
            },
        ),
        EventKind::AgentDisconnected { tank } => (TlFilter::System, format!("{t} -- {} disconnected", name(*tank))),
        EventKind::AgentReconnected { tank } => (TlFilter::System, format!("{t} -- {} reconnected", name(*tank))),
        EventKind::ServerRecovery { paused_ticks } => (TlFilter::System, format!("{t} -- server recovery ({paused_ticks} ticks)")),
        EventKind::MissingInterval { from_tick, to_tick } => (TlFilter::System, format!("{t} -- missing interval t{from_tick}-t{to_tick}")),
        EventKind::Result { text } => (TlFilter::System, format!("{t} -- result: {}", clean(text))),
        EventKind::Aborted { reason } => (TlFilter::System, format!("{t} -- aborted: {}", clean(reason))),
        EventKind::MatchStarted => (TlFilter::System, format!("{t} -- match started")),
        EventKind::Snapshot { .. } => return None,
    })
}

#[allow(clippy::type_complexity)]
/// Phone telemetry: the same facts as the desktop panel, packed so every
/// crew and damage line fits in half the screen width.
fn compact_panel(seat: &crate::feed::Seat, s: &crate::feed::TankSnap, cap: f32) -> String {
    let short = ["Cmdr", "Gun", "Load", "Drv", "Radio"];
    let parts = ["Eng", "L trk", "R trk", "Ring", "Gun"];
    let mut out = format!("{} ({})\n{:.0}/{:.0} L  {:.0} km/h\n", clean(&seat.agent), seat.side.label(), s.fuel_l, cap, s.speed.abs() * 3.6);
    out += &Round::ALL.iter().map(|r| format!("{} {}", r.label(), s.ammo[r.index()])).collect::<Vec<_>>().join("  ");
    out += &match s.loaded {
        Some(r) if s.reload <= 0.0 => format!("\nLoaded {} ready", r.label()),
        Some(r) => format!("\nLoading {} {:.0}%", r.label(), s.reload * 100.0),
        None => "\nNot loaded".into(),
    };
    out += "\nCrew";
    for (i, c) in s.crew.iter().enumerate() {
        out += &format!("\n {} {}", short[i], c.label());
    }
    out += "\nDamage";
    for (i, p) in s.parts.iter().enumerate() {
        out += &format!("\n {} {}", parts[i], p.label());
    }
    if s.on_fire {
        out += "\nON FIRE";
    }
    if s.knocked_out {
        out += "\nKNOCKED OUT";
    }
    out
}

fn timeline_buttons(
    q: Query<(&Interaction, Option<&TlRow>, Option<&TlFilterBtn>, Option<&TlSeek>), Changed<Interaction>>,
    keys: Res<ButtonInput<KeyCode>>,
    feeds: Res<Feeds>,
    mut tl: ResMut<TlState>,
    mut v: ResMut<Viewing>,
) {
    for (i, row, f, seek) in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        if let Some(r) = row {
            tl.selected = tl.rows.get(r.0).copied();
        }
        if let Some(f) = f {
            tl.filter = f.0;
        }
        if seek.is_some() && !v.live {
            seek_to_selected(&feeds.0[v.idx], &tl, &mut v);
        }
    }
    // Up and Down move the selection through the visible rows.
    let up = keys.just_pressed(KeyCode::ArrowUp);
    let down = keys.just_pressed(KeyCode::ArrowDown);
    if (up || down) && !tl.rows.is_empty() {
        let at = tl.selected.and_then(|s| tl.rows.iter().position(|r| *r == s));
        let n = tl.rows.len();
        let next = match (at, down) {
            (None, _) => 0,
            (Some(i), true) => (i + 1).min(n - 1),
            (Some(i), false) => i.saturating_sub(1),
        };
        tl.selected = Some(tl.rows[next]);
    }
}

fn seek_to_selected(m: &Match, tl: &TlState, v: &mut Viewing) {
    if let Some(e) = tl.selected.and_then(|s| m.events.iter().find(|e| e.seq == s)) {
        v.cursor = e.tick as f32;
        v.prev_cursor = v.cursor;
        v.paused = true;
    }
}

#[allow(clippy::type_complexity)]
fn timeline(
    feeds: Res<Feeds>,
    v: Res<Viewing>,
    mut tl: ResMut<TlState>,
    mut rows: Query<(&TlRow, &mut Node, &mut On), (Without<TlSeek>, Without<TlDetail>)>,
    mut row_text: Query<(&TlRowText, &mut Text, &mut TextLayout), Without<TlDetail>>,
    mut detail: Query<(&mut Text, &mut Node), (With<TlDetail>, Without<TlRow>)>,
    mut seek: Query<&mut Node, (With<TlSeek>, Without<TlRow>, Without<TlDetail>)>,
    mut filters: Query<(&TlFilterBtn, &mut On), Without<TlRow>>,
    windows: Query<&Window>,
) {
    let narrow = windows.single().is_ok_and(|w| w.width() < NARROW_PX);
    let m = &feeds.0[v.idx];
    let end = m.events_until(v.cursor);
    let mut lines: Vec<(u64, String)> = Vec::new();
    for e in m.events[..end].iter().rev() {
        let Some((cat, line)) = event_line(m, e) else { continue };
        if tl.filter != TlFilter::All && tl.filter != cat {
            continue;
        }
        lines.push((e.seq, line));
        if lines.len() >= TL_ROWS {
            break;
        }
    }
    tl.rows = lines.iter().map(|(s, _)| *s).collect();
    for (r, mut n, mut on) in &mut rows {
        let want = if r.0 < lines.len() { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
        on.0 = lines.get(r.0).is_some_and(|(s, _)| Some(*s) == tl.selected);
    }
    // Escaped command text has long unbroken tokens; on a phone they must
    // break anywhere rather than run past the panel edge.
    let brk = if narrow { LineBreak::AnyCharacter } else { LineBreak::WordBoundary };
    for (r, mut t, mut lay) in &mut row_text {
        if lay.linebreak != brk {
            lay.linebreak = brk;
        }
        let want = lines.get(r.0).map(|(_, l)| l.as_str()).unwrap_or("");
        if t.0 != want {
            t.0 = want.to_string();
        }
    }
    let sel = tl.selected.and_then(|s| m.events[..end].iter().find(|e| e.seq == s));
    for (mut t, mut n) in &mut detail {
        t.0 = match sel.and_then(|e| event_line(m, e).map(|l| (e, l.1))) {
            // Narrow screens: the highlighted row already shows the line.
            Some((e, line)) => match command_trace(m, &m.events[..end], e) {
                Some(trace) => format!("Selected event #{} at server tick {}\n{trace}", e.seq, e.tick),
                // Narrow screens: the highlighted row already shows the line.
                None if narrow => format!("Selected event #{} at server tick {}", e.seq, e.tick),
                None => format!("Selected event #{} at server tick {}\n{line}", e.seq, e.tick),
            },
            None if lines.is_empty() => "No events match this filter yet.".into(),
            None if narrow => "Tap an event to select it.".into(),
            None => "Click an event, or press Up/Down, to select it.".into(),
        };
        n.display = Display::Flex;
    }
    for mut n in &mut seek {
        n.display = if sel.is_some() { Display::Flex } else { Display::None };
    }
    for (f, mut on) in &mut filters {
        on.0 = f.0 == tl.filter;
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
            format!("tick {}  {} live (delay {} ticks)", v.cursor.floor() as u32, clock(v.cursor / TICK_HZ), crate::LIVE_DELAY_TICKS as u32)
        } else {
            format!("tick {} / {}  {} / {}  {}x", v.cursor.floor() as u32, end as u32, clock(v.cursor / TICK_HZ), clock(end / TICK_HZ), v.speed)
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
    mut q: Query<(&NameLabel, &mut Node, &mut Visibility, &ComputedNode)>,
) {
    let Ok((camera, ct)) = cam.single() else { return };
    let Some(view) = camera.logical_viewport_size() else { return };
    let m = &feeds.0[v.idx];
    for (l, mut n, mut vis, cn) in &mut q {
        let hide = (matches!(cs.mode, CamMode::Station(_)) && l.0 == v.tank) || cs.mode == CamMode::Agent;
        let pos = m.tank_at(l.0, v.cursor).map(|s| tank_pose(&s).translation + Vec3::Y * 4.5);
        // A name that would hang off the screen edge is hidden, never cut.
        let size = cn.size() * cn.inverse_scale_factor();
        let fits = |p: Vec2| label_fits(Vec2::new(p.x - 30.0, p.y - 10.0), size, view);
        match pos.and_then(|p| camera.world_to_viewport(ct, p).ok()) {
            Some(p) if !hide && fits(p) => {
                n.left = Val::Px(p.x - 30.0);
                n.top = Val::Px(p.y - 10.0);
                *vis = Visibility::Inherited;
            }
            _ => *vis = Visibility::Hidden,
        }
    }
}

/// Whether a label placed at `at` with `size` lies wholly inside the view.
fn label_fits(at: Vec2, size: Vec2, view: Vec2) -> bool {
    at.x >= 0.0 && at.y >= 0.0 && at.x + size.x <= view.x && at.y + size.y <= view.y
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_highlight_leaves_timeline_selection_alone() {
        let mut app = App::new();
        app.init_resource::<Viewing>().init_resource::<CamState>().add_systems(Update, hud_toggles);
        let filter = app.world_mut().spawn((TlFilterBtn(TlFilter::Commands), On(true))).id();
        let row = app.world_mut().spawn((TlRow(0), On(true))).id();
        let orbit = app.world_mut().spawn((CamBtn(CamMode::Orbit), On(false))).id();
        let follow = app.world_mut().spawn((CamBtn(CamMode::Follow), On(true))).id();
        app.update();
        let on = |e| app.world().get::<On>(e).unwrap().0;
        assert!(on(filter) && on(row), "selected filter and row keep their highlight");
        assert!(on(orbit) && !on(follow));
    }

    #[test]
    fn rejected_order_trace_shows_order_reason_and_effect() {
        let feeds = crate::feed::demo_matches();
        let m = &feeds[0];
        let rej = m.events.iter().find(|e| matches!(e.kind, EventKind::CommandDecision { accepted: false, .. })).expect("demo has a rejected order");
        let EventKind::CommandDecision { reason, .. } = &rej.kind else { unreachable!() };
        let t = command_trace(m, &m.events, rej).expect("command events have a trace");
        assert!(t.contains("Order #"), "{t}");
        assert!(t.contains("REJECTED"), "{t}");
        assert!(t.contains(&format!("Reason: {}", clean(reason))), "{t}");
        assert!(t.contains("Effect: not carried out"), "{t}");
        // A selected submission traces to the same decision.
        let sub = m.events.iter().find(|e| event_cmd(e) == event_cmd(rej) && matches!(e.kind, EventKind::CommandSubmitted { .. })).unwrap();
        assert_eq!(command_trace(m, &m.events, sub).unwrap(), t);
        // Escaped hostile text never reaches the trace raw.
        for e in &m.events {
            if let Some(tr) = command_trace(m, &m.events, e) {
                assert!(!tr.contains('\u{202e}') && tr.lines().count() <= 4, "{tr}");
            }
        }
        // The finished duel replay carries a hostile order the crew rules reject.
        let duel = feeds.iter().find(|m| m.id == "m-0412").unwrap();
        let rej = duel.events.iter().find(|e| matches!(e.kind, EventKind::CommandDecision { accepted: false, .. })).expect("duel has a rejected order");
        let t = command_trace(duel, &duel.events, rej).unwrap();
        assert!(t.contains("&lt;img") || t.contains("<img"), "{t}");
        assert!(t.contains("<U+202E>") && t.contains("Reason: no round in the breech"), "{t}");
    }

    #[test]
    fn names_hanging_off_the_edge_are_not_placed() {
        let view = Vec2::new(1600.0, 900.0);
        let size = Vec2::new(70.0, 18.0);
        assert!(label_fits(Vec2::new(10.0, 10.0), size, view));
        assert!(!label_fits(Vec2::new(-30.0, 280.0), size, view));
        assert!(!label_fits(Vec2::new(1560.0, 280.0), size, view));
        assert!(!label_fits(Vec2::new(500.0, 890.0), size, view));
    }

    #[test]
    fn other_matches_strip_shows_result_and_aborted_interval() {
        let ms = crate::feed::demo_matches();
        let done = other_match_line(&ms[1], None, &[]);
        let aborted = other_match_line(&ms[2], None, &[]);
        assert!(done.contains("\nRESULT - "), "{done}");
        assert!(!done.contains("ABORTED"));
        assert!(aborted.contains("\nABORTED - "), "{aborted}");
        assert!(aborted.contains("\nno data "), "{aborted}");
        assert!(!aborted.contains("RESULT"));
        assert_eq!(other_match_line(&ms[0], None, &[]), format!("{} {}\nLIVE", ms[0].id, ms[0].mode));
        let stalled = other_match_line(&ms[0], Some(4.0), &["marlow".to_string()]);
        assert!(stalled.ends_with("\nLIVE\nLAG 4.0s - feed stalled\nDISCONNECTED: marlow"), "{stalled}");
        assert!(!other_match_line(&ms[0], Some(0.2), &[]).contains("LAG"));
    }

    #[test]
    fn directory_line_shows_escaped_rejected_order() {
        let feeds = crate::feed::demo_matches();
        let duel = feeds.iter().find(|m| m.id == "m-0412").unwrap();
        let l = last_rejected_order(duel).expect("duel card shows its rejected order");
        assert!(l.starts_with("Rejected #103 ironwood: ") && l.contains("<img src=x") && l.contains("<U+202E>"), "{l}");
        assert!(!l.contains('\u{202e}') && l.chars().count() <= 100, "{l}");
        let c001 = crate::contract::from_json(include_str!("../fixtures/contract-v1-sample.json")).unwrap();
        let c001 = &c001;
        assert_eq!(last_rejected_order(c001), None);
    }
}

/// Shows the agent's view: the sight report the selected seat is given at
/// the playback cursor, plotted and written out.
#[allow(clippy::type_complexity)]
fn agent_view(
    feeds: Res<Feeds>,
    v: Res<Viewing>,
    cs: Res<CamState>,
    mut root: Query<&mut Visibility, (With<AgentView>, Without<AgentBlip>)>,
    mut texts: ParamSet<(Query<&mut Text, With<AgentTitle>>, Query<&mut Text, With<AgentReport>>, Query<&mut Text>)>,
    mut blips: Query<(&AgentBlip, &mut Node, &mut Visibility, &Children), Without<AgentView>>,
) {
    let on = cs.mode == CamMode::Agent;
    for mut vis in &mut root {
        *vis = if on { Visibility::Inherited } else { Visibility::Hidden };
    }
    if !on {
        return;
    }
    let m = &feeds.0[v.idx];
    let tick = v.cursor.floor();
    for mut t in &mut texts.p0() {
        t.0 = format!("Agent's view - {} - tick {}", clean(&m.seats[v.tank].agent), tick as u32);
    }
    let sight = crate::sight::look_at(m, v.tank, tick);
    let contacts = match &sight {
        Some(crate::sight::Sight::Report(c)) => c.clone(),
        _ => Vec::new(),
    };
    let report = match &sight {
        None => "No report yet: this seat has no position at this tick.".to_string(),
        Some(crate::sight::Sight::Destroyed) => "No report: the vehicle is knocked out and its sight is gone.".to_string(),
        Some(crate::sight::Sight::Report(c)) if c.is_empty() => "Forward sight: no contacts.".to_string(),
        Some(crate::sight::Sight::Report(c)) => {
            let mut out = format!("Forward sight: {} contact{}", c.len(), if c.len() == 1 { "" } else { "s" });
            for (i, k) in c.iter().enumerate() {
                out.push_str(&format!("\n  {}. {}, about {} m", i + 1, crate::sight::bearing_words(k.bearing_deg), k.range_m));
            }
            out
        }
    };
    for mut t in &mut texts.p1() {
        t.0 = report.clone();
    }
    let mut labels = Vec::new();
    for (b, mut n, mut vis, kids) in &mut blips {
        match contacts.get(b.0) {
            Some(k) => {
                let half = crate::sight::HALF_ANGLE_DEG as f32;
                n.left = Val::Percent((half - k.bearing_deg as f32) / (2.0 * half) * 100.0);
                n.top = Val::Percent((1.0 - k.range_m as f32 / crate::sight::MAX_RANGE_M as f32) * 100.0);
                *vis = Visibility::Inherited;
                labels.extend(kids.iter().map(|e| (e, format!("{} - {} m", b.0 + 1, k.range_m))));
            }
            None => *vis = Visibility::Hidden,
        }
    }
    let mut all = texts.p2();
    for (e, s) in labels {
        if let Ok(mut t) = all.get_mut(e) {
            t.0 = s;
        }
    }
}
