//! A navigable model of an event, combining several views.

use std::collections::HashMap;
use std::fmt;

use chrono::NaiveDate;
use serde::de::DeserializeOwned;
use url::Url;

use crate::answer::{AnswerValue, ParseAnswerError};
use crate::custom_fields::{CustomFieldsError, Fields};
use crate::{
    All, Category, CategoryItem, CategoryItemId, CategoryType, Link, Question, QuestionId, Room,
    RoomId, ScheduleTime, SessionGroup, SessionId, SessionStatus, SpeakerId, all, grid,
};

/// An event: sessions, speakers, rooms and custom fields, linked together.
///
/// Built from the [`All`] view, optionally completed with data that only
/// other views have:
///
/// - [`Event::merge_grid`] adds service sessions that only the `GridSmart`
///   view includes.
/// - [`Event::merge_sessions`] and [`Event::merge_speakers`] add the original
///   file names of file uploads.
///
/// With the `client` feature, `Client::event` does this for you.
///
/// Sessions are ordered by start time and room; unscheduled sessions come
/// last.
///
/// ```
/// use sessionize::{All, Event};
///
/// let all: All = serde_json::from_str(
///     r#"{
///         "sessions": [{
///             "id": "1", "title": "Keynote", "roomId": 1, "speakers": ["a"],
///             "startsAt": "2025-04-24T09:00:00", "endsAt": "2025-04-24T10:00:00",
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
/// let event = Event::from(all);
///
/// let keynote = event.sessions().next().unwrap();
/// assert_eq!(keynote.room().unwrap().name, "Main hall");
///
/// let ada = keynote.speakers().next().unwrap();
/// assert_eq!(ada.full_name(), "Ada Lovelace");
/// assert_eq!(ada.sessions().next().unwrap().title(), "Keynote");
///
/// let schedule = event.schedule();
/// assert_eq!(schedule[0].date().to_string(), "2025-04-24");
/// assert_eq!(schedule[0].time_slots()[0].start().to_string(), "2025-04-24T09:00:00");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone)]
pub struct Event {
    sessions: Vec<all::Session>,
    speakers: Vec<all::Speaker>,
    questions: Vec<Question>,
    categories: Vec<Category>,
    rooms: Vec<Room>,
    session_file_names: FileNames<SessionId>,
    speaker_file_names: FileNames<SpeakerId>,
    index: Index,
}

type FileNames<K> = HashMap<K, HashMap<QuestionId, String>>;

#[derive(Clone, Default)]
struct Index {
    sessions: HashMap<SessionId, usize>,
    speakers: HashMap<SpeakerId, usize>,
    rooms: HashMap<RoomId, usize>,
    questions: HashMap<QuestionId, usize>,
    items: HashMap<CategoryItemId, ItemPosition>,
}

/// Position of a category item in `Event::categories`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ItemPosition {
    category: usize,
    item: usize,
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Event")
            .field("sessions", &self.sessions)
            .field("speakers", &self.speakers)
            .field("questions", &self.questions)
            .field("categories", &self.categories)
            .field("rooms", &self.rooms)
            .finish_non_exhaustive()
    }
}

impl From<All> for Event {
    fn from(all: All) -> Self {
        let mut event = Self {
            sessions: all.sessions,
            speakers: all.speakers,
            questions: all.questions,
            categories: all.categories,
            rooms: all.rooms,
            session_file_names: HashMap::new(),
            speaker_file_names: HashMap::new(),
            index: Index::default(),
        };

        event.rooms.sort_by_key(|room| (room.sort, room.id));
        event.questions.sort_by_key(|question| question.sort);
        event.categories.sort_by_key(|category| category.sort);
        for category in &mut event.categories {
            category.items.sort_by_key(|item| item.sort);
        }
        event.reindex();

        event
    }
}

