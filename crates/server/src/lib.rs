//! Cloudflare gateway for Tankwar's authoritative match service.
use worker::*;

async fn forward(env: &Env, binding: &str, name: &str, path: &str, payload: serde_json::Value) -> Result<Response> {
    let actor = env.durable_object(binding)?.id_from_name(name)?.get_stub()?;
    let body = serde_json::to_string(&payload)?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_body(Some(worker::wasm_bindgen::JsValue::from_str(&body)));
    actor.fetch_with_request(Request::new_with_init(&format!("https://internal{path}"), &init)?).await
}

#[event(fetch)]
pub async fn fetch(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    if req.path() == "/health" {
        return Response::from_json(&serde_json::json!({"service":"tankwar","status":"initializing","matches_available":false}));
    }
    let path = req.path();
    if path == "/tankwar-viewer_bg.wasm" && req.method() == Method::Get {
        let Some(object) = env.bucket("VIEWER")?.get("tankwar-viewer_bg.wasm").execute().await? else {
            return Response::error("spectator wasm missing", 404);
        };
        let Some(body) = object.body() else { return Response::error("spectator wasm missing", 404); };
        let mut response = Response::from_body(body.response_body()?)?;
        response.headers_mut().set("content-type", "application/wasm")?;
        response.headers_mut().set("cache-control", "public, max-age=300")?;
        return Ok(response);
    }
    if path == "/v1/matches" && req.method() == Method::Get {
        return forward(&env, "QUEUES", "global", "/directory", serde_json::json!({})).await;
    }
    if path == "/v1/leaderboard" && req.method() == Method::Get {
        let mode = req.url()?.query_pairs().find(|(key, _)| key == "mode").map(|(_, value)| value.into_owned()).unwrap_or_default();
        if !matches!(mode.as_str(), "1v1" | "2v2") { return Response::error("invalid mode", 400); }
        return forward(&env, "LADDERS", "season-1", "/leaderboard", serde_json::json!({"mode":mode})).await;
    }
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() == 5 && parts[1] == "v1" && parts[2] == "matches" && parts[4] == "spectate" && req.method() == Method::Get {
        let match_id = parts[3];
        if match_id.is_empty() || match_id.len() > 128 || !match_id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') { return Response::error("invalid match id", 400); }
        return forward(&env, "MATCHES", match_id, "/spectate", serde_json::json!({})).await;
    }
    if parts.len() == 5 && parts[1] == "v1" && parts[2] == "matches" && parts[4] == "replay" && req.method() == Method::Get {
        let match_id = parts[3];
        if match_id.is_empty() || match_id.len() > 128 || !match_id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') { return Response::error("invalid match id", 400); }
        let tick = req.url()?.query_pairs().find(|(key, _)| key == "tick").and_then(|(_, value)| value.parse::<u64>().ok());
        let Some(tick) = tick.filter(|tick| *tick <= tankwar_protocol::MATCH_TICKS) else { return Response::error("invalid replay tick", 400); };
        return forward(&env, "MATCHES", match_id, "/replay", serde_json::json!({"tick":tick})).await;
    }
    let is_queue = matches!(path.as_str(), "/v1/queue" | "/v1/queue/leave");
    let is_match = parts.len() == 5 && parts[1] == "v1" && parts[2] == "matches" && matches!(parts[4], "commands" | "ready" | "observe" | "look");
    if req.method() != Method::Post || !(is_queue || is_match) {
        if !path.starts_with("/v1/") && matches!(req.method(), Method::Get | Method::Head) {
            if let Ok(assets) = env.service("ASSETS") { return assets.fetch_request(req).await; }
        }
        return Response::error("not found", 404);
    }
    let scope = if is_queue { "queue:write" } else if matches!(parts[4], "observe" | "look") { "match:observe" } else { "match:command" };
    let (Ok(issuer), Ok(audience), Ok(jwks)) = (env.var("AUTH_ISSUER"), env.var("AUTH_AUDIENCE"), env.var("AUTH_JWKS")) else {
        return Response::error("authentication is not configured", 503);
    };
    let auth_mode = env.var("AUTH_MODE").map(|v| v.to_string()).unwrap_or_else(|_| "access".into());
    let verification = if auth_mode == "placeholder" {
        let authorization = req.headers().get("authorization")?.unwrap_or_default();
        match authorization.strip_prefix("Bearer ") {
            Some(token) => auth::verify_agent(token, &issuer.to_string(), &audience.to_string(), &jwks.to_string(), scope),
            None => Err("missing_token"),
        }
    } else if auth_mode == "access" {
        let Ok(grants) = env.var("ACCESS_AGENT_BINDINGS") else { return Response::error("authentication is not configured", 503); };
        let assertion = req.headers().get("cf-access-jwt-assertion")?.unwrap_or_default();
        access::verify_service(&assertion, &issuer.to_string(), &audience.to_string(), &jwks.to_string(), &grants.to_string(), scope)
    } else { return Response::error("authentication is not configured", 503); };
    let agent_id = match verification {
        Ok(agent) => agent,
        Err(_) => return Response::error("unauthorized", 401),
    };
    if path == "/v1/queue/leave" {
        return forward(&env, "QUEUES", "global", "/leave", serde_json::json!({"agent_id":agent_id})).await;
    }
    if is_queue {
        #[derive(serde::Deserialize)] struct Join { mode: String }
        let input: Join = match req.json().await { Ok(input) => input, Err(_) => return Response::error("invalid queue request", 400) };
        let mode = match input.mode.as_str() { "1v1" => tankwar_sim::competition::Mode::OneOnOne, "2v2" => tankwar_sim::competition::Mode::TwoOnTwo, _ => return Response::error("invalid mode", 400) };
        return forward(&env, "QUEUES", "global", "/join", serde_json::json!({"agent_id": agent_id, "mode": mode})).await;
    }
    let match_id = parts[3];
    if match_id.is_empty() || match_id.len() > 128 || !match_id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') {
        return Response::error("invalid match id", 400);
    }
    if matches!(parts[4], "observe" | "look") {
        return forward(&env, "MATCHES", match_id, &format!("/{}", parts[4]), serde_json::json!({"agent_id": agent_id})).await;
    }
    if parts[4] == "ready" {
        return forward(&env, "MATCHES", match_id, "/ready", serde_json::json!({"agent_id": agent_id})).await;
    }
    let body = req.text().await?;
    if body.len() > 16_384 { return Response::error("command too large", 413); }
    let command: tankwar_protocol::Command = match serde_json::from_str(&body) {
        Ok(command) => command,
        Err(_) => return Response::error("invalid command", 400),
    };
    if command.match_id != match_id { return Response::error("wrong match", 400); }
    forward(&env, "MATCHES", match_id, "/command", serde_json::json!({ "agent_id": agent_id, "command": command })).await
}

