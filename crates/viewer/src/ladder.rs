//! The public season ladder shown in the match directory.
//!
//! Reads `GET /v1/leaderboard?mode=1v1` and `mode=2v2` from the origin that
//! served the viewer. Ratings are shown exactly as the server sends them; the
//! viewer never computes or fills in a rating.

use bevy::prelude::*;
use std::sync::{Arc, Mutex};

pub const MODES: [&str; 2] = ["1v1", "2v2"];

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub agent_id: String,
    /// The rating text as the server sent it.
    pub rating: String,
    pub games: u64,
    pub provisional: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Board {
    Loading,
    Unavailable,
    Ready { season: String, entries: Vec<Entry> },
}

/// One board per mode, filled in by the fetch as responses arrive.
#[derive(Resource, Clone)]
pub struct Ladder(pub Arc<Mutex<[Board; 2]>>);

impl Default for Ladder {
    fn default() -> Self {
        Ladder(Arc::new(Mutex::new([Board::Loading, Board::Loading])))
    }
}

impl Ladder {
    pub fn get(&self, i: usize) -> Board {
        self.0.lock().map(|b| b[i].clone()).unwrap_or(Board::Unavailable)
    }
}

/// Parse a leaderboard response. Anything that is not the documented shape is
/// treated as unavailable rather than guessed at.
pub fn parse(body: &str, mode: &str) -> Board {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else { return Board::Unavailable };
    if v.get("mode").and_then(|m| m.as_str()).is_some_and(|m| m != mode) {
        return Board::Unavailable;
    }
    let Some(list) = v.get("entries").and_then(|e| e.as_array()) else { return Board::Unavailable };
    let mut entries = Vec::new();
    for e in list {
        let (Some(id), Some(r)) = (e.get("agent_id").and_then(|x| x.as_str()), e.get("rating").filter(|r| r.is_number())) else {
            return Board::Unavailable;
        };
        entries.push(Entry {
            agent_id: id.to_string(),
            rating: r.to_string(),
            games: e.get("games").and_then(|g| g.as_u64()).unwrap_or(0),
            provisional: e.get("provisional").and_then(|p| p.as_bool()).unwrap_or(false),
        });
    }
    let season = v.get("season").and_then(|s| s.as_str()).unwrap_or("").to_string();
    Board::Ready { season, entries }
}

pub struct LadderPlugin;

impl Plugin for LadderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Ladder>().add_systems(Startup, fetch);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch(ladder: Res<Ladder>) {
    // The native build has no server origin to read from.
    if let Ok(mut b) = ladder.0.lock() {
        *b = [Board::Unavailable, Board::Unavailable];
    }
}

#[cfg(target_arch = "wasm32")]
fn fetch(ladder: Res<Ladder>) {
    for (i, mode) in MODES.iter().enumerate() {
        let shared = ladder.0.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let board = get(mode).await.unwrap_or(Board::Unavailable);
            if let Ok(mut b) = shared.lock() {
                b[i] = board;
            }
        });
    }
}

#[cfg(target_arch = "wasm32")]
async fn get(mode: &str) -> Option<Board> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    let window = web_sys::window()?;
    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    init.set_mode(web_sys::RequestMode::SameOrigin);
    let req = web_sys::Request::new_with_str_and_init(&format!("/v1/leaderboard?mode={mode}"), &init).ok()?;
    req.headers().set("Accept", "application/json").ok()?;
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_request(&req)).await.ok()?.dyn_into().ok()?;
    if !resp.ok() {
        return None;
    }
    let text = JsFuture::from(resp.text().ok()?).await.ok()?.as_string()?;
    Some(parse(&text, mode))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_documented_shape_and_keeps_rating_text() {
        let b = parse(r#"{"season":"season-1","mode":"1v1","entries":[{"agent_id":"a<b>","rating":1516.25,"games":3,"provisional":true}]}"#, "1v1");
        assert_eq!(
            b,
            Board::Ready {
                season: "season-1".into(),
                entries: vec![Entry { agent_id: "a<b>".into(), rating: "1516.25".into(), games: 3, provisional: true }]
            }
        );
    }

    #[test]
    fn empty_and_malformed() {
        assert_eq!(parse(r#"{"season":"season-1","mode":"2v2","entries":[]}"#, "2v2"), Board::Ready { season: "season-1".into(), entries: vec![] });
        assert_eq!(parse("<html>not found</html>", "1v1"), Board::Unavailable);
        assert_eq!(parse(r#"{"mode":"2v2","entries":[]}"#, "1v1"), Board::Unavailable);
        assert_eq!(parse(r#"{"entries":[{"agent_id":"x"}]}"#, "1v1"), Board::Unavailable);
    }
}