impl Event {
    /// Adds sessions from the `GridSmart` view that the `All` view doesn't
    /// include, such as service sessions.
    pub fn merge_grid(&mut self, days: &[grid::Day]) {
        self.merge_view_sessions(
            days.iter()
                .flat_map(|day| &day.rooms)
                .flat_map(|room| &room.sessions),
        );
    }

    /// Adds the original file names of session file uploads, and sessions
    /// the `All` view doesn't include.
    pub fn merge_sessions(&mut self, groups: &[SessionGroup]) {
        self.merge_view_sessions(groups.iter().flat_map(|group| &group.sessions));
    }

    /// Adds the original file names of speaker file uploads, and speakers the
    /// `All` view doesn't include.
    pub fn merge_speakers(&mut self, speakers: &[crate::Speaker]) {
        for speaker in speakers {
            add_file_names(
                &mut self.speaker_file_names,
                &speaker.id,
                &speaker.question_answers,
            );
            if !self.index.speakers.contains_key(&speaker.id) {
                self.index
                    .speakers
                    .insert(speaker.id.clone(), self.speakers.len());
                self.speakers.push(all::Speaker::from(speaker));
            }
        }
        self.reindex();
    }

    fn merge_view_sessions<'a>(&mut self, sessions: impl Iterator<Item = &'a crate::Session>) {
        for session in sessions {
            add_file_names(
                &mut self.session_file_names,
                &session.id,
                &session.question_answers,
            );
            if !self.index.sessions.contains_key(&session.id) {
                self.index
                    .sessions
                    .insert(session.id.clone(), self.sessions.len());
                self.sessions.push(all::Session::from(session));
            }
        }
        self.reindex();
    }

    fn reindex(&mut self) {
        let room_sort: HashMap<RoomId, i32> =
            self.rooms.iter().map(|room| (room.id, room.sort)).collect();
        self.sessions.sort_by_key(|session| {
            (
                session.starts_at.is_none(),
                session.starts_at.map(ScheduleTime::naive),
                session
                    .room_id
                    .and_then(|id| room_sort.get(&id).copied())
                    .unwrap_or(i32::MAX),
                session.room_id,
            )
        });

        self.index = Index {
            sessions: self
                .sessions
                .iter()
                .enumerate()
                .map(|(i, session)| (session.id.clone(), i))
                .collect(),
            speakers: self
                .speakers
                .iter()
                .enumerate()
                .map(|(i, speaker)| (speaker.id.clone(), i))
                .collect(),
            rooms: self
                .rooms
                .iter()
                .enumerate()
                .map(|(i, room)| (room.id, i))
                .collect(),
            questions: self
                .questions
                .iter()
                .enumerate()
                .map(|(i, question)| (question.id, i))
                .collect(),
            items: self
                .categories
                .iter()
                .enumerate()
                .flat_map(|(c, category)| {
                    category.items.iter().enumerate().map(move |(i, item)| {
                        (
                            item.id,
                            ItemPosition {
                                category: c,
                                item: i,
                            },
                        )
                    })
                })
                .collect(),
        };
    }

    /// Returns the sessions, ordered by start time and room.
    #[must_use]
    pub fn sessions(&self) -> impl ExactSizeIterator<Item = Session<'_>> {
        self.sessions.iter().map(|session| Session {
            event: self,
            session,
        })
    }

    /// Returns the session with the given ID.
    #[must_use]
    pub fn session(&self, id: &SessionId) -> Option<Session<'_>> {
        self.index.sessions.get(id).map(|&i| Session {
            event: self,
            session: &self.sessions[i],
        })
    }

    /// Returns the speakers.
    #[must_use]
    pub fn speakers(&self) -> impl ExactSizeIterator<Item = Speaker<'_>> {
        self.speakers.iter().map(|speaker| Speaker {
            event: self,
            speaker,
        })
    }

    /// Returns the speaker with the given ID.
    #[must_use]
    pub fn speaker(&self, id: &SpeakerId) -> Option<Speaker<'_>> {
        self.index.speakers.get(id).map(|&i| Speaker {
            event: self,
            speaker: &self.speakers[i],
        })
    }

    /// Returns the rooms in the organizer's order.
    #[must_use]
    pub fn rooms(&self) -> &[Room] {
        &self.rooms
    }

    /// Returns the room with the given ID.
    #[must_use]
    pub fn room(&self, id: RoomId) -> Option<&Room> {
        self.index.rooms.get(&id).map(|&i| &self.rooms[i])
    }

    /// Returns the session and speaker questions in the organizer's order.
    #[must_use]
    pub fn questions(&self) -> &[Question] {
        &self.questions
    }

    /// Returns the question with the given ID.
    #[must_use]
    pub fn question(&self, id: QuestionId) -> Option<&Question> {
        self.index.questions.get(&id).map(|&i| &self.questions[i])
    }

    /// Returns the session and speaker categories in the organizer's order.
    #[must_use]
    pub fn categories(&self) -> &[Category] {
        &self.categories
    }

    /// Returns the schedule: scheduled sessions grouped by day and start time.
    ///
    /// Days are the dates of [`ScheduleTime::naive`]: the event's local dates,
    /// or UTC dates if the endpoint uses UTC. Unscheduled sessions are left
    /// out.
    #[must_use]
    pub fn schedule(&self) -> Vec<Day<'_>> {
        let mut days: Vec<Day<'_>> = Vec::new();

        for session in self.sessions() {
            let Some(start) = session.starts_at() else {
                continue;
            };
            match days.last_mut() {
                Some(day) if day.date == start.date() => day.add(start, session),
                _ => days.push(Day {
                    date: start.date(),
                    time_slots: vec![TimeSlot {
                        start,
                        sessions: vec![session],
                    }],
                }),
            }
        }

        days
    }

    /// Deserializes the custom fields of every regular session into `S` and
    /// every speaker into `P`.
    ///
    /// Service sessions are skipped. Use [`serde::de::IgnoredAny`] for `S` or
    /// `P` to skip sessions or speakers.
    ///
    /// See [custom fields](crate#custom-fields).
    ///
    /// # Errors
    ///
    /// Returns an error for the first session or speaker whose custom fields
    /// don't match `S` or `P`.
    ///
    /// # Examples
    ///
    /// ```
    /// use serde::Deserialize;
    /// use serde::de::IgnoredAny;
    /// use sessionize::{All, Event};
    ///
    /// #[derive(Deserialize)]
    /// struct Talk {
    ///     #[serde(rename = "Level")]
    ///     level: String,
    ///     #[serde(rename = "Expected attendees")]
    ///     attendees: Option<u32>,
    /// }
    ///
    /// let all: All = serde_json::from_str(
    ///     r#"{
    ///         "sessions": [{
    ///             "id": "1", "title": "Keynote", "categoryItems": [10],
    ///             "questionAnswers": [{ "questionId": 5, "answerValue": "120" }],
    ///             "isServiceSession": false, "isPlenumSession": true,
    ///             "isInformed": true, "isConfirmed": true
    ///         }],
    ///         "questions": [
    ///             { "id": 5, "question": "Expected attendees", "questionType": "Integer", "sort": 1 }
    ///         ],
    ///         "categories": [{
    ///             "id": 2, "title": "Level", "sort": 0, "type": "session",
    ///             "items": [{ "id": 10, "name": "Advanced", "sort": 0 }]
    ///         }]
    ///     }"#,
    /// )?;
    ///
    /// // `IgnoredAny` skips the speakers' custom fields.
    /// let typed = Event::from(all).into_typed::<Talk, IgnoredAny>()?;
    ///
    /// let (session, talk) = typed.sessions().next().unwrap();
    /// assert_eq!(session.title(), "Keynote");
    /// assert_eq!(talk.level, "Advanced");
    /// assert_eq!(talk.attendees, Some(120));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn into_typed<S, P>(self) -> Result<TypedEvent<S, P>, TypedEventError>
    where
        S: DeserializeOwned,
        P: DeserializeOwned,
    {
        let sessions = self
            .sessions()
            .filter(|session| !session.is_service_session())
            .map(|session| {
                session
                    .custom_fields()
                    .map(|fields| (session.id().clone(), fields))
                    .map_err(|source| TypedEventError::Session {
                        id: session.id().clone(),
                        source,
                    })
            })
            .collect::<Result<_, _>>()?;
        let speakers = self
            .speakers()
            .map(|speaker| {
                speaker
                    .custom_fields()
                    .map(|fields| (speaker.id().clone(), fields))
                    .map_err(|source| TypedEventError::Speaker {
                        id: speaker.id().clone(),
                        source,
                    })
            })
            .collect::<Result<_, _>>()?;

        Ok(TypedEvent {
            event: self,
            sessions,
            speakers,
        })
    }

    fn selected_categories(&self, items: &[CategoryItemId]) -> Vec<CategorySelection<'_>> {
        let mut selections: Vec<CategorySelection<'_>> = Vec::new();
        let mut selected: Vec<ItemPosition> = items
            .iter()
            .filter_map(|id| self.index.items.get(id).copied())
            .collect();
        selected.sort_unstable();
        selected.dedup();

        for position in selected {
            let category = &self.categories[position.category];
            let item = &category.items[position.item];
            match selections.last_mut() {
                Some(selection) if selection.category.id == category.id => {
                    selection.items.push(item);
                }
                _ => selections.push(CategorySelection {
                    category,
                    items: vec![item],
                }),
            }
        }

        selections
    }

    fn answers<'a>(
        &'a self,
        answers: &'a [all::Answer],
        file_names: Option<&'a HashMap<QuestionId, String>>,
    ) -> Vec<Answer<'a>> {
        let mut answers: Vec<_> = answers
            .iter()
            .filter_map(|answer| {
                let question = self.question(answer.question_id)?;
                Some(Answer {
                    question,
                    text: &answer.answer,
                    file_name: file_names
                        .and_then(|names| names.get(&answer.question_id))
                        .map(String::as_str),
                })
            })
            .collect();
        answers.sort_by_key(|answer| answer.question.sort);

        answers
    }

    fn custom_fields<T: DeserializeOwned>(
        &self,
        answers: &[Answer<'_>],
        category_type: &CategoryType,
        items: &[CategoryItemId],
    ) -> Result<T, CustomFieldsError> {
        let mut fields = Fields::default();
        for answer in answers {
            fields.answer(
                &answer.question.question,
                answer.question.id.get(),
                &answer.question.question_type,
                answer.text,
                answer.file_name,
            );
        }
        for category in self
            .categories
            .iter()
            .filter(|category| &category.category_type == category_type)
        {
            let selected = category
                .items
                .iter()
                .filter(|item| items.contains(&item.id))
                .map(|item| item.name.as_str())
                .collect();
            fields.category(&category.title, category.id.get(), selected);
        }

        fields.deserialize()
    }
}

