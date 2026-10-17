//! Builds an event from views saved as JSON files, without the HTTP client.
//!
//! Useful with your own HTTP stack, for caching, or for static site
//! generators. Download the views first (`EndpointId::view_url` builds the
//! same URLs):
//!
//! ```sh
//! mkdir event
//! for view in All GridSmart; do
//!   curl -s "https://sessionize.com/api/v2/<endpoint-id>/view/$view" > "event/$view.json"
//! done
//! cargo run --example offline --no-default-features -- event
//! ```
//!
//! Without arguments, it reads the demo event saved in the repository's tests.

use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use sessionize::{All, Event, View, grid};

fn read<T: DeserializeOwned>(dir: &Path, view: View) -> Result<T, Box<dyn Error>> {
    let path = dir.join(format!("{view}.json"));
    let json = fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;

    Ok(serde_json::from_str(&json)?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let dir = env::args().nth(1).map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sessionize-demo"),
        PathBuf::from,
    );

    let all: All = read(&dir, View::All)?;
    let days: Vec<grid::Day> = read(&dir, View::GridSmart)?;

    // The same as `Client::event` does after fetching the views.
    let mut event = Event::from(all);
    event.merge_grid(&days);

    println!(
        "{} sessions, {} speakers, {} rooms",
        event.sessions().len(),
        event.speakers().len(),
        event.rooms().len()
    );

    for day in event.schedule() {
        println!("\n{}", day.date());
        for room in day.rooms() {
            let sessions: Vec<_> = day
                .sessions()
                .filter(|session| session.room().is_some_and(|r| r.id == room.id))
                .collect();
            let start = sessions.first().and_then(|s| s.starts_at());
            let end = sessions
                .iter()
                .filter_map(|s| s.ends_at())
                .max_by_key(|t| t.naive());
            let span = match (start, end) {
                (Some(start), Some(end)) => {
                    format!(
                        "{}–{}",
                        start.time().format("%H:%M"),
                        end.time().format("%H:%M")
                    )
                }
                _ => String::new(),
            };

            println!("  {}: {} sessions {span}", room.name, sessions.len());
        }
    }

    Ok(())
}
