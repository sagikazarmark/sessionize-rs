//! HTTP client.

use std::time::Duration;

use futures_util::future::{try_join, try_join4};
use reqwest::header::{HeaderValue, USER_AGENT};
use serde::de::DeserializeOwned;
use url::Url;

use crate::endpoint::default_base_url;
use crate::error::{BuildError, Error};
use crate::{All, EndpointId, Event, SessionGroup, Speaker, View, WallSpeaker, grid};

const DEFAULT_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "-rs/", env!("CARGO_PKG_VERSION"));
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Client for the Sessionize API.
///
/// Cheap to clone; clones share the connection pool. Requires a Tokio
/// runtime.
///
/// Sessionize caches responses for up to five minutes, so the client doesn't
/// cache.
///
/// ```no_run
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// use sessionize::{Client, EndpointId};
///
/// let client = Client::new();
/// let id: EndpointId = "jl4ktls0".parse()?;
///
/// let speakers = client.endpoint(&id).speakers().await?;
/// let event = client.event(&id).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: Url,
    user_agent: Option<HeaderValue>,
    timeout: Option<Duration>,
}

/// Same as [`Client::new`].
///
/// # Panics
///
/// Panics if the TLS backend can't be initialized.
impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    /// Creates a client with the default configuration.
    ///
    /// # Panics
    ///
    /// Panics if the TLS backend can't be initialized. Use
    /// [`Client::builder`] to handle this as an error.
    #[must_use]
    pub fn new() -> Self {
        Self::builder()
            .build()
            .expect("failed to create the HTTP client")
    }

    /// Returns a builder to configure a client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Returns a handle to fetch the views of an endpoint.
    #[must_use]
    pub fn endpoint<'a>(&'a self, id: &'a EndpointId) -> Endpoint<'a> {
        Endpoint { client: self, id }
    }

    /// Fetches the `All` and `GridSmart` views of an endpoint and combines
    /// them into an [`Event`].
    ///
    /// # Errors
    ///
    /// Returns an error if a request fails.
    pub async fn event(&self, id: &EndpointId) -> Result<Event, Error> {
        self.endpoint(id).event().await
    }

    async fn fetch<T: DeserializeOwned>(&self, id: &EndpointId, view: View) -> Result<T, Error> {
        let mut request = self.http.get(id.view_url_on(&self.base_url, view));
        if let Some(user_agent) = &self.user_agent {
            request = request.header(USER_AGENT, user_agent.clone());
        }
        if let Some(timeout) = self.timeout {
            request = request.timeout(timeout);
        }

        let request_error = |source: reqwest::Error| Error::Request {
            view,
            source: source.without_url(),
        };
        let response = request.send().await.map_err(request_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::Status { view, status });
        }
        let body = response.bytes().await.map_err(request_error)?;

        let deserializer = &mut serde_json::Deserializer::from_slice(&body);
        serde_path_to_error::deserialize(deserializer)
            .map_err(|source| Error::Decode { view, source })
    }
}

/// Builder for a [`Client`].
///
/// ```
/// use std::time::Duration;
///
/// use sessionize::Client;
///
/// let client = Client::builder()
///     .user_agent("my-conference-site/1.0")
///     .timeout(Duration::from_secs(10))
///     .build()?;
/// # Ok::<(), sessionize::BuildError>(())
/// ```
#[derive(Debug, Clone, Default)]
#[must_use]
pub struct ClientBuilder {
    base_url: Option<Url>,
    http: Option<reqwest::Client>,
    user_agent: Option<String>,
    timeout: Option<Duration>,
}

impl ClientBuilder {
    /// Sets the base URL of the API. Defaults to
    /// `https://sessionize.com/api/v2/`.
    ///
    /// Views are requested from `{base_url}/{endpoint_id}/view/{view}`.
    pub fn base_url(mut self, base_url: Url) -> Self {
        self.base_url = Some(base_url);
        self
    }

    /// Uses an existing HTTP client, for example to share a connection pool
    /// or configure proxies.
    ///
    /// The client's own user agent and timeout are kept unless set on this
    /// builder.
    pub fn http_client(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }

    /// Sets the `User-Agent` header. Defaults to `sessionize-rs/{version}`.
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Sets the timeout of each request. Defaults to 30 seconds.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Creates the client.
    ///
    /// # Errors
    ///
    /// Returns an error if the base URL isn't an `http` or `https` URL, the
    /// user agent isn't a valid header value, or the HTTP client can't be
    /// created.
    pub fn build(self) -> Result<Client, BuildError> {
        let base_url = self.base_url.unwrap_or_else(default_base_url);
        if !matches!(base_url.scheme(), "http" | "https") || base_url.cannot_be_a_base() {
            return Err(BuildError::InvalidBaseUrl);
        }

        let user_agent = self
            .user_agent
            .map(HeaderValue::try_from)
            .transpose()
            .map_err(BuildError::InvalidUserAgent)?;

        let client = match self.http {
            Some(http) => Client {
                http,
                base_url,
                user_agent,
                timeout: self.timeout,
            },
            None => Client {
                http: reqwest::Client::builder()
                    .user_agent(user_agent.unwrap_or(HeaderValue::from_static(DEFAULT_USER_AGENT)))
                    .timeout(self.timeout.unwrap_or(DEFAULT_TIMEOUT))
                    .build()
                    .map_err(BuildError::Http)?,
                base_url,
                user_agent: None,
                timeout: None,
            },
        };

        Ok(client)
    }
}

/// Fetches the views of an endpoint. Created by [`Client::endpoint`].
///
/// ```no_run
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// use sessionize::{Client, EndpointId, EventOptions};
///
/// let client = Client::new();
/// let id: EndpointId = "jl4ktls0".parse()?;
/// let endpoint = client.endpoint(&id);
///
/// // A single view.
/// let wall = endpoint.speaker_wall().await?;
///
/// // An event including the original names of uploaded files (4 requests).
/// let event = endpoint
///     .event_with(EventOptions::default().with_file_names(true))
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Endpoint<'a> {
    client: &'a Client,
    id: &'a EndpointId,
}

impl Endpoint<'_> {
    /// Fetches the `All` view.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn all(self) -> Result<All, Error> {
        self.client.fetch(self.id, View::All).await
    }

    /// Fetches the `GridSmart` view.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn grid(self) -> Result<Vec<grid::Day>, Error> {
        self.client.fetch(self.id, View::GridSmart).await
    }

    /// Fetches the Sessions view.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn sessions(self) -> Result<Vec<SessionGroup>, Error> {
        self.client.fetch(self.id, View::Sessions).await
    }

    /// Fetches the Speakers view.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn speakers(self) -> Result<Vec<Speaker>, Error> {
        self.client.fetch(self.id, View::Speakers).await
    }

    /// Fetches the `SpeakerWall` view.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn speaker_wall(self) -> Result<Vec<WallSpeaker>, Error> {
        self.client.fetch(self.id, View::SpeakerWall).await
    }

    /// Fetches the `All` and `GridSmart` views concurrently and combines them
    /// into an [`Event`].
    ///
    /// # Errors
    ///
    /// Returns an error if a request fails.
    pub async fn event(self) -> Result<Event, Error> {
        self.event_with(EventOptions::default()).await
    }

    /// Fetches the views needed for an [`Event`] with the given options
    /// concurrently and combines them.
    ///
    /// # Errors
    ///
    /// Returns an error if a request fails.
    pub async fn event_with(self, options: EventOptions) -> Result<Event, Error> {
        let (all, grid, views) = if options.file_names {
            let (all, grid, sessions, speakers) =
                try_join4(self.all(), self.grid(), self.sessions(), self.speakers()).await?;
            (all, grid, Some((sessions, speakers)))
        } else {
            let (all, grid) = try_join(self.all(), self.grid()).await?;
            (all, grid, None)
        };

        let mut event = Event::from(all);
        event.merge_grid(&grid);
        if let Some((sessions, speakers)) = views {
            event.merge_sessions(&sessions);
            event.merge_speakers(&speakers);
        }

        Ok(event)
    }
}

/// Options for [`Endpoint::event_with`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct EventOptions {
    /// Also fetch the Sessions and Speakers views (4 requests instead of 2)
    /// to get the original file names of file uploads.
    pub file_names: bool,
}

impl EventOptions {
    /// Sets [`EventOptions::file_names`].
    #[must_use]
    pub fn with_file_names(mut self, file_names: bool) -> Self {
        self.file_names = file_names;
        self
    }
}