fn add_file_names<K: Clone + Eq + std::hash::Hash>(
    file_names: &mut FileNames<K>,
    owner: &K,
    answers: &[crate::QuestionAnswer],
) {
    for answer in answers {
        if let Some(file_name) = &answer.answer_extra {
            file_names
                .entry(owner.clone())
                .or_default()
                .insert(answer.id, file_name.clone());
        }
    }
}

/// A session of an [`Event`].
#[derive(Clone, Copy)]
pub struct Session<'a> {
    event: &'a Event,
    session: &'a all::Session,
}

impl fmt::Debug for Session<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Session").field(self.session).finish()
    }
}

impl<'a> Session<'a> {
    /// Returns the session data as sent by the API.
    #[must_use]
    pub fn raw(self) -> &'a all::Session {
        self.session
    }

    /// Identifier of the session.
    #[must_use]
    pub fn id(self) -> &'a SessionId {
        &self.session.id
    }

    /// Title of the session.
    #[must_use]
    pub fn title(self) -> &'a str {
        &self.session.title
    }

    /// Description of the session.
    #[must_use]
    pub fn description(self) -> Option<&'a str> {
        self.session.description.as_deref()
    }

    /// Start of the session, if the schedule is announced.
    #[must_use]
    pub fn starts_at(self) -> Option<ScheduleTime> {
        self.session.starts_at
    }

    /// End of the session, if the schedule is announced.
    #[must_use]
    pub fn ends_at(self) -> Option<ScheduleTime> {
        self.session.ends_at
    }

    /// Whether this is a service session, such as a break or lunch.
    #[must_use]
    pub fn is_service_session(self) -> bool {
        self.session.is_service_session
    }

    /// Whether the session is for all attendees (nothing runs in parallel).
    #[must_use]
    pub fn is_plenum_session(self) -> bool {
        self.session.is_plenum_session
    }

    /// Status of the session. Service sessions have none.
    #[must_use]
    pub fn status(self) -> Option<&'a SessionStatus> {
        self.session.status.as_ref()
    }

    /// Link to the live stream, as entered by the organizer.
    #[must_use]
    pub fn live_url(self) -> Option<&'a str> {
        self.session.live_url.as_deref()
    }

    /// Link to the recording, as entered by the organizer.
    #[must_use]
    pub fn recording_url(self) -> Option<&'a str> {
        self.session.recording_url.as_deref()
    }

    /// Room of the session, if the schedule is announced.
    #[must_use]
    pub fn room(self) -> Option<&'a Room> {
        self.session.room_id.and_then(|id| self.event.room(id))
    }

    /// Speakers of the session.
    pub fn speakers(self) -> impl Iterator<Item = Speaker<'a>> {
        let event = self.event;
        self.session
            .speakers
            .iter()
            .filter_map(move |id| event.speaker(id))
    }

    /// Categories with selected items, in the organizer's order.
    #[must_use]
    pub fn categories(self) -> Vec<CategorySelection<'a>> {
        self.event.selected_categories(&self.session.category_items)
    }

    /// Answered questions, in the organizer's order.
    #[must_use]
    pub fn answers(self) -> Vec<Answer<'a>> {
        self.event.answers(
            &self.session.question_answers,
            self.event.session_file_names.get(&self.session.id),
        )
    }

    /// Deserializes the session's custom fields into `T`.
    ///
    /// All session categories are included, with no items if none are
    /// selected. See [custom fields](crate#custom-fields).
    ///
    /// # Errors
    ///
    /// Returns an error if the custom fields don't match `T`.
    pub fn custom_fields<T: DeserializeOwned>(self) -> Result<T, CustomFieldsError> {
        self.event.custom_fields(
            &self.answers(),
            &CategoryType::Session,
            &self.session.category_items,
        )
    }
}

