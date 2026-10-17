//! Types shared by several views.

use std::convert::Infallible;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::{CategoryId, CategoryItemId, QuestionId, RoomId};

/// Defines an enum for a string value of the API, with an `Unknown` fallback
/// so new values don't break deserialization.
macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $value:literal, )*
        }
    ) => {
        $(#[$meta])*
        ///
        /// Values compare and hash by their API value, so an `Unknown` value
        /// equals the known variant with the same value.
        #[derive(Debug, Clone)]
        #[non_exhaustive]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )*
            /// A value this version of the crate doesn't know, as sent by the API.
            Unknown(String),
        }

        impl $name {
            /// Returns the value as sent by the API.
            #[must_use]
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $value, )*
                    Self::Unknown(value) => value,
                }
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.as_str() == other.as_str()
            }
        }

        impl Eq for $name {}

        impl std::hash::Hash for $name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.as_str().hash(state);
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                match value {
                    $( $value => Self::$variant, )*
                    _ => Self::Unknown(value.to_owned()),
                }
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                match Self::from(value.as_str()) {
                    Self::Unknown(_) => Self::Unknown(value),
                    known => known,
                }
            }
        }

        impl FromStr for $name {
            type Err = Infallible;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self::from(s))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct Visitor;

                impl de::Visitor<'_> for Visitor {
                    type Value = $name;

                    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("a string")
                    }

                    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                        Ok($name::from(v))
                    }

                    fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                        Ok($name::from(v))
                    }
                }

                deserializer.deserialize_str(Visitor)
            }
        }
    };
}

string_enum! {
    /// Status of a session.
    ///
    /// Service sessions have no status.
    pub enum SessionStatus {
        /// The session was accepted.
        Accepted = "Accepted",
    }
}

string_enum! {
    /// Type of a question (a custom field answered with free-form input).
    pub enum QuestionType {
        /// Single line of text.
        ShortText = "Short_Text",
        /// Multiple lines of text.
        LongText = "Long_Text",
        /// File upload. The answer is a download URL.
        FileUpload = "File_upload",
        /// Web address, as typed by the user.
        WebAddress = "Web_address",
        /// Whole number.
        Integer = "Integer",
        /// Whole number from 0 to 100.
        Percentage = "Percentage",
        /// Email address.
        EmailAddress = "Email_address",
        /// Checkbox. The answer is `"true"` or `"false"`.
        Checkbox = "Checkbox",
    }
}

string_enum! {
    /// Type of a speaker link.
    ///
    /// Facebook, Instagram and Sessionize links exist but their values haven't
    /// been confirmed yet; they deserialize as [`LinkType::Unknown`].
    pub enum LinkType {
        /// X (Twitter) profile.
        Twitter = "Twitter",
        /// `LinkedIn` profile.
        LinkedIn = "LinkedIn",
        /// Blog.
        Blog = "Blog",
        /// Company website.
        CompanyWebsite = "Company_Website",
        /// Any other link.
        Other = "Other",
    }
}

string_enum! {
    /// Whether a category applies to sessions or speakers.
    pub enum CategoryType {
        /// Session category.
        Session = "session",
        /// Speaker category.
        Speaker = "speaker",
    }
}

/// A link on a speaker's profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Link {
    /// Display title.
    pub title: String,
    /// URL as entered by the speaker. It may not be a valid URL.
    pub url: String,
    /// Type of the link.
    pub link_type: LinkType,
}

/// A room of the event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Room {
    /// Identifier of the room.
    pub id: RoomId,
    /// Name of the room.
    pub name: String,
    /// Position of the room in the organizer's order.
    pub sort: i32,
}

/// A question: a custom field answered with free-form input.
///
/// Questions belong to either sessions or speakers, but the API doesn't say
/// which.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Question {
    /// Identifier of the question.
    pub id: QuestionId,
    /// Text of the question.
    pub question: String,
    /// Type of the expected answer.
    pub question_type: QuestionType,
    /// Position among custom fields (questions and categories) in the
    /// organizer's order.
    pub sort: i32,
}

/// A category: a custom field answered by selecting one or more items.
///
/// The API doesn't say whether one or more items can be selected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Category {
    /// Identifier of the category.
    pub id: CategoryId,
    /// Title of the category, such as "Level".
    pub title: String,
    /// Items that can be selected.
    #[serde(default)]
    pub items: Vec<CategoryItem>,
    /// Position among custom fields (questions and categories) in the
    /// organizer's order.
    pub sort: i32,
    /// Whether the category applies to sessions or speakers.
    #[serde(rename = "type")]
    pub category_type: CategoryType,
}

/// An item of a [`Category`], such as "Advanced".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CategoryItem {
    /// Identifier of the item.
    pub id: CategoryItemId,
    /// Name of the item.
    pub name: String,
    /// Position of the item within its category.
    pub sort: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values_round_trip() {
        for value in [
            "Short_Text",
            "Long_Text",
            "File_upload",
            "Web_address",
            "Integer",
            "Percentage",
            "Email_address",
            "Checkbox",
        ] {
            let parsed = QuestionType::from(value);

            assert!(!matches!(parsed, QuestionType::Unknown(_)), "{value}");
            assert_eq!(parsed.as_str(), value);
        }
    }

    #[test]
    fn unknown_values_are_kept() {
        let parsed: LinkType = serde_json::from_str(r#""Mastodon""#).unwrap();

        assert_eq!(parsed, LinkType::Unknown("Mastodon".to_owned()));
        assert_eq!(serde_json::to_string(&parsed).unwrap(), r#""Mastodon""#);
    }

    #[test]
    fn other_is_a_known_link_type() {
        assert_eq!(LinkType::from("Other"), LinkType::Other);
    }

    #[test]
    fn unknown_values_equal_known_variants() {
        assert_eq!(
            SessionStatus::Unknown("Accepted".to_owned()),
            SessionStatus::Accepted
        );
        assert_ne!(
            SessionStatus::Unknown("Declined".to_owned()),
            SessionStatus::Accepted
        );
    }

    #[test]
    fn from_string_normalizes_known_values() {
        assert_eq!(
            SessionStatus::from("Accepted".to_owned()),
            SessionStatus::Accepted
        );
    }
}
