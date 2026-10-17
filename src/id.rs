//! Identifiers of Sessionize entities.

use std::borrow::Borrow;
use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            /// Creates an identifier from its string form.
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            /// Returns the identifier as a string slice.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Returns the underlying string.
            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(id: String) -> Self {
                Self(id)
            }
        }

        impl From<&str> for $name {
            fn from(id: &str) -> Self {
                Self(id.to_owned())
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

macro_rules! numeric_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(u64);

        impl $name {
            /// Creates an identifier from its numeric value.
            #[must_use]
            pub const fn new(id: u64) -> Self {
                Self(id)
            }

            /// Returns the numeric value of the identifier.
            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl FromStr for $name {
            type Err = ParseIntError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                s.parse().map(Self)
            }
        }

        impl From<u64> for $name {
            fn from(id: u64) -> Self {
                Self(id)
            }
        }

        impl From<$name> for u64 {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

string_id! {
    /// Identifier of a session.
    ///
    /// Regular sessions have numeric identifiers (`"886030"`), service sessions
    /// (breaks, lunch, registration) have UUIDs.
    ///
    /// The API sends session identifiers as strings, except in speaker → session
    /// references where they are integers. Both forms deserialize into the same
    /// value, and it always serializes as a string.
    ///
    /// ```
    /// # use sessionize::SessionId;
    /// let from_string: SessionId = serde_json::from_str(r#""886030""#).unwrap();
    /// let from_integer: SessionId = serde_json::from_str("886030").unwrap();
    /// assert_eq!(from_string, from_integer);
    /// assert_eq!(from_integer.as_str(), "886030");
    /// ```
    SessionId
}

string_id! {
    /// Identifier of a speaker (a UUID).
    SpeakerId
}

numeric_id! {
    /// Identifier of a room.
    RoomId
}

numeric_id! {
    /// Identifier of a category (a custom field such as "Level" or "Track").
    CategoryId
}

numeric_id! {
    /// Identifier of a category item (one option of a category, such as "Advanced").
    CategoryItemId
}

numeric_id! {
    /// Identifier of a question (a custom field such as "Job Title").
    QuestionId
}

string_id! {
    /// Identifier of a group in the Sessions view.
    ///
    /// What it identifies depends on the endpoint's "Group list by" setting,
    /// for example a category item. Deserializes from a string or an integer.
    GroupId
}

impl<'de> Deserialize<'de> for SessionId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer
            .deserialize_any(StringOrInteger("a session id as a string or an integer"))
            .map(Self)
    }
}

impl<'de> Deserialize<'de> for GroupId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer
            .deserialize_any(StringOrInteger("a group id as a string or an integer"))
            .map(Self)
    }
}

/// Deserializes a string or an integer into a string.
struct StringOrInteger(&'static str);
impl de::Visitor<'_> for StringOrInteger {
    type Value = String;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(v.to_owned())
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(v)
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(v.to_string())
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        Ok(v.to_string())
    }
}

impl Serialize for SessionId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl Serialize for SpeakerId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// Numeric group IDs serialize as integers, like the API sends them.
impl Serialize for GroupId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_integer_or_str(&self.0, serializer)
    }
}

/// Serializes an ID as an integer if it's an integer's canonical form (no
/// leading zeros), otherwise as a string.
fn serialize_integer_or_str<S: Serializer>(id: &str, serializer: S) -> Result<S::Ok, S::Error> {
    match id.parse::<u64>() {
        Ok(number) if number.to_string() == id => serializer.serialize_u64(number),
        _ => serializer.serialize_str(id),
    }
}

impl<'de> Deserialize<'de> for SpeakerId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self)
    }
}

/// Serializes session references from speakers as integers, like the API.
pub(crate) mod session_ref {
    use serde::ser::SerializeSeq;
    use serde::{Serialize, Serializer};

    use super::SessionId;

    pub(crate) fn serialize<S: Serializer>(
        id: &SessionId,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        super::serialize_integer_or_str(id.as_str(), serializer)
    }

    pub(crate) fn serialize_all<S: Serializer>(
        ids: &[SessionId],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        struct Ref<'a>(&'a SessionId);

        impl Serialize for Ref<'_> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serialize(self.0, serializer)
            }
        }

        let mut seq = serializer.serialize_seq(Some(ids.len()))?;
        for id in ids {
            seq.serialize_element(&Ref(id))?;
        }
        seq.end()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn session_id_deserializes_from_string_and_integer() {
        let ids: Vec<SessionId> =
            serde_json::from_str(r#"["886030", 886030, "13f26cd2-8c69-4100-8abe-84cf0a04ccb9"]"#)
                .unwrap();

        assert_eq!(ids[0], ids[1]);
        assert_eq!(ids[0].as_str(), "886030");
        assert_eq!(ids[2].as_str(), "13f26cd2-8c69-4100-8abe-84cf0a04ccb9");
    }

    #[test]
    fn session_id_serializes_as_string() {
        let id: SessionId = serde_json::from_str("886030").unwrap();

        assert_eq!(serde_json::to_string(&id).unwrap(), r#""886030""#);
    }

    #[test]
    fn session_id_rejects_other_types() {
        let err = serde_json::from_str::<SessionId>("true").unwrap_err();

        assert!(
            err.to_string()
                .contains("a session id as a string or an integer"),
            "{err}"
        );
    }

    #[test]
    fn speaker_id_round_trips() {
        let json = r#""08521b30-d304-498f-8bbf-627d5f9ffba4""#;
        let id: SpeakerId = serde_json::from_str(json).unwrap();

        assert_eq!(id.to_string(), "08521b30-d304-498f-8bbf-627d5f9ffba4");
        assert_eq!(serde_json::to_string(&id).unwrap(), json);
    }

    #[test]
    fn string_ids_can_be_looked_up_by_str() {
        let map = HashMap::from([(SpeakerId::new("a"), 1), (SpeakerId::new("b"), 2)]);

        assert_eq!(map.get("b"), Some(&2));
    }

    #[test]
    fn group_ids_keep_their_form() {
        let ids: Vec<GroupId> = serde_json::from_str(r#"[10663, "10663", "007", "abc"]"#).unwrap();

        assert_eq!(ids[0], ids[1]);
        assert_eq!(
            serde_json::to_string(&ids).unwrap(),
            r#"[10663,10663,"007","abc"]"#
        );
    }

    #[test]
    fn numeric_id_round_trips() {
        let id: RoomId = serde_json::from_str("58209").unwrap();

        assert_eq!(id, RoomId::new(58209));
        assert_eq!(id.get(), 58209);
        assert_eq!(id.to_string(), "58209");
        assert_eq!(serde_json::to_string(&id).unwrap(), "58209");
    }

    #[test]
    fn numeric_id_parses_from_str() {
        assert_eq!("143673".parse(), Ok(QuestionId::new(143_673)));
        assert!("abc".parse::<QuestionId>().is_err());
    }
}
