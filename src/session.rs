//! Sessions as returned by the Sessions and `GridSmart` views.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::answer::{AnswerValue, ParseAnswerError};
use crate::custom_fields::{CustomFieldsError, Fields};
use crate::{
    CategoryId, CategoryItemId, GroupId, QuestionId, QuestionType, RoomId, ScheduleTime, SessionId,
    SessionStatus, SpeakerId,
};

/// A session with speakers, room and custom fields included by name.
///
/// Returned by the Sessions view (in [`SessionGroup`]s) and the `GridSmart`
/// view (in [`grid::Day`](crate::grid::Day)s).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
#[allow(clippy::struct_excessive_bools, reason = "mirrors the API")]
pub struct Session {
    /// Identifier of the session.
    pub id: SessionId,
    /// Title of the session.
    pub title: String,
    /// Description of the session. Often missing for service sessions.
    pub description: Option<String>,
    /// Start of the session, if the schedule is announced.
    pub starts_at: Option<ScheduleTime>,
    /// End of the session, if the schedule is announced.
    pub ends_at: Option<ScheduleTime>,
    /// Whether this is a service session, such as a break or lunch.
    pub is_service_session: bool,
    /// Whether the session is for all attendees (nothing runs in parallel).
    pub is_plenum_session: bool,
    /// Speakers of the session.
    #[serde(default)]
    pub speakers: Vec<SpeakerRef>,
    /// Categories with the selected items.
    #[serde(default)]
    pub categories: Vec<SelectedCategory>,
    /// Room of the session, if the schedule is announced.
    pub room_id: Option<RoomId>,
    /// Name of the room, if the schedule is announced.
    pub room: Option<String>,
    /// Link to the live stream, as entered by the organizer.
    pub live_url: Option<String>,
    /// Link to the recording, as entered by the organizer.
    pub recording_url: Option<String>,
    /// Status of the session. Service sessions have none.
    pub status: Option<SessionStatus>,
    /// Whether the speakers were informed about the session's status.
    pub is_informed: bool,
    /// Whether the speakers confirmed their participation.
    pub is_confirmed: bool,
    /// Answers to session questions. Not included in the `GridSmart` view.
    #[serde(default)]
    pub question_answers: Vec<QuestionAnswer>,
}

impl Session {
    /// Deserializes the session's custom fields into `T`.
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

/// A speaker referenced by a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SpeakerRef {
    /// Identifier of the speaker.
    pub id: SpeakerId,
    /// Full name of the speaker.
    pub name: String,
}

/// A category with the items selected for a session or speaker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SelectedCategory {
    /// Identifier of the category.
    pub id: CategoryId,
    /// Title of the category.
    pub name: String,
    /// Selected items. Empty if nothing is selected.
    #[serde(rename = "categoryItems", default)]
    pub items: Vec<CategoryItemRef>,
    /// Position among custom fields in the organizer's order.
    pub sort: i32,
}

/// A selected category item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CategoryItemRef {
    /// Identifier of the item.
    pub id: CategoryItemId,
    /// Name of the item.
    pub name: String,
}

/// A question with its answer, as included in the Sessions and Speakers views.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct QuestionAnswer {
    /// Identifier of the question.
    pub id: QuestionId,
    /// Text of the question.
    pub question: String,
    /// Type of the question.
    pub question_type: QuestionType,
    /// The answer, or `None` if unanswered.
    pub answer: Option<String>,
    /// Position among custom fields in the organizer's order.
    pub sort: i32,
    /// Extra data: the original file name for file uploads.
    pub answer_extra: Option<String>,
}

impl QuestionAnswer {
    /// Interprets the answer according to the question type.
    ///
    /// Returns `Ok(None)` if the question is unanswered.
    ///
    /// # Errors
    ///
    /// Returns an error if the answer doesn't match the question type.
    pub fn value(&self) -> Result<Option<AnswerValue>, ParseAnswerError> {
        self.answer
            .as_deref()
            .map(|answer| {
                AnswerValue::parse(&self.question_type, answer, self.answer_extra.as_deref())
            })
            .transpose()
    }
}

/// A group of sessions returned by the Sessions view.
///
/// Sessions are grouped according to the endpoint's "Group list by" setting.
/// Without grouping, there's a single group named "All".
///
/// When grouping by category, sessions without an item of that category
/// (such as service sessions) aren't included in any group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SessionGroup {
    /// Identifier of the group, or `None` without grouping.
    #[serde(rename = "groupId")]
    pub id: Option<GroupId>,
    /// Name of the group.
    #[serde(rename = "groupName")]
    pub name: String,
    /// Sessions in the group.
    #[serde(default)]
    pub sessions: Vec<Session>,
    /// Whether this is the default group.
    pub is_default: bool,
}