/// A speaker of an [`Event`].
#[derive(Clone, Copy)]
pub struct Speaker<'a> {
    event: &'a Event,
    speaker: &'a all::Speaker,
}

impl fmt::Debug for Speaker<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Speaker").field(self.speaker).finish()
    }
}

impl<'a> Speaker<'a> {
    /// Returns the speaker data as sent by the API.
    #[must_use]
    pub fn raw(self) -> &'a all::Speaker {
        self.speaker
    }

    /// Identifier of the speaker.
    #[must_use]
    pub fn id(self) -> &'a SpeakerId {
        &self.speaker.id
    }

    /// First name.
    #[must_use]
    pub fn first_name(self) -> &'a str {
        &self.speaker.first_name
    }

    /// Last name.
    #[must_use]
    pub fn last_name(self) -> &'a str {
        &self.speaker.last_name
    }

    /// Full name.
    #[must_use]
    pub fn full_name(self) -> &'a str {
        &self.speaker.full_name
    }

    /// Biography.
    #[must_use]
    pub fn bio(self) -> Option<&'a str> {
        self.speaker.bio.as_deref()
    }

    /// Tagline, such as a job title. May be empty.
    #[must_use]
    pub fn tag_line(self) -> Option<&'a str> {
        self.speaker.tag_line.as_deref()
    }

    /// Profile picture.
    #[must_use]
    pub fn profile_picture(self) -> Option<&'a Url> {
        self.speaker.profile_picture.as_ref()
    }

    /// Whether the organizer marked the speaker as a top speaker.
    #[must_use]
    pub fn is_top_speaker(self) -> bool {
        self.speaker.is_top_speaker
    }

    /// Links on the speaker's profile.
    #[must_use]
    pub fn links(self) -> &'a [Link] {
        &self.speaker.links
    }

    /// Sessions of the speaker.
    pub fn sessions(self) -> impl Iterator<Item = Session<'a>> {
        let event = self.event;
        self.speaker
            .sessions
            .iter()
            .filter_map(move |id| event.session(id))
    }

    /// Categories with selected items, in the organizer's order.
    #[must_use]
    pub fn categories(self) -> Vec<CategorySelection<'a>> {
        self.event.selected_categories(&self.speaker.category_items)
    }

    /// Answered questions, in the organizer's order.
    #[must_use]
    pub fn answers(self) -> Vec<Answer<'a>> {
        self.event.answers(
            &self.speaker.question_answers,
            self.event.speaker_file_names.get(&self.speaker.id),
        )
    }

    /// Deserializes the speaker's custom fields into `T`.
    ///
    /// All speaker categories are included, with no items if none are
    /// selected. See [custom fields](crate#custom-fields).
    ///
    /// # Errors
    ///
    /// Returns an error if the custom fields don't match `T`.
    pub fn custom_fields<T: DeserializeOwned>(self) -> Result<T, CustomFieldsError> {
        self.event.custom_fields(
            &self.answers(),
            &CategoryType::Speaker,
            &self.speaker.category_items,
        )
    }
}

