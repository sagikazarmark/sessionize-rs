//! Deserializes every view of every fixture into the models.

mod support;

use std::collections::HashSet;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sessionize::{
    All, AnswerValue, CategoryType, GroupId, LinkType, QuestionType, RoomId, SessionGroup,
    SessionId, SessionStatus, Speaker, SpeakerId, WallSpeaker, grid,
};
use support::{DEMO, EVENTS, KCD, KCD_CUSTOM_FIELDS, fixture, read};

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
    event: &str,
    view: &str,
) {
    let value: T = fixture(event, view);
    let json = serde_json::to_string(&value).unwrap();
    let again: T = serde_json::from_str(&json).unwrap();

    assert_eq!(value, again, "{event}/{view}");
}

#[test]
fn every_view_round_trips() {
    for event in EVENTS {
        round_trip::<All>(event, "All");
        round_trip::<Vec<grid::Day>>(event, "GridSmart");
        round_trip::<Vec<SessionGroup>>(event, "Sessions");
        round_trip::<Vec<Speaker>>(event, "Speakers");
        round_trip::<Vec<WallSpeaker>>(event, "SpeakerWall");
    }
}

/// Serializing reproduces the API's JSON. The `GridSmart` view is left out: its
/// sessions have no `questionAnswers`, which the shared session type adds.
#[test]
fn serialization_reproduces_the_api_format() {
    fn check<T: Serialize + DeserializeOwned>(event: &str, view: &str) {
        let original: Value = serde_json::from_str(&read(event, view)).unwrap();
        let value: T = fixture(event, view);
        let serialized = serde_json::to_value(value).unwrap();

        if let Some(difference) = first_difference(&original, &serialized, "") {
            panic!("{event}/{view}: {difference}");
        }
    }

    for event in EVENTS {
        check::<All>(event, "All");
        check::<Vec<SessionGroup>>(event, "Sessions");
        check::<Vec<Speaker>>(event, "Speakers");
        check::<Vec<WallSpeaker>>(event, "SpeakerWall");
    }
}

fn first_difference(expected: &Value, actual: &Value, path: &str) -> Option<String> {
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => {
            for key in expected.keys().chain(actual.keys()) {
                let path = format!("{path}.{key}");
                match (expected.get(key), actual.get(key)) {
                    (Some(e), Some(a)) => {
                        if let Some(difference) = first_difference(e, a, &path) {
                            return Some(difference);
                        }
                    }
                    (e, a) => return Some(format!("{path}: expected {e:?}, got {a:?}")),
                }
            }
            None
        }
        (Value::Array(expected), Value::Array(actual)) if expected.len() == actual.len() => {
            expected
                .iter()
                .zip(actual)
                .enumerate()
                .find_map(|(i, (e, a))| first_difference(e, a, &format!("{path}[{i}]")))
        }
        _ if expected == actual => None,
        _ => Some(format!("{path}: expected {expected}, got {actual}")),
    }
}

#[test]
fn fixtures_have_no_unknown_values() {
    for event in EVENTS {
        let all: All = fixture(event, "All");

        for question in &all.questions {
            assert!(
                !matches!(question.question_type, QuestionType::Unknown(_)),
                "{event}: {:?}",
                question.question_type
            );
        }
        for category in &all.categories {
            assert!(!matches!(category.category_type, CategoryType::Unknown(_)));
        }
        for session in &all.sessions {
            assert!(!matches!(session.status, Some(SessionStatus::Unknown(_))));
        }
        for link in all.speakers.iter().flat_map(|s| &s.links) {
            assert!(!matches!(link.link_type, LinkType::Unknown(_)), "{link:?}");
        }
    }
}

#[test]
fn all_view_references_resolve() {
    for event in EVENTS {
        let all: All = fixture(event, "All");

        for session in &all.sessions {
            for id in &session.speakers {
                assert!(all.speaker(id).is_some(), "{event}: {id}");
            }
            if let Some(id) = session.room_id {
                assert!(all.room(id).is_some(), "{event}: {id}");
            }
            for &id in &session.category_items {
                assert!(all.category_item(id).is_some(), "{event}: {id}");
            }
            for answer in &session.question_answers {
                let question = all.question(answer.question_id).unwrap();
                answer.value(question).unwrap();
            }
        }

        for speaker in &all.speakers {
            // Speaker → session references are integers, session IDs strings.
            for id in &speaker.sessions {
                assert!(all.session(id).is_some(), "{event}: {id}");
            }
            assert_eq!(
                all.sessions_of(&speaker.id).count(),
                speaker.sessions.len(),
                "{event}: {}",
                speaker.id
            );
            for answer in &speaker.question_answers {
                let question = all.question(answer.question_id).unwrap();
                answer.value(question).unwrap();
            }
        }
    }
}

