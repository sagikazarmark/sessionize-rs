//! Builds events from fixtures.

mod support;

use std::collections::HashMap;

use serde::Deserialize;
use serde::de::IgnoredAny;
use sessionize::{
    AnswerValue, Event, FileAnswer, SessionGroup, SessionId, Speaker, SpeakerId, TypedEventError,
    grid,
};
use support::{DEMO, EVENTS, KCD, KCD_CUSTOM_FIELDS, event, fixture};

const SESSION: &str = "886030";
const VIKTOR: &str = "08521b30-d304-498f-8bbf-627d5f9ffba4";

#[test]
fn merging_the_grid_adds_service_sessions() {
    let all_only = Event::from(fixture::<sessionize::All>(KCD, "All"));
    let event = event(KCD, false);

    assert_eq!(all_only.sessions().len(), 24);
    assert_eq!(event.sessions().len(), 34);
    assert_eq!(
        event.sessions().filter(|s| s.is_service_session()).count(),
        10
    );
}

#[test]
fn merging_is_idempotent() {
    let mut event = event(KCD_CUSTOM_FIELDS, true);
    let sessions = event.sessions().len();
    let speakers = event.speakers().len();

    event.merge_grid(&fixture::<Vec<grid::Day>>(KCD_CUSTOM_FIELDS, "GridSmart"));
    event.merge_sessions(&fixture::<Vec<SessionGroup>>(KCD_CUSTOM_FIELDS, "Sessions"));
    event.merge_speakers(&fixture::<Vec<Speaker>>(KCD_CUSTOM_FIELDS, "Speakers"));

    assert_eq!(event.sessions().len(), sessions);
    assert_eq!(event.speakers().len(), speakers);
}

#[test]
fn sessions_are_ordered_by_start_and_room() {
    for name in EVENTS {
        let event = event(name, false);
        let keys: Vec<_> = event
            .sessions()
            .map(|s| (s.starts_at().unwrap().naive(), s.room().unwrap().sort))
            .collect();

        assert!(keys.is_sorted(), "{name}");
    }
}

