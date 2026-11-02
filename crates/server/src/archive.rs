//! Preparation for atomic persistence of match state and append-only history.
use tankwar_protocol::{Event, EventKind};
use tankwar_sim::world::WorldEvent;
use crate::match_runtime::{DurableMatch, Finished, RecordedWorldEvent};

pub struct ArchiveBatch {
    pub commands: Vec<Event>,
    pub world_events: Vec<(u64, RecordedWorldEvent)>,
}

pub fn command_key(id: &str) -> String {
    use std::fmt::Write;
    let mut key = String::from("command-id/");
    for byte in id.as_bytes() { write!(&mut key, "{byte:02x}").unwrap(); }
    key
}

pub fn prepare(game: &mut DurableMatch) -> ArchiveBatch {
    // Preserve the result independently of the archived event list, including legacy records.
    if game.finished.is_none() {
        game.finished = game.world_events.iter().find_map(|entry| match &entry.event {
            WorldEvent::Finished { winner } => Some(Finished { winner: *winner }), _ => None,
        });
    }
    let commands = game.ledger.take_history();
    let world_events = std::mem::take(&mut game.world_events).into_iter().map(|entry| {
        game.archived_world_events += 1;
        (game.archived_world_events, entry)
    }).collect();
    ArchiveBatch { commands, world_events }
}

pub fn event_command(event: &Event) -> Option<&tankwar_protocol::Command> {
    match &event.event { EventKind::Accepted { command } => Some(command), _ => None }
}

pub fn complete_commands(events: &[Event], version: u64) -> bool {
    events.len() as u64 == version && events.iter().enumerate().all(|(index, event)| event.sequence == index as u64 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn key_encoding_is_unambiguous() {
        assert_ne!(command_key("a/"), command_key("a2f"));
        assert_ne!(command_key("é"), command_key("e"));
    }
    #[test]
    fn missing_or_duplicate_command_sequences_are_rejected() {
        use tankwar_protocol::EventKind;
        let event = |sequence| Event { sequence, tick: 0, event: EventKind::Rejected { command_id: "test".into(), reason: "test".into() } };
        assert!(complete_commands(&[event(1), event(2)], 2));
        assert!(!complete_commands(&[event(1)], 2));
        assert!(!complete_commands(&[event(1), event(1)], 2));
    }
}
