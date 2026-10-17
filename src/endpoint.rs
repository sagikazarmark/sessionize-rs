//! Endpoints and views.

use std::fmt;
use std::str::FromStr;

use url::Url;

/// Base URL of the Sessionize API.
pub const DEFAULT_BASE_URL: &str = "https://sessionize.com/api/v2/";

const MAX_ENDPOINT_ID_LEN: usize = 64;

pub(crate) fn default_base_url() -> Url {
    Url::parse(DEFAULT_BASE_URL).expect("the default base URL is valid")
}

/// Identifier of an API endpoint, such as `jl4ktls0`.
///
/// Endpoints are created on the event's API / Embed page in Sessionize. The
/// identifier gives access to the endpoint's data without authentication, so
/// treat it as confidential: its [`Debug`] output is redacted, and it has no
/// [`Display`](fmt::Display) implementation.
///
/// ```
/// # use sessionize::{EndpointId, View};
/// let id: EndpointId = "jl4ktls0".parse().unwrap();
///
/// assert_eq!(format!("{id:?}"), "EndpointId(<redacted>)");
/// assert_eq!(
///     id.view_url(View::All).as_str(),
///     "https://sessionize.com/api/v2/jl4ktls0/view/All"
/// );
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EndpointId(String);

impl EndpointId {
    /// Creates an endpoint identifier.
    ///
    /// # Errors
    ///
    /// Returns an error unless `id` consists of 1 to 64 ASCII letters and
    /// digits.
    pub fn new(id: impl Into<String>) -> Result<Self, InvalidEndpointId> {
        let id = id.into();
        let valid = !id.is_empty()
            && id.len() <= MAX_ENDPOINT_ID_LEN
            && id.bytes().all(|b| b.is_ascii_alphanumeric());

        if valid {
            Ok(Self(id))
        } else {
            Err(InvalidEndpointId(()))
        }
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the URL of a view of this endpoint on the Sessionize API.
    #[must_use]
    pub fn view_url(&self, view: View) -> Url {
        self.view_url_on(&default_base_url(), view)
    }

    /// Returns the URL of a view on the given base URL.
    ///
    /// The base URL must be able to have a path (`http` and `https` URLs can).
    pub(crate) fn view_url_on(&self, base: &Url, view: View) -> Url {
        let mut url = base.clone();
        url.path_segments_mut()
            .expect("the base URL can have a path")
            .pop_if_empty()
            .extend([self.0.as_str(), "view", view.as_str()]);
        url
    }
}

impl fmt::Debug for EndpointId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EndpointId(<redacted>)")
    }
}

impl FromStr for EndpointId {
    type Err = InvalidEndpointId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for EndpointId {
    type Error = InvalidEndpointId;

    fn try_from(id: String) -> Result<Self, Self::Error> {
        Self::new(id)
    }
}

impl TryFrom<&str> for EndpointId {
    type Error = InvalidEndpointId;

    fn try_from(id: &str) -> Result<Self, Self::Error> {
        Self::new(id)
    }
}

/// Error returned for an invalid [`EndpointId`].
///
/// The invalid value isn't included, since it may be close to a real one.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid endpoint ID: expected 1 to 64 ASCII letters and digits")]
pub struct InvalidEndpointId(());

/// A view of an endpoint: the same data in a different shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum View {
    /// All data with references by ID ([`All`](crate::All)).
    All,
    /// Schedule grid by day, room and time slot ([`grid::Day`](crate::grid::Day)).
    GridSmart,
    /// Sessions in groups ([`SessionGroup`](crate::SessionGroup)).
    Sessions,
    /// Speakers with their sessions ([`Speaker`](crate::Speaker)).
    Speakers,
    /// Speakers for a speaker wall ([`WallSpeaker`](crate::WallSpeaker)).
    SpeakerWall,
}

impl View {
    /// Returns the name of the view in the API.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::GridSmart => "GridSmart",
            Self::Sessions => "Sessions",
            Self::Speakers => "Speakers",
            Self::SpeakerWall => "SpeakerWall",
        }
    }
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_endpoint_ids() {
        assert!(EndpointId::new("dpesp6nz").is_ok());
        for invalid in ["", "abc/def", "abc def", "ab?c", &"a".repeat(65)] {
            assert!(EndpointId::new(invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn builds_view_urls_on_any_base() {
        let id = EndpointId::new("abc123").unwrap();

        for base in [
            "http://localhost:8080",
            "http://localhost:8080/",
            "http://localhost:8080/api/v2",
            "http://localhost:8080/api/v2/",
        ] {
            let url = id.view_url_on(&Url::parse(base).unwrap(), View::SpeakerWall);

            assert!(
                url.as_str().ends_with("/abc123/view/SpeakerWall"),
                "{base}: {url}"
            );
            assert!(!url.as_str().contains("//abc123"), "{base}: {url}");
        }
    }
}