#[test]
fn speakers_view_matches_all_view() {
    for event in EVENTS {
        let all: All = fixture(event, "All");
        let speakers: Vec<Speaker> = fixture(event, "Speakers");

        assert_eq!(speakers.len(), all.speakers.len());
        for speaker in &speakers {
            let expected = all.speaker(&speaker.id).unwrap();
            let sessions: Vec<_> = speaker.sessions.iter().map(|s| s.id.clone()).collect();

            assert_eq!(sessions, expected.sessions, "{event}: {}", speaker.id);
            assert_eq!(speaker.links, expected.links);
            assert_eq!(speaker.profile_picture, expected.profile_picture);
        }
    }
}

#[test]
fn grid_includes_service_sessions_missing_from_all() {
    let all: All = fixture(KCD, "All");
    let days: Vec<grid::Day> = fixture(KCD, "GridSmart");

    let grid_sessions: Vec<_> = days
        .iter()
        .flat_map(|day| &day.rooms)
        .flat_map(|room| room.sessions.iter().map(move |session| (room.id, session)))
        .collect();
    let missing: Vec<_> = grid_sessions
        .iter()
        .filter(|(_, session)| all.session(&session.id).is_none())
        .collect();

    assert_eq!(all.sessions.len(), 24);
    assert_eq!(grid_sessions.len(), 34);
    assert_eq!(missing.len(), 10);
    for (room, session) in missing {
        assert!(session.is_service_session, "{}", session.title);
        assert_eq!(session.room_id, Some(*room));
        assert_eq!(session.status, None);
        // Service sessions have UUIDs.
        assert!(session.id.as_str().contains('-'), "{}", session.id);
    }
}

#[test]
fn grid_service_session() {
    let days: Vec<grid::Day> = fixture(KCD, "GridSmart");
    let day = &days[0];
    let slot = &day.time_slots[0];
    let registration = &slot.rooms[0].session;

    assert_eq!(day.date.to_string(), "2025-04-24");
    assert_eq!(day.time_slots.len(), 25);
    assert_eq!(slot.start.to_string(), "08:00:00");
    assert_eq!(registration.title, "Registration");
    assert_eq!(registration.description, None);
    assert_eq!(registration.room.as_deref(), Some("Room 1"));
    assert_eq!(
        registration.starts_at.unwrap().to_string(),
        "2025-04-24T08:00:00"
    );
    assert!(registration.speakers.is_empty());
}

#[test]
fn schedule_times_match_endpoint_setting() {
    for (event, utc) in [(KCD, false), (KCD_CUSTOM_FIELDS, false), (DEMO, true)] {
        let all: All = fixture(event, "All");

        for session in &all.sessions {
            assert_eq!(session.starts_at.unwrap().is_utc(), utc, "{event}");
            assert_eq!(session.ends_at.unwrap().is_utc(), utc, "{event}");
        }
    }
}

#[test]
fn demo_event() {
    let all: All = fixture(DEMO, "All");
    let keynote = all.session(&SessionId::new("14022")).unwrap();
    let aiden = all.speakers_of(keynote).next().unwrap();

    assert_eq!(keynote.title, "Aiden's Keynote");
    assert_eq!(keynote.status, Some(SessionStatus::Accepted));
    assert_eq!(keynote.room_id, Some(RoomId::new(215)));
    assert_eq!(all.room(RoomId::new(215)).unwrap().name, "Green Room");
    assert_eq!(aiden.full_name, "Aiden Test");
    assert_eq!(
        aiden.links.iter().map(|l| &l.link_type).collect::<Vec<_>>(),
        [&LinkType::Twitter, &LinkType::LinkedIn]
    );

    let categories: Vec<_> = keynote
        .category_items
        .iter()
        .map(|&id| {
            let (category, item) = all.category_item(id).unwrap();
            (category.title.as_str(), item.name.as_str())
        })
        .collect();
    assert_eq!(
        categories,
        [
            ("Session format", "Session"),
            ("Track", "Technical"),
            ("Level", "Intermediate"),
            ("Language", "Italian")
        ]
    );

    let answer = &keynote.question_answers[0];
    let question = all.question(answer.question_id).unwrap();
    assert_eq!(question.question, "Demo");
    assert_eq!(answer.value(question), Ok(AnswerValue::Percentage(67)));

    let lunch = all.sessions.iter().find(|s| s.title == "Lunch").unwrap();
    assert!(lunch.is_service_session);
    assert_eq!(lunch.status, None);
}