/// The computed schedule matches the `GridSmart` view: same days, same time
/// slots, same sessions in each slot.
#[test]
fn schedule_matches_grid_view() {
    for name in EVENTS {
        let event = event(name, false);
        let days: Vec<grid::Day> = fixture(name, "GridSmart");

        let expected: Vec<_> = days
            .iter()
            .map(|day| {
                let slots: Vec<_> = day
                    .time_slots
                    .iter()
                    .map(|slot| {
                        let mut ids: Vec<_> = slot
                            .rooms
                            .iter()
                            .map(|room| room.session.id.clone())
                            .collect();
                        ids.sort();
                        (slot.start, ids)
                    })
                    .collect();
                (day.date, slots)
            })
            .collect();
        let actual: Vec<_> = event
            .schedule()
            .iter()
            .map(|day| {
                let slots: Vec<_> = day
                    .time_slots()
                    .iter()
                    .map(|slot| {
                        let mut ids: Vec<_> =
                            slot.sessions().iter().map(|s| s.id().clone()).collect();
                        ids.sort();
                        (slot.start().time(), ids)
                    })
                    .collect();
                (day.date(), slots)
            })
            .collect();

        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn schedule_days() {
    let event = event(DEMO, false);
    let schedule = event.schedule();

    assert_eq!(schedule.len(), 2);
    let rooms: Vec<_> = schedule[0]
        .rooms()
        .iter()
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(rooms, ["Green Room", "Yellow Room", "Restaurant"]);

    let first = schedule[0].sessions().next().unwrap();
    assert_eq!(first.title(), "Aiden's Keynote");
    assert_eq!(
        first.starts_at().unwrap().to_string(),
        "2023-09-16T09:00:00Z"
    );
}

#[test]
fn session_relations() {
    let event = event(DEMO, false);
    let keynote = event.session(&SessionId::new("14022")).unwrap();

    assert_eq!(keynote.room().unwrap().name, "Green Room");

    let speakers: Vec<_> = keynote
        .speakers()
        .map(sessionize::event::Speaker::full_name)
        .collect();
    assert_eq!(speakers, ["Aiden Test"]);

    let aiden = keynote.speakers().next().unwrap();
    let sessions: Vec<_> = aiden
        .sessions()
        .map(sessionize::event::Session::title)
        .collect();
    assert_eq!(sessions, ["Aiden's Keynote"]);
    assert_eq!(aiden.links().len(), 2);

    let categories: Vec<_> = keynote
        .categories()
        .iter()
        .map(|c| {
            (
                c.category().title.as_str(),
                c.items()
                    .iter()
                    .map(|i| i.name.as_str())
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        categories,
        [
            ("Session format", vec!["Session"]),
            ("Track", vec!["Technical"]),
            ("Level", vec!["Intermediate"]),
            ("Language", vec!["Italian"]),
        ]
    );

    let answers = keynote.answers();
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].question().question, "Demo");
    assert_eq!(answers[0].value(), Ok(AnswerValue::Percentage(67)));
}

#[test]
fn file_names_require_the_sessions_and_speakers_views() {
    let file_name = |event: &Event| {
        let session = event.session(&SessionId::new(SESSION)).unwrap();
        let speaker = event.speaker(&SpeakerId::new(VIKTOR)).unwrap();
        let session_file = session.answers()[0].file_name().map(str::to_owned);
        let speaker_file = speaker
            .answers()
            .iter()
            .find(|a| a.question().question == "Test speaker file")
            .unwrap()
            .file_name()
            .map(str::to_owned);
        (session_file, speaker_file)
    };

    assert_eq!(file_name(&event(KCD_CUSTOM_FIELDS, false)), (None, None));
    assert_eq!(
        file_name(&event(KCD_CUSTOM_FIELDS, true)),
        (
            Some("KCD_Budapest_logo-01.svg".to_owned()),
            Some("KCD_Budapest_logo-01-2.svg".to_owned())
        )
    );
}

#[derive(Debug, Deserialize, PartialEq)]
enum Choice {
    #[serde(rename = "choice1")]
    One,
    #[serde(rename = "choice2")]
    Two,
}

#[derive(Debug, Deserialize)]
struct TestFields {
    #[serde(rename = "Test file upload")]
    file: Option<FileAnswer>,
    #[serde(rename = "Test web address")]
    web_address: Option<url::Url>,
    #[serde(rename = "Test checkbox", default)]
    checkbox: bool,
    #[serde(rename = "Test unchecked checkbox", default)]
    unchecked: bool,
    #[serde(rename = "Test email address")]
    email: Option<String>,
    #[serde(rename = "Test long text")]
    long_text: Option<String>,
    #[serde(rename = "Test integer")]
    integer: Option<i64>,
    #[serde(rename = "Test single choice")]
    single: Option<Choice>,
    #[serde(rename = "Test multiple choice")]
    multiple: Vec<Choice>,
}

#[test]
fn session_custom_fields() {
    let event = event(KCD_CUSTOM_FIELDS, true);
    let fields: TestFields = event
        .session(&SessionId::new(SESSION))
        .unwrap()
        .custom_fields()
        .unwrap();

    let file = fields.file.unwrap();
    assert_eq!(file.file_name.as_deref(), Some("KCD_Budapest_logo-01.svg"));
    assert_eq!(
        fields.web_address.unwrap().as_str(),
        "https://sagikazarmark.com/"
    );
    assert!(fields.checkbox);
    assert!(!fields.unchecked);
    assert_eq!(fields.email.as_deref(), Some("mark@mark.com"));
    assert!(fields.long_text.unwrap().contains("\r\n"));
    assert_eq!(fields.integer, Some(12345));
    assert_eq!(fields.single, Some(Choice::One));
    assert_eq!(fields.multiple, [Choice::One, Choice::Two]);
}

#[test]
fn unanswered_custom_fields() {
    let event = event(KCD_CUSTOM_FIELDS, true);
    let session = event
        .sessions()
        .find(|s| !s.is_service_session() && s.id().as_str() != SESSION)
        .unwrap();
    let fields: TestFields = session.custom_fields().unwrap();

    assert!(fields.file.is_none());
    assert!(!fields.checkbox);
    assert_eq!(fields.integer, None);
    assert_eq!(fields.single, None);
    assert!(fields.multiple.is_empty());
}

#[test]
fn speaker_custom_fields() {
    #[derive(Deserialize)]
    struct SpeakerFields {
        #[serde(rename = "Job Title")]
        job_title: Option<String>,
        #[serde(rename = "Company")]
        company: Option<String>,
        #[serde(rename = "Test speaker file")]
        file: Option<FileAnswer>,
    }

    let event = event(KCD_CUSTOM_FIELDS, true);
    let fields: SpeakerFields = event
        .speaker(&SpeakerId::new(VIKTOR))
        .unwrap()
        .custom_fields()
        .unwrap();

    assert_eq!(fields.job_title.as_deref(), Some("Product"));
    assert_eq!(fields.company.as_deref(), Some("ex-GitLab"));
    assert_eq!(
        fields.file.unwrap().file_name.as_deref(),
        Some("KCD_Budapest_logo-01-2.svg")
    );
}

/// The `All` view and the Sessions/Speakers views present custom fields the
/// same way.
#[test]
fn custom_fields_match_between_views() {
    type Fields = HashMap<String, serde_json::Value>;

    let event = event(KCD_CUSTOM_FIELDS, true);

    let groups: Vec<SessionGroup> = fixture(KCD_CUSTOM_FIELDS, "Sessions");
    for session in groups.iter().flat_map(|g| &g.sessions) {
        let from_view: Fields = session.custom_fields().unwrap();
        let from_event: Fields = event.session(&session.id).unwrap().custom_fields().unwrap();

        assert_eq!(from_view, from_event, "{}", session.id);
    }

    let speakers: Vec<Speaker> = fixture(KCD_CUSTOM_FIELDS, "Speakers");
    for speaker in &speakers {
        let from_view: Fields = speaker.custom_fields().unwrap();
        let from_event: Fields = event.speaker(&speaker.id).unwrap().custom_fields().unwrap();

        assert_eq!(from_view, from_event, "{}", speaker.id);
    }
}

#[test]
fn typed_event() {
    let event = event(KCD_CUSTOM_FIELDS, true);
    let typed = event.into_typed::<TestFields, IgnoredAny>().unwrap();

    assert_eq!(typed.sessions().count(), 24);
    assert_eq!(typed.speakers().count(), 25);

    let fields = typed.session_fields(&SessionId::new(SESSION)).unwrap();
    assert_eq!(fields.integer, Some(12345));
}

#[test]
fn typed_event_reports_the_failing_session() {
    #[derive(Debug, Deserialize)]
    struct Required {
        #[serde(rename = "Test integer")]
        _integer: i64,
    }

    let err = event(KCD_CUSTOM_FIELDS, false)
        .into_typed::<Required, IgnoredAny>()
        .unwrap_err();

    let TypedEventError::Session { id, source, .. } = &err else {
        panic!("{err:?}")
    };
    assert_ne!(id.as_str(), SESSION);
    assert_eq!(source.field(), Some("Test integer"));
}

#[test]
fn file_answers_match_between_views() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Files {
        #[serde(rename = "Test file upload")]
        session: Option<FileAnswer>,
    }

    let event = event(KCD_CUSTOM_FIELDS, true);
    let groups: Vec<SessionGroup> = fixture(KCD_CUSTOM_FIELDS, "Sessions");
    let session = groups[0]
        .sessions
        .iter()
        .find(|s| s.id.as_str() == SESSION)
        .unwrap();

    let from_view: Files = session.custom_fields().unwrap();
    let from_event: Files = event.session(&session.id).unwrap().custom_fields().unwrap();

    assert!(from_view.session.is_some());
    assert_eq!(from_view, from_event);
}

/// Sessions without a time or room, as sent when the schedule isn't announced.
fn unannounced() -> Event {
    let all: sessionize::All = serde_json::from_value(serde_json::json!({
        "sessions": [
            {
                "id": "1", "title": "Unscheduled (null)", "startsAt": null, "endsAt": null,
                "roomId": null, "isServiceSession": false, "isPlenumSession": false,
                "isInformed": true, "isConfirmed": true
            },
            {
                "id": "2", "title": "Unscheduled (missing)", "isServiceSession": false,
                "isPlenumSession": false, "isInformed": true, "isConfirmed": true
            },
            {
                "id": "3", "title": "Scheduled", "startsAt": "2025-04-24T09:00:00",
                "endsAt": "2025-04-24T09:30:00", "roomId": 1, "speakers": ["a"],
                "categoryItems": [10, 20], "isServiceSession": false, "isPlenumSession": false,
                "isInformed": true, "isConfirmed": true
            }
        ],
        "speakers": [
            {
                "id": "a", "firstName": "Ada", "lastName": "L", "fullName": "Ada L",
                "isTopSpeaker": false, "sessions": [3], "categoryItems": [21]
            }
        ],
        "rooms": [{ "id": 1, "name": "Main", "sort": 0 }],
        "categories": [
            {
                "id": 1, "title": "Track", "sort": 0, "type": "session",
                "items": [{ "id": 10, "name": "Rust", "sort": 0 }]
            },
            {
                "id": 2, "title": "Seniority", "sort": 1, "type": "speaker",
                "items": [{ "id": 20, "name": "Senior", "sort": 0 }, { "id": 21, "name": "Junior", "sort": 1 }]
            }
        ]
    }))
    .unwrap();

    Event::from(all)
}

#[test]
fn unscheduled_sessions_come_last_and_are_not_in_the_schedule() {
    let event = unannounced();

    let titles: Vec<_> = event
        .sessions()
        .map(sessionize::event::Session::title)
        .collect();
    assert_eq!(
        titles,
        ["Scheduled", "Unscheduled (null)", "Unscheduled (missing)"]
    );

    let unscheduled = event.session(&SessionId::new("2")).unwrap();
    assert_eq!(unscheduled.starts_at(), None);
    assert!(unscheduled.room().is_none());

    let schedule = event.schedule();
    assert_eq!(schedule.len(), 1);
    assert_eq!(schedule[0].sessions().count(), 1);
}

#[test]
fn speaker_categories() {
    #[derive(Deserialize)]
    struct SessionFields {
        #[serde(rename = "Track")]
        track: Option<String>,
        #[serde(rename = "Seniority")]
        seniority: Option<String>,
    }

    #[derive(Deserialize)]
    struct SpeakerFields {
        #[serde(rename = "Seniority")]
        seniority: String,
    }

    let event = unannounced();
    let speaker = event.speaker(&SpeakerId::new("a")).unwrap();
    let session = event.session(&SessionId::new("3")).unwrap();

    let categories: Vec<_> = speaker
        .categories()
        .iter()
        .map(|c| c.category().title.clone())
        .collect();
    assert_eq!(categories, ["Seniority"]);

    let speaker_fields: SpeakerFields = speaker.custom_fields().unwrap();
    assert_eq!(speaker_fields.seniority, "Junior");

    // Session custom fields only include session categories, even though the
    // session (wrongly) references an item of a speaker category.
    let session_fields: SessionFields = session.custom_fields().unwrap();
    assert_eq!(session_fields.track.as_deref(), Some("Rust"));
    assert_eq!(session_fields.seniority, None);
}
