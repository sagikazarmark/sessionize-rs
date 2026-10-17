# sessionize

[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/sagikazarmark/sessionize-rs/ci.yaml?style=flat-square)](https://github.com/sagikazarmark/sessionize-rs/actions/workflows/ci.yaml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/sagikazarmark/sessionize-rs/badge?style=flat-square)](https://securityscorecards.dev/viewer/?uri=github.com/sagikazarmark/sessionize-rs)
[![crates.io](https://img.shields.io/crates/v/sessionize?style=flat-square)](https://crates.io/crates/sessionize)
[![docs.rs](https://img.shields.io/docsrs/sessionize?style=flat-square)](https://docs.rs/sessionize)

Typed Rust client for the read-only [Sessionize API](https://sessionize.com/playbook/api):
fetch an event's sessions, speakers, schedule and custom fields as Rust types.

## Installation

```sh
cargo add sessionize
cargo add tokio --features macros,rt-multi-thread
```

The client is async and needs a [Tokio](https://tokio.rs) runtime.

## Usage

Create a JSON endpoint on your event's API / Embed page in Sessionize
([instructions](https://sessionize.com/playbook/api)), then fetch the event with its ID
(`jl4ktls0` is Sessionize's public demo event):

```rust,no_run
use sessionize::{Client, EndpointId};

# #[tokio::main]
# async fn main() -> Result<(), Box<dyn std::error::Error>> {
let id: EndpointId = "jl4ktls0".parse()?;
let event = Client::new().event(&id).await?;

for day in event.schedule() {
    println!("{}", day.date());

    for slot in day.time_slots() {
        for session in slot.sessions() {
            let room = session.room().map_or("TBA", |room| room.name.as_str());
            let speakers: Vec<_> = session.speakers().map(|s| s.full_name()).collect();

            println!("  {} [{room}] {} ({})", slot.start().time(), session.title(), speakers.join(", "));
        }
    }
}
# Ok(())
# }
```

The endpoint ID gives access to the endpoint's data without authentication, so treat it as confidential.

### Views

Each endpoint serves the same data in several shapes ("views"). `Client::endpoint` fetches them individually:

| View | Method | Type |
|---|---|---|
| `All` | `all()` | `All`: everything, with references by ID |
| `GridSmart` | `grid()` | `Vec<grid::Day>`: schedule grid by day, room and time slot |
| `Sessions` | `sessions()` | `Vec<SessionGroup>`: sessions grouped by the endpoint's settings |
| `Speakers` | `speakers()` | `Vec<Speaker>`: speakers with their sessions |
| `SpeakerWall` | `speaker_wall()` | `Vec<WallSpeaker>`: speakers for a speaker wall |

`Client::event` combines the `All` and `GridSmart` views into an `Event`, because the `All` view may leave out service sessions (breaks, lunch) that the grid has.
The `All` view also leaves out the original names of uploaded files; `Endpoint::event_with(EventOptions::default().with_file_names(true))` fetches the `Sessions` and `Speakers` views as well to fill them in.

Which fields each view includes depends on the endpoint's settings in Sessionize.

### Times

Schedule times are local wall-clock times at the event, unless the endpoint has "Use UTC timezone for schedule" enabled.
`ScheduleTime` represents both; the API doesn't say which time zone the event is in.

### Custom fields

Questions and categories you define in Sessionize are available as untyped answers (`Session::answers` on an `Event`), or deserialized into your own types with serde:

```rust,no_run
use serde::Deserialize;
use sessionize::{Client, EndpointId, FileAnswer};

#[derive(Deserialize)]
struct Talk {
    #[serde(rename = "Level")]
    level: String,
    #[serde(rename = "Track", default)]
    tracks: Vec<String>,
    #[serde(rename = "Slides")]
    slides: Option<FileAnswer>,
    #[serde(rename = "Expected attendees")]
    attendees: Option<u32>,
}

# #[tokio::main]
# async fn main() -> Result<(), Box<dyn std::error::Error>> {
let id: EndpointId = "jl4ktls0".parse()?;
let event = Client::new().event(&id).await?;

for session in event.sessions().filter(|s| !s.is_service_session()) {
    let talk: Talk = session.custom_fields()?;
}
# Ok(())
# }
```

- Fields are matched by question text or category title, or by ID (`#[serde(rename = "143673")]`).
- Answers are parsed into the field's type: numbers, booleans (checkboxes), URLs, enums or strings.
- Unanswered questions are missing: use `Option` or `#[serde(default)]`.
  A misspelled field name looks the same as an unanswered question, so double-check names of optional fields.
- Categories deserialize into a `Vec` of the selected item names, or into a single value if exactly one item is selected.
- File uploads deserialize into `FileAnswer` or a `String` URL.
- `#[serde(flatten)]` and untagged enums get values without conversion (strings, or numbers and booleans for integer, percentage and checkbox questions).

`Event::into_typed` deserializes all sessions and speakers at once and fails on the first mismatch.

## Features

- `rustls` (default): HTTP client using rustls.
- `native-tls`: HTTP client using the platform's TLS library.
- `client`: HTTP client without a TLS backend; enable a TLS feature of `reqwest` yourself, since Sessionize requires HTTPS.

Without any of them, the crate only provides the data types: fetch the JSON yourself (`EndpointId::view_url` builds the URLs) and deserialize it with serde.

## Examples

The [examples](https://github.com/sagikazarmark/sessionize-rs/tree/main/examples) use Sessionize's demo event unless you pass an endpoint ID:

| Example | Shows |
|---|---|
| [`schedule`](https://github.com/sagikazarmark/sessionize-rs/blob/main/examples/schedule.rs) | The schedule by day and time slot |
| [`speakers`](https://github.com/sagikazarmark/sessionize-rs/blob/main/examples/speakers.rs) | Speakers with their links and sessions |
| [`custom_fields`](https://github.com/sagikazarmark/sessionize-rs/blob/main/examples/custom_fields.rs) | Categories and questions, untyped and as a typed struct |
| [`offline`](https://github.com/sagikazarmark/sessionize-rs/blob/main/examples/offline.rs) | Building an `Event` from saved JSON files, without the HTTP client |

```sh
cargo run --example schedule
cargo run --example speakers -- <endpoint-id>
cargo run --example offline --no-default-features -- <directory>
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
