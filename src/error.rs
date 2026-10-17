//! Errors of the HTTP client.

use crate::View;

/// Error returned by [`Client`](crate::Client) requests.
///
/// Messages never include the endpoint ID or URL, since the ID is
/// confidential.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The request failed, for example because of a network error or a
    /// timeout.
    #[error("request for the {view} view failed")]
    #[non_exhaustive]
    Request {
        /// The requested view.
        view: View,
        /// The error, without the URL.
        source: reqwest::Error,
    },
    /// The API returned an unsuccessful HTTP status.
    ///
    /// Sessionize returns 404 Not Found for unknown endpoint IDs.
    #[error("the {view} view returned HTTP status {status}")]
    #[non_exhaustive]
    Status {
        /// The requested view.
        view: View,
        /// The HTTP status.
        status: reqwest::StatusCode,
    },
    /// The response couldn't be decoded.
    ///
    /// The source error includes the path of the failing field, such as
    /// `sessions[3].startsAt`.
    #[error("failed to decode the {view} view")]
    #[non_exhaustive]
    Decode {
        /// The requested view.
        view: View,
        /// The error.
        source: serde_path_to_error::Error<serde_json::Error>,
    },
}

impl Error {
    /// Returns the view the failing request was for.
    #[must_use]
    pub fn view(&self) -> View {
        match self {
            Self::Request { view, .. } | Self::Status { view, .. } | Self::Decode { view, .. } => {
                *view
            }
        }
    }
}

/// Error returned by [`ClientBuilder::build`](crate::ClientBuilder::build).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BuildError {
    /// The base URL isn't an `http` or `https` URL.
    #[error("the base URL must be an http or https URL")]
    InvalidBaseUrl,
    /// The user agent isn't a valid HTTP header value.
    #[error("invalid user agent")]
    InvalidUserAgent(#[source] reqwest::header::InvalidHeaderValue),
    /// The HTTP client couldn't be created.
    #[error("failed to create the HTTP client")]
    Http(#[source] reqwest::Error),
}