pub mod auth;
pub mod access;
pub mod archive;
pub mod ledger;
pub mod match_runtime;
pub mod matchmaking;

// Ranked admission stays closed until the release asset inventory and rights
// review are complete. Opening it requires an explicit code change and review.
fn ranked_admission() -> std::result::Result<(), &'static str> {
    Err("ranked play is unavailable while the asset gate is open")
}

#[cfg(test)]
mod release_gate_tests {
    #[test]
    fn ranked_admission_is_closed_until_assets_are_reviewed() {
        assert!(super::ranked_admission().is_err());
    }
}

/// Internal match actor. Only the gateway supplies its verified agent identity.
#[durable_object]
pub struct MatchActor {
    state: State,
    env: Env,
    lock: std::rc::Rc<futures::lock::Mutex<()>>,
    loop_active: std::rc::Rc<std::cell::Cell<bool>>,
}

#[derive(serde::Deserialize)]
struct InternalCommand { agent_id: String, command: tankwar_protocol::Command }

async fn persist_match_and_frames(storage: &Storage, game: &mut match_runtime::DurableMatch, frames: Vec<tankwar_protocol::Snapshot>) -> Result<()> {
    let batch = archive::prepare(game);
    let game = game.clone();
    storage.transaction(move |transaction| async move {
        for event in batch.commands {
            let command = archive::event_command(&event).ok_or_else(|| Error::RustError("invalid archived command event".into()))?;
            transaction.put(&archive::command_key(&command.id), &event).await?;
            transaction.put(&format!("command/{:020}", event.sequence), &event).await?;
        }
        for (sequence, event) in batch.world_events {
            transaction.put(&format!("world-event/{sequence:020}"), &event).await?;
        }
        for frame in frames {
            transaction.put(&format!("replay/{:05}", frame.tick), &frame).await?;
        }
        transaction.put("match", &game).await
    }).await
}