#[test]
fn sessions_view_groups() {
    let groups: Vec<SessionGroup> = fixture(DEMO, "Sessions");
    let names: Vec<_> = groups.iter().map(|g| g.name.as_str()).collect();

    assert_eq!(names, ["Technical", "Scientific", "Business"]);
    assert_eq!(groups[0].id, Some(GroupId::new("10663")));

    let groups: Vec<SessionGroup> = fixture(KCD, "Sessions");

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].id, None);
    assert_eq!(groups[0].name, "All");
}

#[test]
fn session_question_answers() {
    let groups: Vec<SessionGroup> = fixture(KCD_CUSTOM_FIELDS, "Sessions");
    let session = groups[0]
        .sessions
        .iter()
        .find(|s| s.id == SessionId::new("886030"))
        .unwrap();

    let values: Vec<_> = session
        .question_answers
        .iter()
        .map(|answer| (answer.question.as_str(), answer.value().unwrap().unwrap()))
        .collect();

    let AnswerValue::File(file) = &values[0].1 else {
        panic!("{values:?}")
    };
    assert_eq!(values[0].0, "Test file upload");
    assert_eq!(file.file_name.as_deref(), Some("KCD_Budapest_logo-01.svg"));
    assert_eq!(
        values[1..],
        [
            (
                "Test web address",
                AnswerValue::WebAddress("https://sagikazarmark.com".to_owned())
            ),
            ("Test checkbox", AnswerValue::Checkbox(true)),
            ("Test unchecked checkbox", AnswerValue::Checkbox(false)),
            (
                "Test email address",
                AnswerValue::Email("mark@mark.com".to_owned())
            ),
            (
                "Test long text",
                AnswerValue::Text("asdasda\r\n\r\n\r\nasdasdsaa\r\n\r\n\r\nadsadasd".to_owned())
            ),
            ("Test integer", AnswerValue::Integer(12345)),
        ]
    );

    let categories: Vec<_> = session
        .categories
        .iter()
        .map(|c| {
            (
                c.name.as_str(),
                c.items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        categories,
        [
            ("Test single choice", vec!["choice1"]),
            ("Test multiple choice", vec!["choice1", "choice2"])
        ]
    );

    // Other sessions list the questions unanswered.
    let other = groups[0]
        .sessions
        .iter()
        .find(|s| s.id != session.id)
        .unwrap();
    assert_eq!(other.question_answers.len(), 7);
    assert!(other.question_answers.iter().all(|a| a.value() == Ok(None)));
}

#[test]
fn speaker_file_upload() {
    let speakers: Vec<Speaker> = fixture(KCD_CUSTOM_FIELDS, "Speakers");
    let viktor = speakers
        .iter()
        .find(|s| s.id == SpeakerId::new("08521b30-d304-498f-8bbf-627d5f9ffba4"))
        .unwrap();
    let answer = viktor
        .question_answers
        .iter()
        .find(|a| a.question == "Test speaker file")
        .unwrap();

    let Some(AnswerValue::File(file)) = answer.value().unwrap() else {
        panic!("{answer:?}")
    };
    assert_eq!(
        file.file_name.as_deref(),
        Some("KCD_Budapest_logo-01-2.svg")
    );
    assert!(file.url.as_str().ends_with("~kcd_budapest_logo-01-2.svg"));
}

#[test]
fn speaker_wall() {
    for event in EVENTS {
        let wall: Vec<WallSpeaker> = fixture(event, "SpeakerWall");
        let all: All = fixture(event, "All");
        let ids: HashSet<_> = all.speakers.iter().map(|s| &s.id).collect();

        assert!(!wall.is_empty());
        assert!(wall.iter().all(|s| ids.contains(&s.id)), "{event}");
    }
}
