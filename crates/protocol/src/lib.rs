//! Versioned wire types shared by the authority and its clients.
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
pub const TICKS_PER_SECOND: u64 = 20;
pub const MATCH_TICKS: u64 = 15 * 60 * TICKS_PER_SECOND;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchState { Queued, Assigned, Ready, Running, Finished, Cancelled, Aborted }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Order {
    Engine { running: bool },
    Drive { throttle: i16, steering: i16, ttl_ticks: u32 },
    Load { ammunition: String },
    Fire,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub protocol_version: u32,
    pub id: String,
    pub match_id: String,
    pub seat_id: String,
    pub expected_version: Option<u64>,
    pub order: Order,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    Accepted { command: Command },
    Rejected { command_id: String, reason: String },
    Started { command_id: String },
    Completed { command_id: String },
    Interrupted { command_id: String, reason: String },
    Expired { command_id: String },
    StateChanged { state: MatchState },
    MatchDrawn,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event { pub sequence: u64, pub tick: u64, pub event: EventKind }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TankSnapshot {
    pub seat_id: String,
    pub fuel_millilitres: u64,
    pub engine_running: bool,
    pub ammunition: Vec<(String, u32)>,
    pub loaded_round: Option<String>,
    /// Sim plane: x east, y north. Heading is radians counterclockwise from east.
    #[serde(default)] pub x_m: Option<f64>,
    #[serde(default)] pub y_m: Option<f64>,
    #[serde(default)] pub heading_rad: Option<f64>,
    #[serde(default)] pub team: Option<u8>,
    #[serde(default)] pub reload_ticks_remaining: Option<u32>,
    #[serde(default)] pub knocked_out: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub protocol_version: u32,
    pub match_id: String,
    pub state: MatchState,
    pub tick: u64,
    pub sequence: u64,
    /// Scheduled simulation deadline and actual processing time, in Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub scheduled_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub processed_at_ms: Option<u64>,
    pub tanks: Vec<TankSnapshot>,
    #[serde(default)] pub structures: Vec<StructureSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StructureSnapshot {
    pub id: String,
    pub integrity: u32,
    pub destroyed: bool,
}

/// Approximate, hull-fixed forward sight report; no hidden vehicle identifiers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SightReport {
    pub protocol_version: u32,
    pub match_id: String,
    pub tick: u64,
    pub station: String,
    pub contacts: Vec<SightContact>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SightContact {
    pub bearing_degrees: i32,
    pub approximate_range_metres: u32,
}
