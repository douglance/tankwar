//! Deterministic, server-owned simulation primitives.
pub mod clock;
pub mod lesmont;
pub mod crew;
use std::collections::BTreeMap;
use tankwar_protocol::{MATCH_TICKS, MatchState};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tank {
    pub fuel_ml: u64,
    /// Consumption per tick is an explicit scenario approximation until sourced.
    pub idle_fuel_ml_per_tick: u64,
    pub engine_running: bool,
    pub ammunition: BTreeMap<String, u32>,
    pub chamber: Option<String>,
    /// Unsourced scenario approximation; one loader can handle one round at a time.
    pub reload_ticks: u32,
    pub loading: Option<(String, u32)>,
    pub throttle: i16,
    pub steering: i16,
    pub drive_expires_at: Option<u64>,
}
impl Tank {
    pub fn drive(&mut self, tick: u64, throttle: i16, steering: i16, ttl: u32) -> Result<(), &'static str> {
        if ttl == 0 || !(-1000..=1000).contains(&throttle) || !(-1000..=1000).contains(&steering) { return Err("invalid_drive"); }
        self.throttle = throttle;
        self.steering = steering;
        self.drive_expires_at = Some(tick.saturating_add(u64::from(ttl)));
        Ok(())
    }
    pub fn neutralize(&mut self) { self.throttle = 0; self.steering = 0; self.drive_expires_at = None; }
    pub fn load(&mut self, round: &str) -> Result<(), &'static str> {
        if self.chamber.is_some() { return Err("chamber_occupied"); }
        if self.loading.is_some() { return Err("loader_busy"); }
        let stock = self.ammunition.get_mut(round).ok_or("round_unavailable")?;
        if *stock == 0 { return Err("round_unavailable"); }
        *stock -= 1;
        self.loading = Some((round.to_owned(), self.reload_ticks.max(1)));
        Ok(())
    }
    pub fn fire(&mut self) -> Result<String, &'static str> { self.chamber.take().ok_or("chamber_empty") }
    pub fn step(&mut self, tick: u64) {
        if let Some((_, remaining)) = self.loading.as_mut() { *remaining -= 1; }
        if self.loading.as_ref().is_some_and(|(_, remaining)| *remaining == 0) {
            self.chamber = self.loading.take().map(|(round, _)| round);
        }
        if self.drive_expires_at.is_some_and(|expiry| tick >= expiry) { self.neutralize(); }
        if self.engine_running {
            // Placeholder load model: full drive or steering adds one idle-rate
            // equivalent. Actual fuel curves depend on the sourced vehicle.
            let load = self.throttle.unsigned_abs().max(self.steering.unsigned_abs()) as u64;
            let extra = self.idle_fuel_ml_per_tick.saturating_mul(load).div_ceil(1000);
            self.fuel_ml = self.fuel_ml.saturating_sub(self.idle_fuel_ml_per_tick.saturating_add(extra));
            if self.fuel_ml == 0 { self.engine_running = false; self.neutralize(); }
        }
    }
}

pub struct Match { pub state: MatchState, pub tick: u64, pub tanks: Vec<Tank> }
impl Match {
    pub fn step(&mut self) {
        if self.state != MatchState::Running { return; }
        self.tick += 1;
        for tank in &mut self.tanks { tank.step(self.tick); }
        if self.tick >= MATCH_TICKS { self.state = MatchState::Finished; }
    }
}

pub fn expected_score(own: f64, opponent: f64) -> f64 { 1.0 / (1.0 + 10_f64.powf((opponent - own) / 400.0)) }
pub fn updated_rating(own: f64, opponent: f64, score: f64) -> f64 { own + 32.0 * (score - expected_score(own, opponent)) }
pub fn team_mean(team: &[f64; 2]) -> f64 { (team[0] + team[1]) / 2.0 }

#[cfg(test)]
mod tests {
    use super::*;
    fn tank() -> Tank { Tank { fuel_ml: 2, idle_fuel_ml_per_tick: 1, engine_running: true, ammunition: BTreeMap::from([("ap".into(), 1)]), chamber: None, reload_ticks: 2, loading: None, throttle: 0, steering: 0, drive_expires_at: None } }
    #[test] fn resources_are_conserved() { let mut t = tank(); t.load("ap").unwrap(); assert_eq!(t.ammunition["ap"], 0); assert!(t.load("ap").is_err()); assert!(t.fire().is_err()); t.step(1); assert!(t.fire().is_err()); t.step(2); assert_eq!(t.fire().unwrap(), "ap"); assert!(t.fire().is_err()); t.step(1); t.step(2); assert_eq!(t.fuel_ml, 0); assert!(!t.engine_running); }
    #[test] fn orders_expire() { let mut t = tank(); t.fuel_ml = 10; t.drive(10, 500, -100, 2).unwrap(); t.step(11); assert_eq!(t.throttle, 500); t.step(12); assert_eq!(t.throttle, 0); }
    #[test] fn engine_load_consumes_additional_fuel_and_cannot_underflow() {
        let mut idle = tank(); idle.fuel_ml = 10;
        let mut driving = idle.clone(); driving.drive(0, 1000, 0, 5).unwrap();
        let mut steering = idle.clone(); steering.drive(0, 0, -1000, 5).unwrap();
        idle.step(1); driving.step(1); steering.step(1);
        assert_eq!((idle.fuel_ml, driving.fuel_ml, steering.fuel_ml), (9, 8, 8));
        driving.fuel_ml = 1; driving.step(2);
        assert_eq!(driving.fuel_ml, 0); assert!(!driving.engine_running); assert_eq!(driving.throttle, 0);
    }
    #[test] fn elo_reference() { assert_eq!(updated_rating(1500.0, 1500.0, 1.0), 1516.0); assert_eq!(updated_rating(1500.0, 1500.0, 0.5), 1500.0); assert_eq!(team_mean(&[1400.0, 1600.0]), 1500.0); }
    #[test] fn timeout_at_18000_ticks() { let mut m = Match { state: MatchState::Running, tick: MATCH_TICKS - 1, tanks: vec![] }; m.step(); assert_eq!(m.state, MatchState::Finished); }
}

pub mod competition;
pub mod geometry;
pub mod world;
