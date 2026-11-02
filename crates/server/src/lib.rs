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
    let is_queue = matches!(path.as_str(), "/v1/queue" | "/v1/queue/leave");
    let is_match = parts.len() == 5 && parts[1] == "v1" && parts[2] == "matches" && matches!(parts[4], "commands" | "ready" | "observe");
    if req.method() != Method::Post || !(is_queue || is_match) {
        if !path.starts_with("/v1/") && matches!(req.method(), Method::Get | Method::Head) {
            if let Ok(assets) = env.service("ASSETS") { return assets.fetch_request(req).await; }
        }
        return Response::error("not found", 404);
    }
    let scope = if is_queue { "queue:write" } else if parts[4] == "observe" { "match:observe" } else { "match:command" };
    let (Ok(issuer), Ok(audience), Ok(jwks)) = (env.var("AUTH_ISSUER"), env.var("AUTH_AUDIENCE"), env.var("AUTH_JWKS")) else {
        return Response::error("authentication is not configured", 503);
    };
    let authorization = req.headers().get("authorization")?.unwrap_or_default();
    let Some(token) = authorization.strip_prefix("Bearer ") else { return Response::error("unauthorized", 401); };
    let agent_id = match auth::verify_agent(token, &issuer.to_string(), &audience.to_string(), &jwks.to_string(), scope) {
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
    if parts[4] == "observe" {
        return forward(&env, "MATCHES", match_id, "/observe", serde_json::json!({"agent_id": agent_id})).await;
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
pub mod ledger;
pub mod match_runtime;
pub mod matchmaking;

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
                    let _ = game.advance(Date::now().as_millis());
                    let running = game.world.state == tankwar_protocol::MatchState::Running;
                    storage.put("match", game).await?;
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
            let Some(game): Option<match_runtime::DurableMatch> = self.state.storage().get("match").await? else { return Response::error("match not found", 404); };
            return Response::from_json(&serde_json::json!({
                "snapshot": game.world.spectator_snapshot(&game.ledger.match_id, game.ledger.version),
                "commands": game.ledger.events,
                "world_events": game.world_events,
            }));
        }
        if req.path() == "/observe" {
            #[derive(serde::Deserialize)] struct Observe { agent_id: String }
            let input: Observe = req.json().await?;
            let _guard = self.lock.lock().await;
            let Some(game): Option<match_runtime::DurableMatch> = self.state.storage().get("match").await? else { return Response::error("match not found", 404); };
            return match game.observe_self(&input.agent_id) {
                Ok(snapshot) => Response::from_json(&snapshot),
                Err(reason) => Response::error(reason, 403),
            };
        }
        if req.path() == "/ready" {
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
            if all_ready && game.world.state == tankwar_protocol::MatchState::Ready {
                if let Err(reason) = game.start(Date::now().as_millis()) { return Response::error(reason, 409); }
            }
            let running = game.world.state == tankwar_protocol::MatchState::Running;
            storage.put("match", game).await?;
            if running { storage.set_alarm(2000_i64).await?; self.start_loop(); }
            return Response::from_json(&serde_json::json!({"all_ready": all_ready, "running": running}));
        }
        if req.path() == "/start" {
            let _guard = self.lock.lock().await;
            let storage = self.state.storage();
            let Some(mut game): Option<match_runtime::DurableMatch> = storage.get("match").await? else { return Response::error("match not found", 404); };
            if game.world.state == tankwar_protocol::MatchState::Ready {
                if let Err(reason) = game.start(Date::now().as_millis()) { return Response::error(reason, 409); }
                storage.put("match", &game).await?;
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
        match game.accept(&input.agent_id, input.command) {
            ledger::Decision::Accept(event) => {
                storage.put("match", game).await?;
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
            storage.put("match", &game).await?;
        }
        if game.world.state == tankwar_protocol::MatchState::Running {
            if !self.loop_active.get() {
                let now = Date::now().as_millis();
                let paused_millis = game.clock.as_ref().map(|clock| now.saturating_sub(clock.next_tick_ms().saturating_sub(50))).unwrap_or(0);
                game.clock = Some(tankwar_sim::clock::Clock::start(now).map_err(|e| Error::RustError(e.into()))?);
                game.world_events.push(match_runtime::RecordedWorldEvent { tick: game.world.tick, event: tankwar_sim::world::WorldEvent::Recovered { paused_millis } });
                storage.put("match", &game).await?;
                self.start_loop();
            }
            storage.set_alarm(2000_i64).await?;
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
        let input: InternalQueueRequest = req.json().await?;
        let _guard = self.lock.lock().await;
        let storage = self.state.storage();
        let mut queue: matchmaking::Queue = storage.get("queue").await?.unwrap_or_default();
        match queue.join(&input.agent_id, input.mode) {
            Ok(assignment) => {
                storage.put("queue", queue).await?;
                if let Some(ref assigned) = assignment {
                    let actor = self.env.durable_object("MATCHES")?.id_from_name(&assigned.match_id)?.get_stub()?;
                    let payload = serde_json::to_string(assigned)?;
                    let mut init = RequestInit::new();
                    init.with_method(Method::Post).with_body(Some(worker::wasm_bindgen::JsValue::from_str(&payload)));
                    let response = actor.fetch_with_request(Request::new_with_init("https://match.internal/initialize", &init)?).await?;
                    if response.status_code() != 200 { return Response::error("match initialization failed", 503); }
                }
                Response::from_json(&assignment)
            }
            Err(reason) => Response::error(reason, 409),
        }
    }
}
