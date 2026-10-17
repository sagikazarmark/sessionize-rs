//! Prints the schedule of an event, grouped by day and time slot.
//!
//! ```sh
//! cargo run --example schedule                    # Sessionize's demo event
//! cargo run --example schedule -- <endpoint-id>
//! ```

use std::env;
use std::error::Error;

use sessionize::event::Speaker;
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

    // Fetches the All and GridSmart views and combines them.
    let event = Client::new().event(&id).await?;

    for day in event.schedule() {
        println!("{}", day.date().format("%A, %B %-d, %Y"));

        for slot in day.time_slots() {
            for session in slot.sessions() {
                let start = slot.start().time().format("%H:%M");
                let end = session
                    .ends_at()
                    .map(|end| end.time().format("%H:%M").to_string())
                    .unwrap_or_default();
                let room = session.room().map_or("TBA", |room| room.name.as_str());
                let speakers: Vec<_> = session.speakers().map(Speaker::full_name).collect();

                print!("  {start}–{end} [{room}] {}", session.title());
                if !speakers.is_empty() {
                    print!(" ({})", speakers.join(", "));
                }
                println!();
            }
        }
        println!();
    }

    let unscheduled = event.sessions().filter(|s| s.starts_at().is_none()).count();
    if unscheduled > 0 {
        println!("{unscheduled} sessions aren't scheduled yet.");
    }

    Ok(())
}
