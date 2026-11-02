//! Village fights published by the server.
//!
//! The directory reads `GET /v1/matches` and one spectate body per id. Replay
//! reads `GET /v1/matches/{id}/replay?tick=N` every 20 ticks, which is the
//! widest gap the battlefield will still move across. The native build keeps
//! the compiled demo feeds; it has no server origin.

use crate::camera::CamState;
use crate::feed::Match;
use crate::{Feeds, Screen, Viewing};
use bevy::prelude::*;
use serde_json::Value;
use std::sync::{Arc, Mutex};
#[cfg(target_arch = "wasm32")]
use crate::contract::from_server;
use tankwar_protocol::{Event as PEvent, Snapshot};

pub const DEMO_NOTE: &str = "Demo feed: these matches come from a local generator until the server's spectator stream is wired in.\nSpectator sound is ambience for people watching; agents never receive it.";
const SERVER_NOTE: &str = "These are the village fights on the server. Choose Replay to watch one from the first tick.";

#[derive(Resource)]
pub struct Catalog {
    pub generation: u32,
    pub note: String,
}

impl Default for Catalog {
    fn default() -> Self {
        Self {
            generation: 0,
            note: if cfg!(target_arch = "wasm32") { "Loading village fights from the server.".into() } else { DEMO_NOTE.into() },
        }
    }
}

/// Set when a recording has been filled in and the battlefield should open.
#[derive(Resource, Default)]
pub struct OpenReady(pub Option<usize>);

#[derive(Clone)]
#[allow(dead_code)]
struct Slot {
    id: String,
    end_tick: u64,
    commands: Vec<PEvent>,
    world: Vec<Value>,
    preview: Snapshot,
}

#[allow(dead_code)]
enum Msg {
    List { matches: Vec<Match>, slots: Vec<Slot> },
    Failed,
    Ready { idx: usize, m: Match },
    ReplayFailed,
}

struct Inner {
    msgs: Vec<Msg>,
    slots: Vec<Slot>,
    loading: bool,
}

#[derive(Resource, Clone)]
pub struct Bridge(Arc<Mutex<Inner>>);

impl Default for Bridge {
    fn default() -> Self {
        Bridge(Arc::new(Mutex::new(Inner { msgs: Vec::new(), slots: Vec::new(), loading: false })))
    }
}

pub struct RemotePlugin;

impl Plugin for RemotePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Catalog>()
            .init_resource::<Bridge>()
            .init_resource::<OpenReady>()
            .add_systems(Update, (apply, open_ready).chain());
        #[cfg(target_arch = "wasm32")]
        app.add_systems(Startup, start_fetch);
    }
}

fn apply(bridge: Res<Bridge>, mut feeds: ResMut<Feeds>, mut catalog: ResMut<Catalog>, mut open: ResMut<OpenReady>) {
    let msgs: Vec<Msg> = {
        let Ok(mut inner) = bridge.0.lock() else { return };
        inner.msgs.drain(..).collect()
    };
    for msg in msgs {
        match msg {
            Msg::List { matches, slots } => {
                let n = matches.len();
                feeds.0 = matches;
                if let Ok(mut inner) = bridge.0.lock() {
                    inner.slots = slots;
                    inner.loading = false;
                }
                catalog.note = if n == 0 { "The server has no village fights to show.".into() } else { SERVER_NOTE.into() };
                catalog.generation = catalog.generation.wrapping_add(1);
            }
            Msg::Failed => {
                catalog.note = "The server match list did not load. Refresh the page.".into();
                catalog.generation = catalog.generation.wrapping_add(1);
            }
            Msg::Ready { idx, m } => {
                if idx < feeds.0.len() {
                    feeds.0[idx] = m;
                }
                if let Ok(mut inner) = bridge.0.lock() {
                    inner.loading = false;
                }
                catalog.note = SERVER_NOTE.into();
                open.0 = Some(idx);
            }
            Msg::ReplayFailed => {
                if let Ok(mut inner) = bridge.0.lock() {
                    inner.loading = false;
                }
                catalog.note = "That recording did not load. Try it again.".into();
            }
        }
    }
}

