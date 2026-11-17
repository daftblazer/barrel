//! The story's progress: which events have played. Field tables hang people and objects on it
//! by a span of two events. Docs: docs/tables.md, "Ryza DX: story spans".

use std::collections::HashSet;

/// The events that have played, by name without the `EVENT_` of the game's enum (`MM00_010`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Story {
    done: HashSet<String>,
}

fn bare(event: &str) -> &str {
    event.strip_prefix("EVENT_").unwrap_or(event)
}

impl Story {
    /// A story where the events of `names` (comma separated, with or without `EVENT_`) have played.
    pub fn after(names: &str) -> Story {
        Story { done: names.split(',').map(str::trim).filter(|n| !n.is_empty()).map(|n| bare(n).to_ascii_uppercase()).collect() }
    }

    /// Marks `event` as played.
    pub fn finish(&mut self, event: &str) {
        self.done.insert(bare(event).to_ascii_uppercase());
    }

    pub fn done(&self, event: &str) -> bool {
        self.done.contains(bare(event))
    }

    /// Whether the story is within `span` (the game's test, `0x1405a2c20`): its start has
    /// played and its end has not. A span without a start has begun, unless `started` asks
    /// for one that names its start.
    pub fn within(&self, span: &Span, started: bool) -> bool {
        let begun = match &span.start {
            Some(e) => self.done(e),
            None => !started,
        };
        begun && !span.end.as_deref().is_some_and(|e| self.done(e))
    }
}

/// A stretch of the story between two events, either end open.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub start: Option<String>,
    pub end: Option<String>,
}

impl Span {
    /// A span from a row's two event names; an empty name is an open end.
    pub fn new(start: Option<&str>, end: Option<&str>) -> Span {
        let name = |e: Option<&str>| e.map(str::trim).filter(|e| !e.is_empty()).map(|e| bare(e).to_ascii_uppercase());
        Span { start: name(start), end: name(end) }
    }

    pub fn open(&self) -> bool {
        self.start.is_none() && self.end.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_span_runs_from_its_start_to_its_end() {
        let span = Span::new(Some("MM00_010"), Some("EVENT_MM00_080"));
        assert!(!Story::default().within(&span, false));
        assert!(Story::after("EVENT_MM00_010").within(&span, false));
        assert!(!Story::after("MM00_010, MM00_080").within(&span, true));
    }

    #[test]
    fn an_open_start_has_begun_unless_one_is_asked_for() {
        let span = Span::new(None, Some("MM01_111"));
        assert!(Story::default().within(&span, false) && !Story::default().within(&span, true));
        let mut story = Story::default();
        story.finish("EVENT_MM01_111");
        assert!(!story.within(&span, false));
    }
}
