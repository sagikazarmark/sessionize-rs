//! Types of the `All` view: all data with references by ID.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::answer::{AnswerValue, ParseAnswerError};
use crate::{
    Category, CategoryId, CategoryItem, CategoryItemId, Link, Question, QuestionId, Room, RoomId,
    ScheduleTime, SessionId, SessionStatus, SpeakerId,
};

/// All data of an event, returned by the `All` view.
///
/// Sessions and speakers reference each other, rooms, questions and category
/// items by ID. Use the lookup methods to resolve them, or convert into an
/// [`Event`](crate::Event) for a navigable model.
///
/// Service sessions are only included if the endpoint's Sessions view
/// includes them; the `GridSmart` view may have more.
///
/// ```
/// use sessionize::All;
///
/// let all: All = serde_json::from_str(
///     r#"{
///         "sessions": [{
///             "id": "1", "title": "Keynote", "roomId": 1, "speakers": ["a"],
///             "isServiceSession": false, "isPlenumSession": true,
///             "isInformed": true, "isConfirmed": true
///         }],
///         "speakers": [{
///             "id": "a", "firstName": "Ada", "lastName": "Lovelace", "fullName": "Ada Lovelace",
///             "isTopSpeaker": true, "sessions": [1]
///         }],
///         "rooms": [{ "id": 1, "name": "Main hall", "sort": 0 }]
///     }"#,
/// )?;
///
/// let keynote = all.session(&"1".into()).unwrap();
/// let room = all.room(keynote.room_id.unwrap()).unwrap();
/// let speakers: Vec<_> = all.speakers_of(keynote).map(|s| s.full_name.as_str()).collect();
///
/// assert_eq!(room.name, "Main hall");
/// assert_eq!(speakers, ["Ada Lovelace"]);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct All {
    /// Sessions.
    #[serde(default)]
    pub sessions: Vec<Session>,
    /// Speakers.
    #[serde(default)]
    pub speakers: Vec<Speaker>,
    /// Session and speaker questions.
    #[serde(default)]
    pub questions: Vec<Question>,
    /// Session and speaker categories.
    #[serde(default)]
    pub categories: Vec<Category>,
    /// Rooms.
    #[serde(default)]
    pub rooms: Vec<Room>,
}

impl All {
    /// Returns the session with the given ID.
    #[must_use]
    pub fn session(&self, id: &SessionId) -> Option<&Session> {
        self.sessions.iter().find(|session| &session.id == id)
    }

    /// Returns the speaker with the given ID.
    #[must_use]
    pub fn speaker(&self, id: &SpeakerId) -> Option<&Speaker> {
        self.speakers.iter().find(|speaker| &speaker.id == id)
    }

    /// Returns the room with the given ID.
    #[must_use]
    pub fn room(&self, id: RoomId) -> Option<&Room> {
        self.rooms.iter().find(|room| room.id == id)
    }

    /// Returns the question with the given ID.
    #[must_use]
    pub fn question(&self, id: QuestionId) -> Option<&Question> {
        self.questions.iter().find(|question| question.id == id)
    }

    /// Returns the category with the given ID.
    #[must_use]
    pub fn category(&self, id: CategoryId) -> Option<&Category> {
        self.categories.iter().find(|category| category.id == id)
    }

    /// Returns the category item with the given ID, along with its category.
    #[must_use]
    pub fn category_item(&self, id: CategoryItemId) -> Option<(&Category, &CategoryItem)> {
        self.categories.iter().find_map(|category| {
            category
                .items
                .iter()
                .find(|item| item.id == id)
                .map(|item| (category, item))
        })
    }

    /// Returns the sessions of the given speaker.
    pub fn sessions_of<'a>(&'a self, speaker: &'a SpeakerId) -> impl Iterator<Item = &'a Session> {
        self.sessions
            .iter()
            .filter(move |session| session.speakers.contains(speaker))
    }