fn open_ready(mut pending: ResMut<OpenReady>, feeds: Res<Feeds>, mut v: ResMut<Viewing>, mut cam: ResMut<CamState>, mut next: ResMut<NextState<Screen>>) {
    let Some(idx) = pending.0.take() else { return };
    let Some(m) = feeds.0.get(idx) else { return };
    *v = Viewing::open(idx, false, m);
    cam.focus = Vec3::ZERO;
    next.set(Screen::Battlefield);
}

/// Ask the server for the replay frames of one listed fight, then open it.
#[cfg(target_arch = "wasm32")]
pub fn load_recording(bridge: &Bridge, idx: usize) -> bool {
    let slot = {
        let Ok(mut inner) = bridge.0.lock() else { return false };
        if inner.loading {
            return false;
        }
        let Some(slot) = inner.slots.get(idx).cloned() else { return false };
        inner.loading = true;
        slot
    };
    let bridge = bridge.0.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let mut frames = vec![slot.preview.clone()];
        for tick in sample_ticks(slot.end_tick) {
            if let Some(frame) = fetch_replay(&slot.id, tick).await {
                frames.push(frame);
            }
        }
        let msg = match from_server(frames, slot.commands.clone(), &slot.world) {
            Some(m) => Msg::Ready { idx, m },
            None => Msg::ReplayFailed,
        };
        if let Ok(mut inner) = bridge.lock() {
            inner.msgs.push(msg);
        }
    });
    true
}

/// Returns whether the battlefield should open now. On the server build a thin
/// card waits for replay frames first.
pub fn prepare_watch(bridge: &Bridge, idx: usize, id: &str, dense: bool, note: &mut String) -> bool {
    #[cfg(target_arch = "wasm32")]
    if !dense {
        if load_recording(bridge, idx) {
            *note = format!("Loading the recording for {id}.");
        }
        return false;
    }
    let _ = (bridge, idx, id, dense, note);
    true
}

/// Ticks to request so neighbouring frames stay within the interpolation gap.
pub fn sample_ticks(end: u64) -> Vec<u64> {
    if end == 0 {
        return vec![0];
    }
    let mut ticks = Vec::new();
    let mut tick = 0_u64;
    while tick < end {
        ticks.push(tick);
        tick += 20;
    }
    if ticks.last().copied() != Some(end) {
        ticks.push(end);
    }
    ticks
}