async fn read_history<T: serde::de::DeserializeOwned>(storage: &Storage, prefix: &str, count: u64) -> Result<Vec<T>> {
    let mut events = Vec::new();
    for keys in archive::key_batches(prefix, count) {
        let values = storage.get_multiple(keys.clone()).await?;
        for key in keys {
            let key = worker::wasm_bindgen::JsValue::from_str(&key);
            if values.has(&key) {
                events.push(serde_wasm_bindgen::from_value(values.get(&key)).map_err(|e| Error::RustError(e.to_string()))?);
            }
        }
    }
    Ok(events)
}

impl MatchActor {
    fn start_loop(&self) {
        if self.loop_active.replace(true) { return; }
        let active = self.loop_active.clone();
        let lock = self.lock.clone();
        let storage = self.state.storage();
        self.state.wait_until(async move {
            loop {
                Delay::from(std::time::Duration::from_millis(50)).await;
                let _guard = lock.lock().await;
                let outcome: Result<bool> = async {
                    let Some(mut game): Option<match_runtime::DurableMatch> = storage.get("match").await? else { return Ok(false); };
                    if game.world.state != tankwar_protocol::MatchState::Running { return Ok(false); }
                    let frames = game.advance_with_frames(Date::now().as_millis()).unwrap_or_default();
                    let running = game.world.state == tankwar_protocol::MatchState::Running;
                    persist_match_and_frames(&storage, &mut game, frames).await?;
                    Ok(running)
                }.await;
                if !matches!(outcome, Ok(true)) { break; }
            }
            active.set(false);
        });
    }
}