    /// Returns the speakers of the given session.
    pub fn speakers_of<'a>(&'a self, session: &'a Session) -> impl Iterator<Item = &'a Speaker> {
        session.speakers.iter().filter_map(|id| self.speaker(id))
    }
}

/// A session of the [`All`] view.
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
    pub speakers: Vec<SpeakerId>,
    /// Selected category items.
    #[serde(default)]
    pub category_items: Vec<CategoryItemId>,
    /// Answers to session questions. Unanswered questions are left out.
    #[serde(default)]
    pub question_answers: Vec<Answer>,
    /// Room of the session, if the schedule is announced.
    pub room_id: Option<RoomId>,
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
}

/// A speaker of the [`All`] view.
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
    /// Whether the organizer marked the speaker as a top speaker.
    pub is_top_speaker: bool,
    /// Links on the speaker's profile.
    #[serde(default)]
    pub links: Vec<Link>,
    /// Sessions of the speaker.
    #[serde(default, serialize_with = "crate::id::session_ref::serialize_all")]
    pub sessions: Vec<SessionId>,
    /// Selected category items.
    #[serde(default)]
    pub category_items: Vec<CategoryItemId>,
    /// Answers to speaker questions. Unanswered questions are left out.
    #[serde(default)]
    pub question_answers: Vec<Answer>,
}

/// An answer to a question in the [`All`] view.
///
/// The `All` view doesn't include the original file name of file uploads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Answer {
    /// Identifier of the question, see [`All::question`].
    pub question_id: QuestionId,
    /// The answer.
    #[serde(rename = "answerValue")]
    pub answer: String,
}

impl Answer {
    /// Interprets the answer according to the question's type.
    ///
    /// `question` should be the question with [`Answer::question_id`], see
    /// [`All::question`].
    ///
    /// # Errors
    ///
    /// Returns an error if the answer doesn't match the question type.
    pub fn value(&self, question: &Question) -> Result<AnswerValue, ParseAnswerError> {
        AnswerValue::parse(&question.question_type, &self.answer, None)
    }
}

impl From<&crate::Session> for Session {
    fn from(session: &crate::Session) -> Self {
        Self {
            id: session.id.clone(),
            title: session.title.clone(),
            description: session.description.clone(),
            starts_at: session.starts_at,
            ends_at: session.ends_at,
            is_service_session: session.is_service_session,
            is_plenum_session: session.is_plenum_session,
            speakers: session.speakers.iter().map(|s| s.id.clone()).collect(),
            category_items: selected_items(&session.categories),
            question_answers: answers(&session.question_answers),
            room_id: session.room_id,
            live_url: session.live_url.clone(),
            recording_url: session.recording_url.clone(),
            status: session.status.clone(),
            is_informed: session.is_informed,
            is_confirmed: session.is_confirmed,
        }
    }
}

impl From<&crate::Speaker> for Speaker {
    fn from(speaker: &crate::Speaker) -> Self {
        Self {
            id: speaker.id.clone(),
            first_name: speaker.first_name.clone(),
            last_name: speaker.last_name.clone(),
            full_name: speaker.full_name.clone(),
            bio: speaker.bio.clone(),
            tag_line: speaker.tag_line.clone(),
            profile_picture: speaker.profile_picture.clone(),
            is_top_speaker: speaker.is_top_speaker,
            links: speaker.links.clone(),
            sessions: speaker.sessions.iter().map(|s| s.id.clone()).collect(),
            category_items: selected_items(&speaker.categories),
            question_answers: answers(&speaker.question_answers),
        }
    }
}

fn selected_items(categories: &[crate::SelectedCategory]) -> Vec<CategoryItemId> {
    categories
        .iter()
        .flat_map(|category| &category.items)
        .map(|item| item.id)
        .collect()
}

fn answers(answers: &[crate::QuestionAnswer]) -> Vec<Answer> {
    answers
        .iter()
        .filter_map(|answer| {
            answer.answer.as_ref().map(|value| Answer {
                question_id: answer.id,
                answer: value.clone(),
            })
        })
        .collect()
}
