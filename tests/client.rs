//! Tests the HTTP client against a mock server.

#![cfg(feature = "client")]

mod support;

use std::error::Error as _;
use std::time::Duration;

use sessionize::{BuildError, Client, EndpointId, Error, EventOptions, View};
use support::{KCD_CUSTOM_FIELDS, VIEWS, read};
use url::Url;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ID: &str = "secret1234";

fn id() -> EndpointId {
    ID.parse().unwrap()
}

async fn server() -> MockServer {
    let server = MockServer::start().await;
    for view in VIEWS {
        Mock::given(method("GET"))
            .and(path(format!("/api/v2/{ID}/view/{view}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(read(KCD_CUSTOM_FIELDS, view), "application/json"),
            )
            .mount(&server)
            .await;
    }
    server
}

fn client(server: &MockServer) -> Client {
    Client::builder()
        .base_url(Url::parse(&format!("{}/api/v2/", server.uri())).unwrap())
        .build()
        .unwrap()
}

/// The error and its sources, as a single string.
fn chain(err: &Error) -> String {
    let mut message = err.to_string();
    let mut source = err.source();
    while let Some(err) = source {
        message.push_str(": ");
        message.push_str(&err.to_string());
        source = err.source();
    }
    message
}

#[tokio::test]
async fn fetches_every_view() {
    let server = server().await;
    let client = client(&server);
    let id = id();
    let endpoint = client.endpoint(&id);

    assert_eq!(endpoint.all().await.unwrap().sessions.len(), 24);
    assert_eq!(endpoint.grid().await.unwrap()[0].time_slots.len(), 25);
    assert_eq!(endpoint.sessions().await.unwrap()[0].sessions.len(), 24);
    assert_eq!(endpoint.speakers().await.unwrap().len(), 25);
    assert_eq!(endpoint.speaker_wall().await.unwrap().len(), 24);
}

#[tokio::test]
async fn event_fetches_all_and_grid() {
    let server = server().await;
    let event = client(&server).event(&id()).await.unwrap();

    assert_eq!(event.sessions().len(), 34);
    let requested: Vec<_> = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| r.url.path().rsplit('/').next().unwrap().to_owned())
        .collect();
    assert_eq!(requested.len(), 2);
    assert!(requested.contains(&"All".to_owned()));
    assert!(requested.contains(&"GridSmart".to_owned()));
}

#[tokio::test]
async fn event_with_file_names_fetches_four_views() {
    let server = server().await;
    let client = client(&server);
    let id = id();
    let event = client
        .endpoint(&id)
        .event_with(EventOptions::default().with_file_names(true))
        .await
        .unwrap();

    let session = event.session(&"886030".into()).unwrap();
    assert_eq!(
        session.answers()[0].file_name(),
        Some("KCD_Budapest_logo-01.svg")
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
}

#[tokio::test]
async fn sends_user_agent() {
    let server = MockServer::start().await;
    Mock::given(header(
        "user-agent",
        concat!("sessionize-rs/", env!("CARGO_PKG_VERSION")),
    ))
    .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
    .expect(1)
    .mount(&server)
    .await;
    Mock::given(header("user-agent", "custom/1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let id = id();
    client(&server).endpoint(&id).speakers().await.unwrap();

    let custom = Client::builder()
        .base_url(server.uri().parse().unwrap())
        .http_client(reqwest::Client::new())
        .user_agent("custom/1.0")
        .build()
        .unwrap();
    custom.endpoint(&id).speakers().await.unwrap();
}

#[tokio::test]
async fn unknown_endpoint_returns_status_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404).set_body_raw("<!DOCTYPE html>", "text/html"))
        .mount(&server)
        .await;

    let err = client(&server).endpoint(&id()).all().await.unwrap_err();

    assert!(
        matches!(
            err,
            Error::Status {
                view: View::All,
                status: reqwest::StatusCode::NOT_FOUND,
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "the All view returned HTTP status 404 Not Found"
    );
}

#[tokio::test]
async fn invalid_response_reports_the_field() {
    let server = MockServer::start().await;
    let body = read(KCD_CUSTOM_FIELDS, "All").replacen(
        r#""startsAt": "2025-04-24T09:00:00""#,
        r#""startsAt": "tomorrow""#,
        1,
    );
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;

    let err = client(&server).endpoint(&id()).all().await.unwrap_err();

    let Error::Decode { view, source, .. } = &err else {
        panic!("{err:?}")
    };
    assert_eq!(*view, View::All);
    assert_eq!(source.path().to_string(), "sessions[0].startsAt");
    assert!(
        chain(&err).contains("invalid schedule time \"tomorrow\""),
        "{}",
        chain(&err)
    );
}

#[tokio::test]
async fn errors_do_not_contain_the_endpoint_id() {
    // Nothing listens on port 9 (discard) of localhost in the test environment.
    let client = Client::builder()
        .base_url(Url::parse("http://127.0.0.1:9/api/v2/").unwrap())
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    let err = client.endpoint(&id()).all().await.unwrap_err();

    assert!(
        matches!(
            err,
            Error::Request {
                view: View::All,
                ..
            }
        ),
        "{err:?}"
    );
    assert!(!chain(&err).contains(ID), "{}", chain(&err));
    assert!(!format!("{err:?}").contains(ID), "{err:?}");
}

#[test]
fn rejects_invalid_base_urls() {
    for url in ["mailto:someone@example.com", "ftp://example.com/"] {
        let err = Client::builder()
            .base_url(Url::parse(url).unwrap())
            .build()
            .unwrap_err();

        assert!(matches!(err, BuildError::InvalidBaseUrl), "{url}: {err:?}");
    }
}

#[test]
fn rejects_invalid_user_agents() {
    let err = Client::builder().user_agent("a\nb").build().unwrap_err();

    assert!(matches!(err, BuildError::InvalidUserAgent(_)), "{err:?}");
}