/// A category with the items selected for a session or speaker.
#[derive(Debug, Clone)]
pub struct CategorySelection<'a> {
    category: &'a Category,
    items: Vec<&'a CategoryItem>,
}

impl<'a> CategorySelection<'a> {
    /// The category.
    #[must_use]
    pub fn category(&self) -> &'a Category {
        self.category
    }

    /// The selected items, in the organizer's order.
    #[must_use]
    pub fn items(&self) -> &[&'a CategoryItem] {
        &self.items
    }
}

/// An answer to a question.
#[derive(Debug, Clone, Copy)]
pub struct Answer<'a> {
    question: &'a Question,
    text: &'a str,
    file_name: Option<&'a str>,
}

impl<'a> Answer<'a> {
    /// The question.
    #[must_use]
    pub fn question(self) -> &'a Question {
        self.question
    }

    /// The answer as sent by the API.
    #[must_use]
    pub fn answer(self) -> &'a str {
        self.text
    }

    /// The original file name of a file upload, if the Sessions or Speakers
    /// view was merged into the event.
    #[must_use]
    pub fn file_name(self) -> Option<&'a str> {
        self.file_name
    }

    /// Interprets the answer according to the question type.
    ///
    /// # Errors
    ///
    /// Returns an error if the answer doesn't match the question type.
    pub fn value(self) -> Result<AnswerValue, ParseAnswerError> {
        AnswerValue::parse(&self.question.question_type, self.text, self.file_name)
    }
}