impl DurableObject for MatchActor {
    fn new(state: State, env: Env) -> Self { Self { state, env, lock: std::rc::Rc::new(futures::lock::Mutex::new(())), loop_active: std::rc::Rc::new(std::cell::Cell::new(false)) } }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        if req.method() != Method::Post { return Response::error("not found", 404); }
        if req.path() == "/initialize" {
            let assignment: matchmaking::Assignment = req.json().await?;
            let game = match match_runtime::DurableMatch::from_assignment(&assignment) {
                Ok(game) => game,
                Err(reason) => return Response::error(reason, 400),
            };
            let _guard = self.lock.lock().await;
            let storage = self.state.storage();
            if let Some(existing) = storage.get::<match_runtime::DurableMatch>("match").await? {
                if existing.ledger.match_id != game.ledger.match_id || existing.ledger.seats != game.ledger.seats {
                    return Response::error("assignment conflict", 409);
                }
            } else {
                storage.put("match", game).await?;
            }
            if storage.get_alarm().await?.is_none() { storage.set_alarm(60_000_i64).await?; }
            return Response::from_json(&serde_json::json!({"match_id": assignment.match_id}));
        }
        if req.path() == "/spectate" {
            let _guard = self.lock.lock().await;
            let storage = self.state.storage();
            let Some(game): Option<match_runtime::DurableMatch> = storage.get("match").await? else { return Response::error("match not found", 404); };
            let mut commands = read_history::<tankwar_protocol::Event>(&storage, "command", game.ledger.version).await?;
            commands.extend(game.ledger.events.clone());
            commands.sort_by_key(|event| event.sequence);
            if !archive::complete_commands(&commands, game.ledger.version) { return Response::error("command archive incomplete", 500); }
            let mut world_events = read_history::<match_runtime::RecordedWorldEvent>(&storage, "world-event", game.archived_world_events).await?;
            if world_events.len() as u64 != game.archived_world_events { return Response::error("event archive incomplete", 500); }
            world_events.extend(game.world_events.clone());
            return Response::from_json(&serde_json::json!({
                "snapshot": game.world.spectator_snapshot(&game.ledger.match_id, game.ledger.version),
                "commands": commands,
                "world_events": world_events,
            }));
        }
        if req.path() == "/replay" {
            #[derive(serde::Deserialize)] struct ReplayRequest { tick: u64 }
            let input: ReplayRequest = req.json().await?;
            let _guard = self.lock.lock().await;
            let frame: Option<tankwar_protocol::Snapshot> = self.state.storage().get(&format!("replay/{:05}", input.tick)).await?;
            return match frame { Some(frame) => Response::from_json(&frame), None => Response::error("replay frame unavailable", 404) };
        }
        if matches!(req.path().as_str(), "/observe" | "/look") {
            #[derive(serde::Deserialize)] struct Observe { agent_id: String }
            let input: Observe = req.json().await?;
            let _guard = self.lock.lock().await;
            let Some(game): Option<match_runtime::DurableMatch> = self.state.storage().get("match").await? else { return Response::error("match not found", 404); };
            if req.path() == "/look" {
                return match game.look(&input.agent_id) {
                    Ok(report) => Response::from_json(&report),
                    Err(reason) => Response::error(reason, 403),
                };
            }
            return match game.observe_self(&input.agent_id) { Ok(snapshot) => Response::from_json(&snapshot), Err(reason) => Response::error(reason, 403) };
        }
        if req.path() == "/ready" {
            if let Err(reason) = ranked_admission() { return Response::error(reason, 503); }
            #[derive(serde::Deserialize)]
            struct Ready { agent_id: String }
            let input: Ready = req.json().await?;
            let _guard = self.lock.lock().await;
            let storage = self.state.storage();
            let Some(mut game): Option<match_runtime::DurableMatch> = storage.get("match").await? else { return Response::error("match not found", 404); };
            let all_ready = match game.mark_ready(&input.agent_id) {
                Ok(ready) => ready,
                Err(reason) => return Response::error(reason, 409),
            };
            let mut frames = Vec::new();
            if all_ready && game.world.state == tankwar_protocol::MatchState::Ready {
                if let Err(reason) = game.start(Date::now().as_millis()) { return Response::error(reason, 409); }
                frames.push(game.world.spectator_snapshot(&game.ledger.match_id, game.ledger.version));
            }
            let running = game.world.state == tankwar_protocol::MatchState::Running;
            persist_match_and_frames(&storage, &mut game, frames).await?;
            if running { storage.set_alarm(2000_i64).await?; self.start_loop(); }
            return Response::from_json(&serde_json::json!({"all_ready": all_ready, "running": running}));
        }
        if req.path() == "/start" {
            if let Err(reason) = ranked_admission() { return Response::error(reason, 503); }
            let _guard = self.lock.lock().await;
            let storage = self.state.storage();
            let Some(mut game): Option<match_runtime::DurableMatch> = storage.get("match").await? else { return Response::error("match not found", 404); };
            if game.world.state == tankwar_protocol::MatchState::Ready {
                if let Err(reason) = game.start(Date::now().as_millis()) { return Response::error(reason, 409); }
                let frame = game.world.spectator_snapshot(&game.ledger.match_id, game.ledger.version);
                persist_match_and_frames(&storage, &mut game, vec![frame]).await?;
            } else if game.world.state != tankwar_protocol::MatchState::Running { return Response::error("match not ready", 409); }
            storage.set_alarm(2000_i64).await?;
            self.start_loop();
            return Response::from_json(&serde_json::json!({"state":"running"}));
        }
        if req.path() != "/command" { return Response::error("not found", 404); }
        let input: InternalCommand = req.json().await?;
        let _guard = self.lock.lock().await;
        let storage = self.state.storage();
        let Some(mut game): Option<match_runtime::DurableMatch> = storage.get("match").await? else { return Response::error("match not found", 404); };
        if let Some(prior) = storage.get::<tankwar_protocol::Event>(&archive::command_key(&input.command.id)).await? {
            return match game.ledger.decide_archived(&input.agent_id, &input.command, &prior) {
                ledger::Decision::Duplicate(event) => Response::from_json(&event),
                ledger::Decision::Reject(reason) => Response::error(reason, 409),
                ledger::Decision::Accept(_) => Response::error("invalid archive decision", 500),
            };
        }
        match game.accept(&input.agent_id, input.command) {
            ledger::Decision::Accept(event) => {
                persist_match_and_frames(&storage, &mut game, Vec::new()).await?;
                Response::from_json(&event)
            }
            ledger::Decision::Duplicate(event) => Response::from_json(&event),
            ledger::Decision::Reject(reason) => Response::error(reason, 409),
        }
    }

    async fn alarm(&self) -> Result<Response> {
        let _guard = self.lock.lock().await;
        let storage = self.state.storage();
        let Some(mut game): Option<match_runtime::DurableMatch> = storage.get("match").await? else { return Response::empty(); };
        if matches!(game.ledger.state, tankwar_protocol::MatchState::Assigned | tankwar_protocol::MatchState::Ready) && game.world.state == tankwar_protocol::MatchState::Ready {
            game.cancel_unready().map_err(|e| Error::RustError(e.into()))?;
            persist_match_and_frames(&storage, &mut game, Vec::new()).await?;
        }
        if game.world.state == tankwar_protocol::MatchState::Running {
            if !self.loop_active.get() {
                let frames = game.recover(Date::now().as_millis());
                persist_match_and_frames(&storage, &mut game, frames).await?;
                if game.world.state == tankwar_protocol::MatchState::Running { self.start_loop(); }
            }
            if game.world.state == tankwar_protocol::MatchState::Running { storage.set_alarm(2000_i64).await?; }
        }
        if matches!(game.world.state, tankwar_protocol::MatchState::Finished | tankwar_protocol::MatchState::Aborted | tankwar_protocol::MatchState::Cancelled) {
            if game.world.state == tankwar_protocol::MatchState::Finished && !game.rating_published {
                let (mode, first, second, outcome) = game.result().map_err(|e| Error::RustError(e.into()))?;
                let payload = serde_json::json!({"season":"season-1", "match_id":game.ledger.match_id, "mode":mode, "first":first, "second":second, "outcome":outcome});
                if let Ok(response) = forward(&self.env, "LADDERS", "season-1", "/result", payload).await {
                    if response.status_code() == 200 { game.rating_published = true; storage.put("match", &game).await?; }
                }
            }
            if !game.queue_released {
                if let Ok(response) = forward(&self.env, "QUEUES", "global", "/release", serde_json::json!({"match_id": game.ledger.match_id})).await {
                    if response.status_code() == 200 { game.queue_released = true; storage.put("match", &game).await?; }
                }
            }
            if !game.queue_released || (game.world.state == tankwar_protocol::MatchState::Finished && !game.rating_published) { storage.set_alarm(2000_i64).await?; }
        }
        Response::empty()
    }
}