pub fn parse_directory(body: &str) -> Option<Vec<String>> {
    let v: serde_json::Map<String, Value> = serde_json::from_str(body).ok()?;
    let mut ids: Vec<String> = v
        .into_iter()
        .map(|(id, _)| id)
        .filter(|id| !id.is_empty() && id.len() <= 128 && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'))
        .collect();
    ids.sort_by_key(|id| std::cmp::Reverse(rank(id)));
    Some(ids)
}

fn rank(id: &str) -> i64 {
    if id == "village" {
        -1
    } else if let Some(n) = id.strip_prefix("village-").and_then(|n| n.parse::<i64>().ok()) {
        n
    } else {
        i64::MIN / 2
    }
}

#[cfg(target_arch = "wasm32")]
fn start_fetch(bridge: Res<Bridge>) {
    let bridge = bridge.0.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let Some(body) = get_text("/v1/matches").await else {
            if let Ok(mut inner) = bridge.lock() {
                inner.msgs.push(Msg::Failed);
            }
            return;
        };
        let Some(ids) = parse_directory(&body) else {
            if let Ok(mut inner) = bridge.lock() {
                inner.msgs.push(Msg::Failed);
            }
            return;
        };
        if ids.is_empty() {
            if let Ok(mut inner) = bridge.lock() {
                inner.msgs.push(Msg::List { matches: Vec::new(), slots: Vec::new() });
            }
            return;
        }
        let queue = Arc::new(Mutex::new(ids.clone()));
        let acc: Arc<Mutex<Vec<(Match, Slot)>>> = Arc::new(Mutex::new(Vec::new()));
        let left = Arc::new(Mutex::new(ids.len()));
        let workers = 6.min(ids.len());
        for _ in 0..workers {
            let queue = queue.clone();
            let acc = acc.clone();
            let left = left.clone();
            let bridge = bridge.clone();
            wasm_bindgen_futures::spawn_local(async move {
                loop {
                    let next = queue.lock().ok().and_then(|mut q| q.pop());
                    let Some(id) = next else { break };
                    if let Some(pair) = fetch_slot(&id).await {
                        if let Ok(mut acc) = acc.lock() {
                            acc.push(pair);
                        }
                    }
                    let finished = left.lock().ok().is_some_and(|mut n| {
                        *n = n.saturating_sub(1);
                        *n == 0
                    });
                    if finished {
                        let mut pairs = acc.lock().map(|mut a| std::mem::take(&mut *a)).unwrap_or_default();
                        pairs.sort_by_key(|(m, _)| std::cmp::Reverse(rank(&m.id)));
                        let (matches, slots): (Vec<Match>, Vec<Slot>) = pairs.into_iter().unzip();
                        if let Ok(mut inner) = bridge.lock() {
                            inner.msgs.push(if matches.is_empty() { Msg::Failed } else { Msg::List { matches, slots } });
                        }
                        break;
                    }
                }
            });
        }
    });
}

#[cfg(target_arch = "wasm32")]
async fn fetch_slot(id: &str) -> Option<(Match, Slot)> {
    let body = get_text(&format!("/v1/matches/{id}/spectate")).await?;
    let parsed: SpectateBody = serde_json::from_str(&body).ok()?;
    let slot = Slot {
        id: id.to_string(),
        end_tick: parsed.snapshot.tick,
        commands: parsed.commands.clone(),
        world: parsed.world_events.clone(),
        preview: parsed.snapshot.clone(),
    };
    let m = from_server(vec![parsed.snapshot], parsed.commands, &parsed.world_events)?;
    Some((m, slot))
}

#[cfg(target_arch = "wasm32")]
async fn fetch_replay(id: &str, tick: u64) -> Option<Snapshot> {
    let body = get_text(&format!("/v1/matches/{id}/replay?tick={tick}")).await?;
    serde_json::from_str(&body).ok()
}

#[cfg(target_arch = "wasm32")]
async fn get_text(url: &str) -> Option<String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    let window = web_sys::window()?;
    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    init.set_mode(web_sys::RequestMode::SameOrigin);
    let req = web_sys::Request::new_with_str_and_init(url, &init).ok()?;
    req.headers().set("Accept", "application/json").ok()?;
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_request(&req)).await.ok()?.dyn_into().ok()?;
    if !resp.ok() {
        return None;
    }
    JsFuture::from(resp.text().ok()?).await.ok()?.as_string()
}

#[cfg(target_arch = "wasm32")]
#[derive(serde::Deserialize)]
struct SpectateBody {
    snapshot: Snapshot,
    #[serde(default)]
    commands: Vec<PEvent>,
    #[serde(default)]
    world_events: Vec<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_orders_newest_village_first() {
        let ids = parse_directory(r#"{"village":"OneOnOne","village-2":"OneOnOne","village-10":"OneOnOne","ranked match":"no"}"#).unwrap();
        assert_eq!(ids, vec!["village-10".to_string(), "village-2".to_string(), "village".to_string()]);
    }

    #[test]
    fn samples_cover_the_interpolation_gap() {
        let ticks = sample_ticks(721);
        assert_eq!(ticks.first().copied(), Some(0));
        assert_eq!(ticks.last().copied(), Some(721));
        for pair in ticks.windows(2) {
            assert!(pair[1] - pair[0] <= 20, "{pair:?}");
        }
        assert_eq!(sample_ticks(0), vec![0]);
    }
}
