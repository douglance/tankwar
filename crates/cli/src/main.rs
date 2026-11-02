use incurs::cli::Cli;
use incurs::command::{CommandDef, TypedContext, TypedResult};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tankwar_protocol::{Command, Order};

#[derive(Deserialize, incurs::Args)]
struct HealthArgs {}
#[derive(Deserialize, incurs::Args)]
struct OrderArgs {
    /// A JSON command conforming to the Tankwar protocol.
    command: String,
}
#[derive(Deserialize, incurs::Args)]
struct QueueArgs { /// Match mode, 1v1 or 2v2.
    mode: String }
#[derive(Deserialize, incurs::Args)]
struct ReadyArgs { /// Match ID returned by the queue.
    match_id: String }
#[derive(Deserialize, incurs::Args)]
struct FireArgs { match_id: String, seat_id: String, command_id: String }
#[derive(Deserialize, incurs::Args)]
struct EngineArgs { match_id: String, seat_id: String, command_id: String, running: bool }
#[derive(Deserialize, incurs::Args)]
struct DriveArgs { match_id: String, seat_id: String, command_id: String, throttle: i16, steering: i16, ttl_ticks: u32 }
#[derive(Deserialize, incurs::Args)]
struct LoadArgs { match_id: String, seat_id: String, command_id: String, ammunition: String }
#[derive(Deserialize, incurs::Options)]
struct Options {}
#[derive(JsonSchema, Serialize)]
struct ServerOutput { status: String, body: String }

fn server_url(base: &str, path: &str) -> Result<reqwest::Url, &'static str> {
    let mut url = reqwest::Url::parse(base).map_err(|_| "Invalid server URL")?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if url.scheme() != "https" && !(url.scheme() == "http" && local) {
        return Err("Server URL must use HTTPS, except on localhost");
    }
    if !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() {
        return Err("Server URL must not contain credentials, a query, or a fragment");
    }
    url.set_path(&format!("{}{}", url.path().trim_end_matches('/'), path));
    Ok(url)
}

fn valid_match_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

async fn send_order(command_json: &str) -> TypedResult<ServerOutput> {
    let command: Command = match serde_json::from_str(command_json) {
        Ok(value) => value,
        Err(_) => return TypedResult::error("invalid_command", "Command must be valid Tankwar command JSON"),
    };
    send_command(command).await
}

async fn send_command(command: Command) -> TypedResult<ServerOutput> {
    if !valid_match_id(&command.match_id) {
        return TypedResult::error("invalid_command", "Match ID contains invalid characters");
    }
    send_authenticated(&format!("/v1/matches/{}/commands", command.match_id), serde_json::to_value(command).unwrap()).await
}

async fn crew_order(match_id: String, seat_id: String, id: String, order: Order) -> TypedResult<ServerOutput> {
    send_command(Command { protocol_version: tankwar_protocol::PROTOCOL_VERSION, id, match_id, seat_id, expected_version: None, order }).await
}