/// Serializes updates for one season. Only internal services can reach this object.
#[durable_object]
pub struct LadderActor {
    state: State,
    lock: futures::lock::Mutex<()>,
}

#[derive(serde::Deserialize)]
struct InternalResult {
    season: String,
    match_id: String,
    mode: tankwar_sim::competition::Mode,
    first: Vec<String>,
    second: Vec<String>,
    outcome: tankwar_sim::competition::Outcome,
}

impl DurableObject for LadderActor {
    fn new(state: State, _env: Env) -> Self { Self { state, lock: futures::lock::Mutex::new(()) } }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        if req.method() != Method::Post { return Response::error("not found", 404); }
        if req.path() == "/leaderboard" {
            #[derive(serde::Deserialize)] struct Query { mode: String }
            let input: Query = req.json().await?;
            let mode = match input.mode.as_str() { "1v1" => tankwar_sim::competition::Mode::OneOnOne, "2v2" => tankwar_sim::competition::Mode::TwoOnTwo, _ => return Response::error("invalid mode", 400) };
            let _guard = self.lock.lock().await;
            let ratings: tankwar_sim::competition::Ratings = self.state.storage().get("ratings").await?.unwrap_or(tankwar_sim::competition::Ratings::for_season("season-1").map_err(|e| Error::RustError(e.into()))?);
            return Response::from_json(&serde_json::json!({"season":ratings.season(), "mode":input.mode, "entries":ratings.leaderboard(mode)}));
        }
        if req.path() != "/result" { return Response::error("not found", 404); }
        let input: InternalResult = req.json().await?;
        if input.season.is_empty() || input.season.len() > 128 { return Response::error("invalid season", 400); }
        let _guard = self.lock.lock().await;
        let storage = self.state.storage();
        let mut ratings: tankwar_sim::competition::Ratings = match storage.get("ratings").await? {
            Some(ratings) => ratings,
            None => match tankwar_sim::competition::Ratings::for_season(&input.season) {
                Ok(ratings) => ratings,
                Err(reason) => return Response::error(reason, 400),
            },
        };
        if ratings.season() != input.season { return Response::error("wrong season", 409); }
        match ratings.apply(&input.match_id, input.mode, &input.first, &input.second, input.outcome) {
            Ok(changes) => {
                storage.put("ratings", ratings).await?;
                Response::from_json(&changes)
            }
            Err(reason) => Response::error(reason, 409),
        }
    }
}

