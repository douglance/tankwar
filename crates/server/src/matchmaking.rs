//! Deterministic queue state. Persistence and authorization belong to its owner.
use std::collections::{BTreeMap, VecDeque};
use serde::{Deserialize, Serialize};
use tankwar_sim::competition::Mode;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assignment { pub match_id: String, pub mode: Mode, pub first: Vec<String>, pub second: Vec<String> }
#[derive(Default, Serialize, Deserialize)]
pub struct Queue {
    one: VecDeque<String>,
    two: VecDeque<String>,
    assignments: BTreeMap<String, Assignment>,
    next_match: u64,
    #[serde(default)] matches: BTreeMap<String, Mode>,
}
impl Queue {
    pub fn join(&mut self, agent: &str, mode: Mode) -> Result<Option<Assignment>, &'static str> {
        if agent.is_empty() || agent.len() > 128 { return Err("invalid_agent"); }
        if let Some(assignment) = self.assignments.get(agent) { return Ok(Some(assignment.clone())); }
        let in_one = self.one.iter().any(|v| v == agent);
        let in_two = self.two.iter().any(|v| v == agent);
        if (mode == Mode::OneOnOne && in_one) || (mode == Mode::TwoOnTwo && in_two) { return Ok(None); }
        if in_one || in_two { return Err("already_queued"); }
        let queue = match mode { Mode::OneOnOne => &mut self.one, Mode::TwoOnTwo => &mut self.two };
        queue.push_back(agent.to_owned());
        let count = match mode { Mode::OneOnOne => 2, Mode::TwoOnTwo => 4 };
        if queue.len() < count { return Ok(None); }
        let next = self.next_match.checked_add(1).ok_or("match_id_exhausted")?;
        let participants: Vec<_> = queue.drain(..count).collect();
        let half = count / 2;
        let assignment = Assignment { match_id: format!("match-{next}"), mode, first: participants[..half].to_vec(), second: participants[half..].to_vec() };
        for agent in participants { self.assignments.insert(agent, assignment.clone()); }
        self.matches.insert(assignment.match_id.clone(), mode);
        self.next_match = next;
        Ok(Some(assignment))
    }
    pub fn matches(&self) -> &BTreeMap<String, Mode> { &self.matches }
    pub fn assignment(&self, agent: &str) -> Option<&Assignment> { self.assignments.get(agent) }
    /// Called only after the match owner has made the assignment terminal.
    pub fn release(&mut self, match_id: &str) {
        self.assignments.retain(|_, assignment| assignment.match_id != match_id);
    }
    pub fn leave(&mut self, agent: &str) -> bool {
        let before = self.one.len() + self.two.len();
        self.one.retain(|v| v != agent); self.two.retain(|v| v != agent);
        before != self.one.len() + self.two.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn modes_stay_separate_and_assign_unique_agents_once() {
        let mut q=Queue::default();
        assert_eq!(q.join("a", Mode::OneOnOne), Ok(None));
        assert_eq!(q.join("a", Mode::OneOnOne), Ok(None));
        assert_eq!(q.join("a", Mode::TwoOnTwo), Err("already_queued"));
        assert_eq!(q.join("b", Mode::TwoOnTwo), Ok(None));
        assert_eq!(q.join("c", Mode::TwoOnTwo), Ok(None));
        let m=q.join("d", Mode::OneOnOne).unwrap().unwrap();
        assert_eq!(m.first, vec!["a"]); assert_eq!(m.second, vec!["d"]);
        assert_eq!(q.join("a", Mode::OneOnOne).unwrap(), Some(m.clone()));
        let restored: Queue=serde_json::from_slice(&serde_json::to_vec(&q).unwrap()).unwrap();
        assert_eq!(restored.assignment("d"), Some(&m));
        let mut restored = restored; restored.release(&m.match_id);
        assert_eq!(restored.matches().get(&m.match_id), Some(&Mode::OneOnOne));
        assert!(restored.join("a", Mode::OneOnOne).unwrap().is_none());
    }
    #[test] fn forms_two_sides_of_two_and_cancels_waiting_ticket() {
        let mut q=Queue::default();
        for a in ["a","b","c"] { assert!(q.join(a, Mode::TwoOnTwo).unwrap().is_none()); }
        assert!(q.leave("c")); assert!(!q.leave("c"));
        assert!(q.join("d", Mode::TwoOnTwo).unwrap().is_none());
        let m=q.join("e", Mode::TwoOnTwo).unwrap().unwrap();
        assert_eq!(m.first, vec!["a","b"]); assert_eq!(m.second, vec!["d","e"]);
    }
}
