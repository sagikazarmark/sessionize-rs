//! Tests against the live Sessionize API, to detect API changes.
//!
//! Skipped by default. Run with `cargo test --test live -- --ignored`.

#![cfg(feature = "client")]

use sessionize::{Client, EndpointId, EventOptions};

/// Sessionize's demo event, and KCD Budapest 2025.
const ENDPOINTS: [&str; 2] = ["jl4ktls0", "dpesp6nz"];

#[tokio::test]
#[ignore = "requires network access"]
async fn every_view_deserializes() {
    let client = Client::new();

    for id in ENDPOINTS {
        let id: EndpointId = id.parse().unwrap();
        let endpoint = client.endpoint(&id);

        endpoint.all().await.unwrap();
        endpoint.grid().await.unwrap();
        endpoint.sessions().await.unwrap();
        endpoint.speakers().await.unwrap();
        endpoint.speaker_wall().await.unwrap();

        let event = endpoint
            .event_with(EventOptions::default().with_file_names(true))
            .await
            .unwrap();
        assert!(event.sessions().len() > 0);
        for session in event.sessions() {
            for answer in session.answers() {
                answer.value().unwrap();
            }
        }
    }
}
