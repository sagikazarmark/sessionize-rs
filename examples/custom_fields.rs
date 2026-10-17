//! Reads the custom fields of Sessionize's demo event, untyped and into a
//! typed struct.
//!
//! The demo event has four session categories ("Session format", "Track",
//! "Level", "Language") and one session question ("Demo", a percentage).
//! Adapt `Talk` to the custom fields of your own event.
//!
//! ```sh
//! cargo run --example custom_fields
//! ```

use std::error::Error;

use serde::Deserialize;
use serde::de::IgnoredAny;
use sessionize::{Client, EndpointId};

/// Endpoint of Sessionize's demo event.
const DEMO_ENDPOINT: &str = "jl4ktls0";

/// The custom fields of a session.
#[derive(Debug, Deserialize)]
struct Talk {
    /// A single-choice category, as an enum.
    #[serde(rename = "Session format")]
    format: Format,
    /// A single-choice category, as a string.
    #[serde(rename = "Track")]
    track: String,
    #[serde(rename = "Level")]
    level: Level,
    /// Categories can also be read as a list of the selected items.
    #[serde(rename = "Language", default)]
    languages: Vec<String>,
    /// A percentage question. Unanswered questions are `None`.
    #[serde(rename = "Demo")]
    demo: Option<u8>,
}

#[derive(Debug, Deserialize)]
enum Format {
    #[serde(rename = "Lightning talk")]
    LightningTalk,
    Session,
    Workshop,
}

#[derive(Debug, Deserialize)]
enum Level {
    #[serde(rename = "Introductory and overview")]
    Introductory,
    Intermediate,
    Advanced,
    Expert,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let id: EndpointId = DEMO_ENDPOINT.parse()?;
    let event = Client::new().event(&id).await?;

    // Untyped: the categories and answers of a session as sent by the API.
    if let Some(session) = event.sessions().find(|s| !s.is_service_session()) {
        println!("{}", session.title());

        for selection in session.categories() {
            let items: Vec<_> = selection.items().iter().map(|i| i.name.as_str()).collect();
            println!("  {}: {}", selection.category().title, items.join(", "));
        }
        for answer in session.answers() {
            println!("  {}: {:?}", answer.question().question, answer.value()?);
        }
        println!();
    }

    // Typed: deserialize the custom fields of every regular session at once.
    // `IgnoredAny` skips the speakers' custom fields.
    let typed = event.into_typed::<Talk, IgnoredAny>()?;

    for (session, talk) in typed.sessions() {
        let demo = talk.demo.map_or("-".to_owned(), |demo| format!("{demo}%"));

        println!(
            "{:<20} {:<14} {:<11} {:<13} {:<11} {demo}",
            session.title(),
            format!("{:?}", talk.format),
            talk.track,
            format!("{:?}", talk.level),
            talk.languages.join(", "),
        );
    }

    Ok(())
}