/// Serializes queue tickets and assignments; not exposed by the public gateway yet.
#[durable_object]
pub struct QueueActor {
    state: State,
    env: Env,
    lock: futures::lock::Mutex<()>,
}

#[derive(serde::Deserialize)]
struct InternalQueueRequest { agent_id: String, mode: tankwar_sim::competition::Mode }

impl DurableObject for QueueActor {
    fn new(state: State, env: Env) -> Self { Self { state, env, lock: futures::lock::Mutex::new(()) } }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        if req.method() != Method::Post { return Response::error("not found", 404); }
        if req.path() == "/directory" {
            let _guard = self.lock.lock().await;
            let queue: matchmaking::Queue = self.state.storage().get("queue").await?.unwrap_or_default();
            return Response::from_json(queue.matches());
        }
        if req.path() == "/leave" {
            #[derive(serde::Deserialize)] struct Leave { agent_id: String }
            let input: Leave = req.json().await?;
            let _guard = self.lock.lock().await;
            let storage = self.state.storage();
            let mut queue: matchmaking::Queue = storage.get("queue").await?.unwrap_or_default();
            if queue.assignment(&input.agent_id).is_some() { return Response::error("already assigned", 409); }
            let left = queue.leave(&input.agent_id);
            if left { storage.put("queue", queue).await?; }
            return Response::from_json(&serde_json::json!({"left":left}));
        }
        if req.path() == "/release" {
            #[derive(serde::Deserialize)] struct Release { match_id: String }
            let input: Release = req.json().await?;
            let _guard = self.lock.lock().await;
            let storage = self.state.storage();
            let mut queue: matchmaking::Queue = storage.get("queue").await?.unwrap_or_default();
            queue.release(&input.match_id);
            storage.put("queue", queue).await?;
            return Response::empty();
        }
        if req.path() != "/join" { return Response::error("not found", 404); }
        if let Err(reason) = ranked_admission() { return Response::error(reason, 503); }
        let input: InternalQueueRequest = req.json().await?;
        let _guard = self.lock.lock().await;
        let storage = self.state.storage();
        let mut queue: matchmaking::Queue = storage.get("queue").await?.unwrap_or_default();
        match queue.join(&input.agent_id, input.mode) {
            Ok(assignment) => {
                // Schedule recovery before recording an assignment, so a failed actor
                // call or interrupted response cannot strand its participants.
                if assignment.is_some() { storage.set_alarm(2000_i64).await?; }
                storage.put("queue", &queue).await?;
                drop(_guard);
                if let Some(ref assigned) = assignment {
                    let actor = self.env.durable_object("MATCHES")?.id_from_name(&assigned.match_id)?.get_stub()?;
                    let payload = serde_json::to_string(assigned)?;
                    let mut init = RequestInit::new();
                    init.with_method(Method::Post).with_body(Some(worker::wasm_bindgen::JsValue::from_str(&payload)));
                    let response = actor.fetch_with_request(Request::new_with_init("https://match.internal/initialize", &init)?).await?;
                    if response.status_code() != 200 { return Response::error("match initialization failed", 503); }
                    let _guard = self.lock.lock().await;
                    let mut latest: matchmaking::Queue = storage.get("queue").await?.unwrap_or_default();
                    latest.initialized(&assigned.match_id);
                    storage.put("queue", &latest).await?;
                }
                Response::from_json(&assignment)
            }
            Err(reason) => Response::error(reason, 409),
        }
    }

    async fn alarm(&self) -> Result<Response> {
        let storage = self.state.storage();
        let pending = {
            let _guard = self.lock.lock().await;
            let queue: matchmaking::Queue = storage.get("queue").await?.unwrap_or_default();
            queue.pending_initializations()
        };
        for assignment in pending {
            let result = forward(&self.env, "MATCHES", &assignment.match_id, "/initialize", serde_json::to_value(&assignment)?).await;
            if matches!(result, Ok(ref response) if response.status_code() == 200) {
                let _guard = self.lock.lock().await;
                let mut queue: matchmaking::Queue = storage.get("queue").await?.unwrap_or_default();
                queue.initialized(&assignment.match_id);
                storage.put("queue", &queue).await?;
            }
        }
        let _guard = self.lock.lock().await;
        let queue: matchmaking::Queue = storage.get("queue").await?.unwrap_or_default();
        if !queue.pending_initializations().is_empty() { storage.set_alarm(2000_i64).await?; }
        Response::empty()
    }
}
