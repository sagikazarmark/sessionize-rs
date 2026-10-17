//! Shared helpers for integration tests.

#![allow(
    dead_code,
    unreachable_pub,
    reason = "each test crate uses a subset of the helpers"
)]

use std::fs;
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use sessionize::{All, Event, SessionGroup, Speaker, grid};

/// Fixture directories.
pub const EVENTS: [&str; 3] = [KCD, KCD_CUSTOM_FIELDS, DEMO];

/// Real event, local times, no categories.
pub const KCD: &str = "kcd-budapest-2025";

/// Real event with one test answer per question type.
pub const KCD_CUSTOM_FIELDS: &str = "kcd-budapest-2025-custom-fields";

/// Sessionize demo event, UTC times.
pub const DEMO: &str = "sessionize-demo";

/// Views served by every endpoint.
pub const VIEWS: [&str; 5] = ["All", "GridSmart", "Sessions", "Speakers", "SpeakerWall"];

pub fn read(event: &str, view: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(event)
        .join(format!("{view}.json"));

    fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

pub fn fixture<T: DeserializeOwned>(event: &str, view: &str) -> T {
    serde_json::from_str(&read(event, view)).unwrap_or_else(|err| panic!("{event}/{view}: {err}"))
}

/// Builds an event like the client does, optionally with file names.
pub fn event(name: &str, file_names: bool) -> Event {
    let mut event = Event::from(fixture::<All>(name, "All"));
    event.merge_grid(&fixture::<Vec<grid::Day>>(name, "GridSmart"));
    if file_names {
        event.merge_sessions(&fixture::<Vec<SessionGroup>>(name, "Sessions"));
        event.merge_speakers(&fixture::<Vec<Speaker>>(name, "Speakers"));
    }

    event
}
