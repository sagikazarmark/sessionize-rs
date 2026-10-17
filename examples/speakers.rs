//! Lists the speakers of an event with their links and sessions, top speakers
//! first.
//!
//! ```sh
//! cargo run --example speakers                    # Sessionize's demo event
//! cargo run --example speakers -- <endpoint-id>
//! ```

use std::env;
use std::error::Error;

use sessionize::{Client, EndpointId};

/// Endpoint of Sessionize's demo event.
const DEMO_ENDPOINT: &str = "jl4ktls0";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let id: EndpointId = env::args()
        .nth(1)
        .as_deref()
        .unwrap_or(DEMO_ENDPOINT)
        .parse()?;

    let event = Client::new().event(&id).await?;

    let mut speakers: Vec<_> = event.speakers().collect();
    speakers.sort_by_key(|speaker| (!speaker.is_top_speaker(), speaker.full_name()));

    for speaker in speakers {
        let star = if speaker.is_top_speaker() { " ★" } else { "" };
        println!("{}{star}", speaker.full_name());

        if let Some(tag_line) = speaker.tag_line().filter(|t| !t.trim().is_empty()) {
            println!("  {}", tag_line.trim());
        }
        for link in speaker.links() {
            println!("  {}: {}", link.title, link.url);
        }
        for session in speaker.sessions() {
            let when = session.starts_at().map_or_else(
                || "not scheduled".to_owned(),
                |start| start.naive().format("%a %H:%M").to_string(),
            );
            let room = session.room().map_or("TBA", |room| room.name.as_str());

            println!("  - {} ({when}, {room})", session.title());
        }
        println!();
    }

    Ok(())
}