async fn send_authenticated(path: &str, body: serde_json::Value) -> TypedResult<ServerOutput> {
    let client_id = std::env::var("TANKWAR_ACCESS_CLIENT_ID").ok().filter(|v| !v.is_empty());
    let client_secret = std::env::var("TANKWAR_ACCESS_CLIENT_SECRET").ok().filter(|v| !v.is_empty());
    let token = std::env::var("TANKWAR_ACCESS_TOKEN").ok().filter(|v| !v.is_empty());
    if client_id.is_some() != client_secret.is_some() || (client_id.is_none() && token.is_none()) {
        return TypedResult::error("missing_auth", "Set both TANKWAR_ACCESS_CLIENT_ID and TANKWAR_ACCESS_CLIENT_SECRET");
    }
    let base = std::env::var("TANKWAR_URL").unwrap_or_else(|_| "https://tankwar.doug-lance.workers.dev".into());
    let url = match server_url(&base, path) {
        Ok(url) => url,
        Err(message) => return TypedResult::error("invalid_url", message),
    };
    let client = match reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build() {
        Ok(client) => client,
        Err(_) => return TypedResult::error("connection_error", "Could not create server client"),
    };
    let mut request = client.post(url);
    if let (Some(id), Some(secret)) = (client_id, client_secret) {
        request = request.header("CF-Access-Client-Id", id).header("CF-Access-Client-Secret", secret);
    } else if let Some(token) = token {
        // Placeholder bearer credentials are retained for explicitly configured local servers.
        request = request.bearer_auth(token);
    }
    match request.json(&body).send().await {
        Ok(response) => {
            let status = response.status();
            if !status.is_success() {
                return TypedResult::error("server_rejected", format!("Server returned HTTP {status}"));
            }
            match response.text().await {
                Ok(body) => TypedResult::ok(ServerOutput { status: status.to_string(), body }),
                Err(_) => TypedResult::error("response_error", "Could not read the server response"),
            }
        }
        Err(_) => TypedResult::error("connection_error", "Could not reach the Tankwar server"),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let health = CommandDef::typed::<HealthArgs, Options, (), ServerOutput, _, _>(
        "health", |_ctx: TypedContext<HealthArgs, Options, ()>| async move {
            let base = std::env::var("TANKWAR_URL").unwrap_or_else(|_| "https://tankwar.doug-lance.workers.dev".into());
            let url = match server_url(&base, "/health") {
                Ok(url) => url,
                Err(message) => return TypedResult::error("invalid_url", message),
            };
            match reqwest::get(url).await {
                Ok(response) => {
                    let status = response.status().to_string();
                    match response.text().await {
                        Ok(body) => TypedResult::ok(ServerOutput { status, body }),
                        Err(_) => TypedResult::error("response_error", "Could not read the server response"),
                    }
                }
                Err(_) => TypedResult::error("connection_error", "Could not reach the Tankwar server"),
            }
        },
    ).description("Check the Tankwar server status").done();
    let order = CommandDef::typed::<OrderArgs, Options, (), ServerOutput, _, _>(
        "order", |ctx: TypedContext<OrderArgs, Options, ()>| async move { send_order(&ctx.args.command).await },
    ).description("Submit a command to a match").done();
    let fire = CommandDef::typed::<FireArgs, Options, (), ServerOutput, _, _>(
        "fire", |ctx: TypedContext<FireArgs, Options, ()>| async move {
            let a = ctx.args; crew_order(a.match_id, a.seat_id, a.command_id, Order::Fire).await
        },
    ).description("Fire the loaded round").done();
    let engine = CommandDef::typed::<EngineArgs, Options, (), ServerOutput, _, _>(
        "engine", |ctx: TypedContext<EngineArgs, Options, ()>| async move {
            let a = ctx.args; crew_order(a.match_id, a.seat_id, a.command_id, Order::Engine { running: a.running }).await
        },
    ).description("Start or stop the engine").done();
    let drive = CommandDef::typed::<DriveArgs, Options, (), ServerOutput, _, _>(
        "drive", |ctx: TypedContext<DriveArgs, Options, ()>| async move {
            let a = ctx.args; crew_order(a.match_id, a.seat_id, a.command_id, Order::Drive { throttle:a.throttle, steering:a.steering, ttl_ticks:a.ttl_ticks }).await
        },
    ).description("Set throttle and steering for a bounded number of ticks").done();
    let load = CommandDef::typed::<LoadArgs, Options, (), ServerOutput, _, _>(
        "load", |ctx: TypedContext<LoadArgs, Options, ()>| async move {
            let a = ctx.args; crew_order(a.match_id, a.seat_id, a.command_id, Order::Load { ammunition:a.ammunition }).await
        },
    ).description("Load an ammunition type").done();
    let queue = CommandDef::typed::<QueueArgs, Options, (), ServerOutput, _, _>(
        "queue", |ctx: TypedContext<QueueArgs, Options, ()>| async move {
            if !matches!(ctx.args.mode.as_str(), "1v1" | "2v2") { return TypedResult::error("invalid_mode", "Mode must be 1v1 or 2v2"); }
            send_authenticated("/v1/queue", serde_json::json!({"mode":ctx.args.mode})).await
        },
    ).description("Join the global match queue").done();
    let ready = CommandDef::typed::<ReadyArgs, Options, (), ServerOutput, _, _>(
        "ready", |ctx: TypedContext<ReadyArgs, Options, ()>| async move {
            if !valid_match_id(&ctx.args.match_id) { return TypedResult::error("invalid_match", "Match ID contains invalid characters"); }
            send_authenticated(&format!("/v1/matches/{}/ready", ctx.args.match_id), serde_json::json!({})).await
        },
    ).description("Mark your assigned tank ready").done();
    let leave = CommandDef::typed::<HealthArgs, Options, (), ServerOutput, _, _>(
        "leave", |_ctx: TypedContext<HealthArgs, Options, ()>| async move { send_authenticated("/v1/queue/leave", serde_json::json!({})).await },
    ).description("Leave the waiting queue").done();
    let observe = CommandDef::typed::<ReadyArgs, Options, (), ServerOutput, _, _>(
        "observe", |ctx: TypedContext<ReadyArgs, Options, ()>| async move {
            if !valid_match_id(&ctx.args.match_id) { return TypedResult::error("invalid_match", "Match ID contains invalid characters"); }
            send_authenticated(&format!("/v1/matches/{}/observe", ctx.args.match_id), serde_json::json!({})).await
        },
    ).description("Inspect your assigned tank's instruments").done();
    let look = CommandDef::typed::<ReadyArgs, Options, (), ServerOutput, _, _>(
        "look", |ctx: TypedContext<ReadyArgs, Options, ()>| async move {
            if !valid_match_id(&ctx.args.match_id) { return TypedResult::error("invalid_match", "Match ID contains invalid characters"); }
            send_authenticated(&format!("/v1/matches/{}/look", ctx.args.match_id), serde_json::json!({})).await
        },
    ).description("Look through the approximate forward sight").done();
    Cli::create("tankwar").version("0.1.0").command("health", health).command("queue", queue).command("leave", leave).command("ready", ready).command("observe", observe).command("look", look).command("order", order).command("fire", fire).command("engine", engine).command("drive", drive).command("load", load).serve().await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn url_prevents_leaking_tokens_to_insecure_or_malformed_hosts() {
        assert!(server_url("http://example.com", "/x").is_err());
        assert!(server_url("https://user:pass@example.com", "/x").is_err());
        assert!(server_url("https://example.com/?access_token=x", "/x").is_err());
        assert_eq!(server_url("http://127.0.0.1:8000", "/x").unwrap().as_str(), "http://127.0.0.1:8000/x");
        assert!(!valid_match_id("../other"));
        assert!(valid_match_id("match-123"));
    }
}
