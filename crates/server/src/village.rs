//! Persistent allocation of unranked public village fights.
use serde::{Deserialize, Serialize};
use tankwar_protocol::MatchState;

#[derive(Serialize, Deserialize)]
pub struct Directory { ids: Vec<String>, next: u64 }

impl Default for Directory {
    fn default() -> Self { Self { ids: vec!["village".into()], next: 1 } }
}

impl Directory {
    pub fn active(&self) -> &str { self.ids.last().expect("village directory is nonempty") }
    pub fn ids(&self) -> &[String] { &self.ids }
    pub fn advance_if_terminal(&mut self, state: MatchState) -> Result<bool, &'static str> {
        if !matches!(state, MatchState::Finished | MatchState::Cancelled | MatchState::Aborted) { return Ok(false); }
        let next = self.next.checked_add(1).ok_or("village_id_exhausted")?;
        self.ids.push(format!("village-{}", self.next));
        self.next = next;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_fight_allocates_once_and_retains_replays_across_restart() {
        let mut directory = Directory::default();
        assert!(!directory.advance_if_terminal(MatchState::Running).unwrap());
        assert!(directory.advance_if_terminal(MatchState::Finished).unwrap());
        let saved = serde_json::to_string(&directory).unwrap();
        let mut restarted: Directory = serde_json::from_str(&saved).unwrap();
        assert_eq!(restarted.active(), "village-1");
        assert!(!restarted.advance_if_terminal(MatchState::Running).unwrap());
        assert_eq!(restarted.ids(), &["village", "village-1"]);
        assert!(restarted.advance_if_terminal(MatchState::Aborted).unwrap());
        assert_eq!(restarted.ids(), &["village", "village-1", "village-2"]);
    }

    #[test]
    fn each_named_village_is_unranked_and_has_its_own_ledger() {
        let first = crate::match_runtime::DurableMatch::village_named("village-1", 0).unwrap();
        let second = crate::match_runtime::DurableMatch::village_named("village-2", 0).unwrap();
        assert!(!first.ranked && !second.ranked);
        assert_eq!(first.ledger.match_id, "village-1");
        assert_eq!(second.ledger.match_id, "village-2");
    }
}