/// A day of an event's [schedule](Event::schedule).
#[derive(Debug, Clone)]
pub struct Day<'a> {
    date: NaiveDate,
    time_slots: Vec<TimeSlot<'a>>,
}

impl<'a> Day<'a> {
    fn add(&mut self, start: ScheduleTime, session: Session<'a>) {
        match self.time_slots.last_mut() {
            Some(slot) if slot.start.naive() == start.naive() => slot.sessions.push(session),
            _ => self.time_slots.push(TimeSlot {
                start,
                sessions: vec![session],
            }),
        }
    }

    /// Date of the day.
    #[must_use]
    pub fn date(&self) -> NaiveDate {
        self.date
    }

    /// Time slots of the day, in order.
    #[must_use]
    pub fn time_slots(&self) -> &[TimeSlot<'a>] {
        &self.time_slots
    }

    /// Sessions of the day, ordered by start time and room.
    pub fn sessions(&self) -> impl Iterator<Item = Session<'a>> + '_ {
        self.time_slots
            .iter()
            .flat_map(|slot| slot.sessions.iter().copied())
    }

    /// Rooms used on the day, in the organizer's order.
    #[must_use]
    pub fn rooms(&self) -> Vec<&'a Room> {
        let mut rooms: Vec<&Room> = self.sessions().filter_map(Session::room).collect();
        rooms.sort_by_key(|room| (room.sort, room.id));
        rooms.dedup_by_key(|room| room.id);

