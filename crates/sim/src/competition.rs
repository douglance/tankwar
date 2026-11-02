//! Rating and lifecycle rules independent of transport and persistence.
use std::collections::{BTreeMap, BTreeSet};
use tankwar_protocol::MatchState;
use crate::{expected_score, team_mean};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Mode { OneOnOne, TwoOnTwo }
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Outcome { FirstWins, SecondWins, Draw }
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RatingChange { pub agent_id: String, pub before: f64, pub after: f64 }
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LadderEntry { pub agent_id: String, pub rating: f64, pub games: u32, pub provisional: bool }
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Ratings {
    ratings: BTreeMap<String, f64>,
    games: BTreeMap<String, u32>,
    season: String,
    results: BTreeMap<String, (Mode, Vec<String>, Vec<String>, Outcome, Vec<RatingChange>)>,
}
fn rating_key(mode: Mode, agent: &str) -> String { format!("{}:{agent}", match mode { Mode::OneOnOne => 1, Mode::TwoOnTwo => 2 }) }
impl Ratings {
    pub fn for_season(season: impl Into<String>) -> Result<Self, &'static str> {
        let season = season.into();
        if season.is_empty() { return Err("invalid_season"); }
        Ok(Self { season, ..Self::default() })
    }
    pub fn season(&self) -> &str { &self.season }
    pub fn games(&self, mode: Mode, agent: &str) -> u32 { *self.games.get(&rating_key(mode, agent)).unwrap_or(&0) }
    pub fn provisional(&self, mode: Mode, agent: &str) -> bool { self.games(mode, agent) < 10 }
    pub fn rating(&self, mode: Mode, agent: &str) -> f64 { *self.ratings.get(&rating_key(mode, agent)).unwrap_or(&1500.0) }
    pub fn leaderboard(&self, mode: Mode) -> Vec<LadderEntry> {
        let prefix = match mode { Mode::OneOnOne => "1:", Mode::TwoOnTwo => "2:" };
        let mut entries: Vec<_> = self.ratings.iter().filter_map(|(key, rating)| {
            let agent = key.strip_prefix(prefix)?;
            Some(LadderEntry { agent_id: agent.into(), rating: *rating, games: self.games(mode, agent), provisional: self.provisional(mode, agent) })
        }).collect();
        entries.sort_by(|a, b| b.rating.total_cmp(&a.rating).then_with(|| a.agent_id.cmp(&b.agent_id)));
        entries
    }
    /// The owner must persist the entire mutation atomically, including the match id.
    pub fn apply(&mut self, match_id: &str, mode: Mode, first: &[String], second: &[String], outcome: Outcome) -> Result<Vec<RatingChange>, &'static str> {
        if match_id.is_empty() { return Err("invalid_match_id"); }
        if let Some((old_mode, old_first, old_second, old_outcome, result)) = self.results.get(match_id) {
            if *old_mode != mode || old_first != first || old_second != second || *old_outcome != outcome { return Err("result_conflict"); }
            return Ok(result.clone());
        }
        let size = match mode { Mode::OneOnOne => 1, Mode::TwoOnTwo => 2 };
        if first.len() != size || second.len() != size { return Err("invalid_team_size"); }
        let mut unique = BTreeSet::new();
        if first.iter().chain(second).any(|id| id.is_empty() || !unique.insert(id)) { return Err("duplicate_agent"); }
        let side_rating = |team: &[String]| {
            if size == 1 { self.rating(mode, &team[0]) }
            else { team_mean(&[self.rating(mode, &team[0]), self.rating(mode, &team[1])]) }
        };
        let a = side_rating(first); let b = side_rating(second);
        let score = match outcome { Outcome::FirstWins => 1.0, Outcome::SecondWins => 0.0, Outcome::Draw => 0.5 };
        let changes: Vec<_> = first.iter().map(|agent| (agent, b, score)).chain(second.iter().map(|agent| (agent, a, 1.0-score))).map(|(agent, opponent, own_score)| {
            let before = self.rating(mode, agent);
            RatingChange { agent_id: agent.clone(), before, after: before + 32.0 * (own_score - expected_score(before, opponent)) }
        }).collect();
        for change in &changes {
            let key = rating_key(mode, &change.agent_id);
            self.ratings.insert(key.clone(), change.after);
            *self.games.entry(key).or_default() += 1;
        }
        self.results.insert(match_id.into(), (mode, first.to_vec(), second.to_vec(), outcome, changes.clone()));
        Ok(changes)
    }
}

