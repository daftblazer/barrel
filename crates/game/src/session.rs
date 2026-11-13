//! A game session: what the game is doing from a new game on, one scene at a time. A scene is
//! an event playing or free play in a field; an event's end picks what follows (the story's
//! next event by `event_start::Chain`, else play goes on in the field the event left the
//! player in).

use crate::event_start::{new_game_event, Chain};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scene {
    /// An event playing, by its `EVENT_*` name.
    Event(String),
    /// Free play in a field, by its map number (the town's 0 to 11, the areas from 12).
    Field(u32),
}

pub struct Session {
    chain: Chain,
    scene: Scene,
    /// The field the player is in (the last an event changed to), if any yet.
    field: Option<u32>,
    /// Logy leads (the new game's choice).
    pub logy: bool,
}

impl Session {
    /// A new game with the lead chosen (`logy`), opening with its first event.
    pub fn new_game(chain: Chain, logy: bool) -> Session {
        Session { chain, scene: Scene::Event(new_game_event(logy, false).to_string()), field: None, logy }
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// The event playing has ended, having last changed to field `last_field` (if it changed
    /// field at all): the story's next event if one starts, else free play in the field.
    pub fn event_ended(&mut self, last_field: Option<u32>) -> &Scene {
        if let Some(f) = last_field {
            self.field = Some(f);
        }
        if let Scene::Event(name) = &self.scene {
            let name = name.clone();
            self.scene = match self.chain.after(&name) {
                Some(next) => Scene::Event(next.to_string()),
                // With no field yet the game would be nowhere; the Atelier (map 0) stands in.
                None => Scene::Field(self.field.unwrap_or(0)),
            };
        }
        &self.scene
    }
}