        rooms
    }
}

/// Sessions starting at the same time.
#[derive(Debug, Clone)]
pub struct TimeSlot<'a> {
    start: ScheduleTime,
    sessions: Vec<Session<'a>>,
}

impl<'a> TimeSlot<'a> {
    /// Start time of the slot.
    #[must_use]
    pub fn start(&self) -> ScheduleTime {
        self.start
    }

    /// Sessions starting in the slot, in room order.
    #[must_use]
    pub fn sessions(&self) -> &[Session<'a>] {
        &self.sessions
    }
}

/// An [`Event`] with custom fields deserialized into `S` for sessions and `P`
/// for speakers.
///
/// Created by [`Event::into_typed`].
#[derive(Debug, Clone)]
pub struct TypedEvent<S, P> {
    event: Event,
    sessions: HashMap<SessionId, S>,
    speakers: HashMap<SpeakerId, P>,
}

impl<S, P> TypedEvent<S, P> {
    /// Returns the event.
    #[must_use]
    pub fn event(&self) -> &Event {
        &self.event
    }

    /// Returns the event, dropping the custom fields.
    #[must_use]
    pub fn into_event(self) -> Event {
        self.event
    }

    /// Returns the custom fields of a regular session.
    #[must_use]
    pub fn session_fields(&self, id: &SessionId) -> Option<&S> {
        self.sessions.get(id)
    }

    /// Returns the custom fields of a speaker.
    #[must_use]
    pub fn speaker_fields(&self, id: &SpeakerId) -> Option<&P> {
        self.speakers.get(id)
    }

    /// Returns the regular sessions with their custom fields, ordered by start
    /// time and room.
    pub fn sessions(&self) -> impl Iterator<Item = (Session<'_>, &S)> {
        self.event
            .sessions()
            .filter_map(|session| Some((session, self.sessions.get(session.id())?)))
    }

    /// Returns the speakers with their custom fields.
    pub fn speakers(&self) -> impl Iterator<Item = (Speaker<'_>, &P)> {
        self.event
            .speakers()
            .filter_map(|speaker| Some((speaker, self.speakers.get(speaker.id())?)))
    }
}

/// Error returned by [`Event::into_typed`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TypedEventError {
    /// The custom fields of a session don't match the session type.
    #[error("invalid custom fields of session {id}")]
    #[non_exhaustive]
    Session {
        /// Identifier of the session.
        id: SessionId,
        /// The error.
        source: CustomFieldsError,
    },
    /// The custom fields of a speaker don't match the speaker type.
    #[error("invalid custom fields of speaker {id}")]
    #[non_exhaustive]
    Speaker {
        /// Identifier of the speaker.
        id: SpeakerId,
        /// The error.
        source: CustomFieldsError,
    },
}