pub fn transition(from: MatchState, to: MatchState) -> Result<MatchState, &'static str> {
    use MatchState::*;
    if matches!((&from, &to), (Queued, Assigned | Cancelled) | (Assigned, Ready | Cancelled | Aborted) | (Ready, Running | Cancelled | Aborted) | (Running, Finished | Aborted)) { Ok(to) }
    else { Err("invalid_transition") }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ids(values: &[&str]) -> Vec<String> { values.iter().map(|x| x.to_string()).collect() }
    #[test] fn teams_change_individual_mode_ratings_once() {
        let mut r = Ratings::default(); let a = ids(&["a", "b"]); let b = ids(&["c", "d"]);
        let result = r.apply("m1", Mode::TwoOnTwo, &a, &b, Outcome::FirstWins).unwrap();
        assert_eq!(result.iter().map(|c| c.after).collect::<Vec<_>>(), vec![1516., 1516., 1484., 1484.]);
        assert_eq!(r.apply("m1", Mode::TwoOnTwo, &a, &b, Outcome::FirstWins).unwrap(), result);
        assert_eq!(r.rating(Mode::OneOnOne, "a"), 1500.);
        assert_eq!(r.apply("m1", Mode::TwoOnTwo, &a, &b, Outcome::Draw), Err("result_conflict"));
    }
    #[test] fn seasons_and_provisional_status_are_separate_and_idempotent() {
        let mut season = Ratings::for_season("1944").unwrap();
        let other = Ratings::for_season("1945").unwrap();
        let a=ids(&["a"]); let b=ids(&["b"]);
        for i in 0..9 { season.apply(&format!("m{i}"), Mode::OneOnOne, &a, &b, Outcome::Draw).unwrap(); }
        assert!(season.provisional(Mode::OneOnOne, "a"));
        season.apply("m9", Mode::OneOnOne, &a, &b, Outcome::Draw).unwrap();
        season.apply("m9", Mode::OneOnOne, &a, &b, Outcome::Draw).unwrap();
        assert_eq!(season.games(Mode::OneOnOne, "a"), 10);
        assert!(!season.provisional(Mode::OneOnOne, "a"));
        assert!(season.provisional(Mode::TwoOnTwo, "a"));
        assert_eq!(other.games(Mode::OneOnOne, "a"), 0);
        assert_eq!(other.season(), "1945");
    }
    #[test] fn season_and_result_idempotency_survive_storage_roundtrip() {
        let mut r = Ratings::for_season("season-1").unwrap();
        let a=ids(&["a"]); let b=ids(&["b"]);
        r.apply("match-1", Mode::OneOnOne, &a, &b, Outcome::FirstWins).unwrap();
        let bytes=serde_json::to_vec(&r).unwrap();
        let mut restored: Ratings=serde_json::from_slice(&bytes).unwrap();
        restored.apply("match-1", Mode::OneOnOne, &a, &b, Outcome::FirstWins).unwrap();
        assert_eq!(restored.season(), "season-1");
        assert_eq!(restored.rating(Mode::OneOnOne, "a"), 1516.0);
        assert_eq!(restored.games(Mode::OneOnOne, "a"), 1);
    }
    #[test] fn leaderboard_orders_ratings_and_marks_provisional_by_mode() {
        let mut ratings = Ratings::for_season("s1").unwrap();
        ratings.apply("m1", Mode::OneOnOne, &ids(&["z"]), &ids(&["a"]), Outcome::FirstWins).unwrap();
        let entries = ratings.leaderboard(Mode::OneOnOne);
        assert_eq!(entries.iter().map(|e| e.agent_id.as_str()).collect::<Vec<_>>(), vec!["z", "a"]);
        assert_eq!(entries[0].rating, 1516.0); assert_eq!(entries[0].games, 1); assert!(entries[0].provisional);
        assert!(ratings.leaderboard(Mode::TwoOnTwo).is_empty());
    }
    #[test] fn each_teammate_is_rated_against_the_opposing_mean() {
        let mut ratings = Ratings::for_season("s1").unwrap();
        let a=ids(&["a", "b"]); let opponents=ids(&["c", "d"]);
        ratings.ratings.insert(rating_key(Mode::TwoOnTwo, "a"), 1700.0);
        ratings.ratings.insert(rating_key(Mode::TwoOnTwo, "b"), 1300.0);
        let result=ratings.apply("unequal", Mode::TwoOnTwo, &a, &opponents, Outcome::FirstWins).unwrap();
        assert!((result[0].after - (1700.0 + 32.0 * (1.0 - 1.0 / (1.0 + 10_f64.powf(-0.5))))).abs() < 1e-9);
        assert!((result[1].after - (1300.0 + 32.0 * (1.0 - 1.0 / (1.0 + 10_f64.powf(0.5))))).abs() < 1e-9);
        assert_eq!(ratings.leaderboard(Mode::TwoOnTwo).len(), 4);
    }
    #[test] fn invalid_teams_do_not_change_ratings() {
        let mut r = Ratings::default();
        assert_eq!(r.apply("m", Mode::OneOnOne, &ids(&["a"]), &ids(&["a"]), Outcome::Draw), Err("duplicate_agent"));
        assert_eq!(r.rating(Mode::OneOnOne, "a"), 1500.);
    }
    #[test] fn lifecycle_rejects_skips_and_terminal_transitions() {
        assert_eq!(transition(MatchState::Queued, MatchState::Running), Err("invalid_transition"));
        assert_eq!(transition(MatchState::Running, MatchState::Aborted), Ok(MatchState::Aborted));
        assert_eq!(transition(MatchState::Finished, MatchState::Running), Err("invalid_transition"));
    }
}
