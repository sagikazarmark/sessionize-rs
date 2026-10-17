//! Speakers as returned by the Speakers and `SpeakerWall` views.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::custom_fields::{CustomFieldsError, Fields};
use crate::{Link, QuestionAnswer, SelectedCategory, SessionId, SpeakerId};

/// A speaker with sessions and custom fields included by name.
///
/// Returned by the Speakers view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Speaker {
    /// Identifier of the speaker.
    pub id: SpeakerId,
    /// First name.
    pub first_name: String,
    /// Last name.
    pub last_name: String,
    /// Full name.
    pub full_name: String,
    /// Biography, if enabled for the endpoint.
    pub bio: Option<String>,
    /// Tagline, such as a job title, if enabled for the endpoint. May be empty.
    pub tag_line: Option<String>,
    /// Profile picture.
    pub profile_picture: Option<Url>,
    /// Sessions of the speaker.
    #[serde(default)]
    pub sessions: Vec<SessionRef>,
    /// Whether the organizer marked the speaker as a top speaker.
    pub is_top_speaker: bool,
    /// Links on the speaker's profile.
    #[serde(default)]
    pub links: Vec<Link>,
    /// Answers to speaker questions, including unanswered ones.
    #[serde(default)]
    pub question_answers: Vec<QuestionAnswer>,
    /// Categories with the selected items.
    #[serde(default)]
    pub categories: Vec<SelectedCategory>,
}

impl Speaker {
    /// Deserializes the speaker's custom fields into `T`.
    ///
    /// See [custom fields](crate#custom-fields).
    ///
    /// # Errors
    ///
    /// Returns an error if the custom fields don't match `T`.
    pub fn custom_fields<T: DeserializeOwned>(&self) -> Result<T, CustomFieldsError> {
        Fields::from_view(&self.question_answers, &self.categories).deserialize()
    }
}

/// A session referenced by a speaker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SessionRef {
    /// Identifier of the session.
    #[serde(serialize_with = "crate::id::session_ref::serialize")]
    pub id: SessionId,
    /// Title of the session.
    pub name: String,
}

/// A speaker as returned by the `SpeakerWall` view.
///
/// The order of speakers in the view is random on every request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct WallSpeaker {
    /// Identifier of the speaker.
    pub id: SpeakerId,
    /// First name.
    pub first_name: String,
    /// Last name.
    pub last_name: String,
    /// Full name.
    pub full_name: String,
    /// Tagline, such as a job title, if enabled for the endpoint. May be empty.
    pub tag_line: Option<String>,
    /// Profile picture.
    pub profile_picture: Option<Url>,
    /// Whether the organizer marked the speaker as a top speaker.
    pub is_top_speaker: bool,
}
