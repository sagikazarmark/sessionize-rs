//! Deserializing custom fields into user-defined types.
//!
//! Custom fields (question answers and categories) are presented to serde as
//! a map keyed by question text and category title. When deserializing a
//! struct, fields can also be matched by ID (`#[serde(rename = "143673")]`).
//! When deserializing a map, fields sharing a name are keyed by ID instead.
//!
//! Answers are strings in the API; they're parsed into whatever type the
//! target field has (numbers, booleans, URLs, enums). Unanswered questions are
//! left out of the map, so `Option` fields become `None`. Categories are
//! sequences of the selected item names: they deserialize into a `Vec`, or
//! into a single value if exactly one item is selected.
//!
//! Types that serde buffers (`#[serde(flatten)]`, untagged and internally
//! tagged enums) see the values without conversion: integers, percentages
//! and checkboxes as numbers and booleans, other answers as strings, and
//! categories as sequences.

use std::collections::{HashMap, HashSet};
use std::fmt;

use serde::de::value::{MapDeserializer, SeqDeserializer, StrDeserializer};
use serde::de::{self, DeserializeOwned, DeserializeSeed, IntoDeserializer, Unexpected, Visitor};
use serde::forward_to_deserialize_any;

use crate::answer::parse_bool;
use crate::{QuestionAnswer, QuestionType, SelectedCategory};

/// Error returned when custom fields can't be deserialized into the requested
/// type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomFieldsError {
    field: Option<String>,
    message: String,
}

impl CustomFieldsError {
    /// Returns the name (or ID) of the custom field that failed, if known.
    #[must_use]
    pub fn field(&self) -> Option<&str> {
        self.field.as_deref()
    }

    fn in_field(mut self, field: &str) -> Self {
        if self.field.is_none() {
            self.field = Some(field.to_owned());
        }
        self
    }
}

impl fmt::Display for CustomFieldsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.field {
            Some(field) => write!(f, "custom field {field:?}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for CustomFieldsError {}

impl de::Error for CustomFieldsError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self {
            field: None,
            message: msg.to_string(),
        }
    }

    fn missing_field(field: &'static str) -> Self {
        Self {
            field: Some(field.to_owned()),
            message: "missing (unanswered, or no custom field has this name)".to_owned(),
        }
    }
}

/// The custom fields of a session or speaker.
#[derive(Debug, Default)]
pub(crate) struct Fields<'a>(Vec<Field<'a>>);

#[derive(Debug)]
struct Field<'a> {
    name: &'a str,
    id: u64,
    value: Value<'a>,
}

#[derive(Debug, Clone)]
enum Value<'a> {
    Answer(Text<'a>),
    Category(Vec<&'a str>),
}

impl<'a> Fields<'a> {
    /// Collects fields from the Sessions and Speakers views.
    pub(crate) fn from_view(
        answers: &'a [QuestionAnswer],
        categories: &'a [SelectedCategory],
    ) -> Self {
        let mut fields = Self::default();
        for answer in answers {
            if let Some(value) = &answer.answer {
                fields.answer(
                    &answer.question,
                    answer.id.get(),
                    &answer.question_type,
                    value,
                    answer.answer_extra.as_deref(),
                );
            }
        }
        for category in categories {
            fields.category(
                &category.name,
                category.id.get(),
                category
                    .items
                    .iter()
                    .map(|item| item.name.as_str())
                    .collect(),
            );
        }
        fields
    }

    pub(crate) fn answer(
        &mut self,
        name: &'a str,
        id: u64,
        question_type: &QuestionType,
        value: &'a str,
        file_name: Option<&'a str>,
    ) {
        let kind = match question_type {
            QuestionType::Integer => Kind::Integer,
            QuestionType::Percentage => Kind::Percentage,
            QuestionType::Checkbox => Kind::Checkbox,
            QuestionType::FileUpload => Kind::File(file_name),
            _ => Kind::Text,
        };
        self.0.push(Field {
            name,
            id,
            value: Value::Answer(Text { text: value, kind }),
        });
    }

    pub(crate) fn category(&mut self, name: &'a str, id: u64, items: Vec<&'a str>) {
        self.0.push(Field {
            name,
            id,
            value: Value::Category(items),
        });
    }

    pub(crate) fn deserialize<T: DeserializeOwned>(self) -> Result<T, CustomFieldsError> {
        T::deserialize(FieldsDeserializer(self.0))
    }
}

struct FieldsDeserializer<'a>(Vec<Field<'a>>);

impl<'de> de::Deserializer<'de> for FieldsDeserializer<'_> {
    type Error = CustomFieldsError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        // Key fields by name, or by ID if several share the name.
        let mut names: HashMap<&str, usize> = HashMap::new();
        for field in &self.0 {
            *names.entry(field.name).or_default() += 1;
        }
        let entries = self
            .0
            .iter()
            .map(|field| {
                let key = if names[field.name] > 1 {
                    field.id.to_string()
                } else {
                    field.name.to_owned()
                };
                (key, field.value.clone())
            })
            .collect();

        visit_fields(entries, visitor)
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        // Offer each field under its name and its ID, if the struct asks for
        // them. Fields the struct doesn't ask for are left out, so
        // `deny_unknown_fields` doesn't reject them.
        let mut entries = Vec::new();
        for field in self.0 {
            if fields.contains(&field.name) {
                entries.push((field.name.to_owned(), field.value.clone()));
            }
            let id = field.id.to_string();
            if fields.contains(&id.as_str()) {
                entries.push((id, field.value));
            }
        }

        visit_fields(entries, visitor)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_unit()
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map enum
        identifier
    }
}

fn visit_fields<'de, V: Visitor<'de>>(
    entries: Vec<(String, Value<'_>)>,
    visitor: V,
) -> Result<V::Value, CustomFieldsError> {
    let mut seen = HashSet::new();
    for (key, _) in &entries {
        if !seen.insert(key.as_str()) {
            return Err(CustomFieldsError {
                field: Some(key.clone()),
                message: "more than one custom field has this name, match it by ID instead"
                    .to_owned(),
            });
        }
    }

    let mut access = FieldsAccess {
        entries: entries.into_iter(),
        pending: None,
    };
    let value = visitor.visit_map(&mut access)?;

    Ok(value)
}

struct FieldsAccess<'a> {
    entries: std::vec::IntoIter<(String, Value<'a>)>,
    pending: Option<(String, Value<'a>)>,
}

impl<'de> de::MapAccess<'de> for &mut FieldsAccess<'_> {
    type Error = CustomFieldsError;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        let Some((key, value)) = self.entries.next() else {
            return Ok(None);
        };
        let deserializer: StrDeserializer<'_, CustomFieldsError> = key.as_str().into_deserializer();
        let key_value = seed.deserialize(deserializer)?;
        self.pending = Some((key, value));

        Ok(Some(key_value))
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        let (key, value) = self
            .pending
            .take()
            .ok_or_else(|| de::Error::custom("value requested before key"))?;

        match value {
            Value::Answer(text) => seed.deserialize(text),
            Value::Category(items) => seed.deserialize(Category(items)),
        }
        .map_err(|err| err.in_field(&key))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.entries.len())
    }
}

#[derive(Debug, Clone, Copy)]
enum Kind<'a> {
    Text,
    Integer,
    Percentage,
    Checkbox,
    File(Option<&'a str>),
}

/// A text value: an answer, or the name of a selected category item.
#[derive(Debug, Clone, Copy)]
struct Text<'a> {
    text: &'a str,
    kind: Kind<'a>,
}

impl<'a> Text<'a> {
    fn plain(text: &'a str) -> Self {
        Self {
            text,
            kind: Kind::Text,
        }
    }

    fn invalid<'de, V: Visitor<'de>>(self, visitor: &V) -> CustomFieldsError {
        de::Error::invalid_value(Unexpected::Str(self.text), visitor)
    }

    /// Returns the text unless it's out of range for its question type.
    fn number(self) -> Option<&'a str> {
        match self.kind {
            Kind::Percentage => self
                .text
                .parse::<u8>()
                .is_ok_and(|percentage| percentage <= 100)
                .then_some(self.text),
            _ => Some(self.text),
        }
    }
}

impl IntoDeserializer<'_, CustomFieldsError> for Text<'_> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

macro_rules! parse_text {
    ($($method:ident => $ty:ty, $visit:ident;)*) => {
        $(
            fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
                match self.number().map(str::parse::<$ty>) {
                    Some(Ok(value)) => visitor.$visit(value),
                    _ => Err(self.invalid(&visitor)),
                }
            }
        )*
    };
}

impl<'de> de::Deserializer<'de> for Text<'_> {
    type Error = CustomFieldsError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.kind {
            Kind::Integer => match self.text.parse() {
                Ok(value) => visitor.visit_i64(value),
                Err(_) => visitor.visit_str(self.text),
            },
            Kind::Percentage => match self.number().map(str::parse) {
                Some(Ok(value)) => visitor.visit_u64(value),
                _ => visitor.visit_str(self.text),
            },
            Kind::Checkbox => match parse_bool(self.text) {
                Some(value) => visitor.visit_bool(value),
                None => visitor.visit_str(self.text),
            },
            Kind::Text | Kind::File(_) => visitor.visit_str(self.text),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match parse_bool(self.text) {
            Some(value) => visitor.visit_bool(value),
            None => Err(self.invalid(&visitor)),
        }
    }

    parse_text! {
        deserialize_i8 => i8, visit_i8;
        deserialize_i16 => i16, visit_i16;
        deserialize_i32 => i32, visit_i32;
        deserialize_i64 => i64, visit_i64;
        deserialize_i128 => i128, visit_i128;
        deserialize_u8 => u8, visit_u8;
        deserialize_u16 => u16, visit_u16;
        deserialize_u32 => u32, visit_u32;
        deserialize_u64 => u64, visit_u64;
        deserialize_u128 => u128, visit_u128;
        deserialize_f32 => f32, visit_f32;
        deserialize_f64 => f64, visit_f64;
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_str(self.text)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_str(self.text)
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_str(self.text)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_bytes(self.text.as_bytes())
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_bytes(self.text.as_bytes())
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let mut seq = SeqDeserializer::new(std::iter::once(self));
        let value = visitor.visit_seq(&mut seq)?;
        seq.end()?;

        Ok(value)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        // A file upload deserializes into a struct such as `FileAnswer`.
        let Kind::File(file_name) = self.kind else {
            return self.deserialize_any(visitor);
        };
        let entries = std::iter::once(("url", Text::plain(self.text)))
            .chain(file_name.map(|file_name| ("file_name", Text::plain(file_name))));
        let mut map = MapDeserializer::new(entries);
        let value = visitor.visit_map(&mut map)?;
        map.end()?;

        Ok(value)
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        let deserializer: StrDeserializer<'_, CustomFieldsError> = self.text.into_deserializer();
        visitor.visit_enum(deserializer)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_unit()
    }

    forward_to_deserialize_any! {
        char unit unit_struct tuple tuple_struct
    }
}

/// The names of the selected items of a category.
struct Category<'a>(Vec<&'a str>);

impl<'a> Category<'a> {
    fn single(&self) -> Result<Text<'a>, CustomFieldsError> {
        match self.0.as_slice() {
            [item] => Ok(Text::plain(item)),
            [] => Err(de::Error::custom("no item is selected")),
            items => Err(de::Error::custom(format_args!(
                "{} items are selected, expected one",
                items.len()
            ))),
        }
    }
}

macro_rules! single_item {
    ($($method:ident)*) => {
        $(
            fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
                self.single()?.$method(visitor)
            }
        )*
    };
}

impl<'de> de::Deserializer<'de> for Category<'_> {
    type Error = CustomFieldsError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let mut seq = SeqDeserializer::new(self.0.into_iter().map(Text::plain));
        let value = visitor.visit_seq(&mut seq)?;
        seq.end()?;

        Ok(value)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        if self.0.is_empty() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.single()?.deserialize_enum(name, variants, visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_unit()
    }

    single_item! {
        deserialize_bool deserialize_i8 deserialize_i16 deserialize_i32 deserialize_i64
        deserialize_i128 deserialize_u8 deserialize_u16 deserialize_u32 deserialize_u64
        deserialize_u128 deserialize_f32 deserialize_f64 deserialize_char deserialize_str
        deserialize_string deserialize_bytes deserialize_byte_buf deserialize_identifier
    }

    forward_to_deserialize_any! {
        unit unit_struct seq tuple tuple_struct map struct
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde::Deserialize;

    use super::*;

    fn fields() -> Fields<'static> {
        let mut fields = Fields::default();
        fields.answer("Name", 1, &QuestionType::ShortText, "Mark", None);
        fields.answer("Count", 2, &QuestionType::Integer, "12345", None);
        fields.answer("Share", 3, &QuestionType::Percentage, "67", None);
        fields.answer("Agree", 4, &QuestionType::Checkbox, "true", None);
        fields.answer(
            "Slides",
            5,
            &QuestionType::FileUpload,
            "https://sessionize.com/download/x~slides.pdf",
            Some("Slides.pdf"),
        );
        fields.category("Level", 6, vec!["Advanced"]);
        fields.category("Tags", 7, vec!["Rust", "Go"]);
        fields.category("Track", 8, vec![]);
        fields
    }

    #[derive(Debug, Deserialize, PartialEq)]
    enum Level {
        Beginner,
        Advanced,
    }

    #[test]
    fn deserializes_typed_fields() {
        #[derive(Debug, Deserialize)]
        struct Typed {
            #[serde(rename = "Name")]
            name: String,
            #[serde(rename = "Count")]
            count: u32,
            #[serde(rename = "Share")]
            share: f64,
            #[serde(rename = "Agree")]
            agree: bool,
            #[serde(rename = "Slides")]
            slides: crate::FileAnswer,
            #[serde(rename = "Level")]
            level: Level,
            #[serde(rename = "Tags")]
            tags: Vec<String>,
            #[serde(rename = "Track")]
            track: Option<String>,
            #[serde(rename = "Missing")]
            missing: Option<String>,
        }

        let typed: Typed = fields().deserialize().unwrap();

        assert_eq!(typed.name, "Mark");
        assert_eq!(typed.count, 12345);
        assert!((typed.share - 67.0).abs() < f64::EPSILON);
        assert!(typed.agree);
        assert_eq!(
            typed.slides.url.as_str(),
            "https://sessionize.com/download/x~slides.pdf"
        );
        assert_eq!(typed.slides.file_name.as_deref(), Some("Slides.pdf"));
        assert_eq!(typed.level, Level::Advanced);
        assert_eq!(typed.tags, ["Rust", "Go"]);
        assert_eq!(typed.track, None);
        assert_eq!(typed.missing, None);
    }

    #[test]
    fn matches_fields_by_id() {
        #[derive(Deserialize)]
        struct ById {
            #[serde(rename = "2")]
            count: i64,
        }

        let typed: ById = fields().deserialize().unwrap();

        assert_eq!(typed.count, 12345);
    }

    #[test]
    fn deserializes_into_map() {
        let map: HashMap<String, serde_json::Value> = fields().deserialize().unwrap();

        assert_eq!(map["Name"], "Mark");
        assert_eq!(map["Count"], 12345);
        assert_eq!(map["Agree"], true);
        assert_eq!(map["Tags"], serde_json::json!(["Rust", "Go"]));
    }

    #[test]
    fn deny_unknown_fields_ignores_fields_not_asked_for() {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Strict {
            #[serde(rename = "Name")]
            name: String,
        }

        let typed: Strict = fields().deserialize().unwrap();

        assert_eq!(typed.name, "Mark");
    }

    fn error<T: DeserializeOwned + fmt::Debug>() -> CustomFieldsError {
        fields().deserialize::<T>().unwrap_err()
    }

    #[test]
    fn reports_the_failing_field() {
        #[derive(Debug, Deserialize)]
        struct WrongType {
            #[serde(rename = "Name")]
            _name: u8,
        }

        let err = error::<WrongType>();

        assert_eq!(err.field(), Some("Name"));
        assert_eq!(
            err.to_string(),
            r#"custom field "Name": invalid value: string "Mark", expected u8"#
        );
    }

    #[test]
    fn rejects_multiple_items_for_a_single_value() {
        #[derive(Debug, Deserialize)]
        struct Single {
            #[serde(rename = "Tags")]
            _tags: String,
        }

        assert_eq!(
            error::<Single>().to_string(),
            r#"custom field "Tags": 2 items are selected, expected one"#
        );
    }

    #[test]
    fn rejects_empty_selection_for_a_required_value() {
        #[derive(Debug, Deserialize)]
        struct Required {
            #[serde(rename = "Track")]
            _track: String,
        }

        assert_eq!(
            error::<Required>().to_string(),
            r#"custom field "Track": no item is selected"#
        );
    }

    #[test]
    fn reports_missing_fields() {
        #[derive(Debug, Deserialize)]
        struct Required {
            #[serde(rename = "Missing")]
            _missing: String,
        }

        let err = error::<Required>();

        assert_eq!(err.field(), Some("Missing"));
        assert_eq!(
            err.to_string(),
            r#"custom field "Missing": missing (unanswered, or no custom field has this name)"#
        );
    }

    #[test]
    fn matches_fields_by_name_and_id_in_the_same_struct() {
        #[derive(Deserialize)]
        struct Both {
            #[serde(rename = "Level")]
            by_name: String,
            #[serde(rename = "6")]
            by_id: String,
        }

        let typed: Both = fields().deserialize().unwrap();

        assert_eq!(typed.by_name, "Advanced");
        assert_eq!(typed.by_id, "Advanced");
    }

    #[test]
    fn rejects_out_of_range_percentages() {
        #[derive(Debug, Deserialize)]
        struct Share {
            #[serde(rename = "Share")]
            _share: u8,
        }

        let mut fields = Fields::default();
        fields.answer("Share", 1, &QuestionType::Percentage, "150", None);
        let err = fields.deserialize::<Share>().unwrap_err();

        assert_eq!(err.field(), Some("Share"));
    }

    fn ambiguous() -> Fields<'static> {
        let mut fields = fields();
        fields.answer("Level", 9, &QuestionType::ShortText, "Expert", None);
        fields
    }

    #[test]
    fn rejects_ambiguous_names() {
        #[derive(Debug, Deserialize)]
        struct Ambiguous {
            #[serde(rename = "Level")]
            _level: String,
        }

        let err = ambiguous().deserialize::<Ambiguous>().unwrap_err();

        assert_eq!(err.field(), Some("Level"));
    }

    #[test]
    fn ambiguous_names_are_keyed_by_id_in_maps() {
        let map: HashMap<String, serde_json::Value> = ambiguous().deserialize().unwrap();

        assert!(!map.contains_key("Level"));
        assert_eq!(map["6"], serde_json::json!(["Advanced"]));
        assert_eq!(map["9"], "Expert");
    }

    #[test]
    fn ignored_any_accepts_anything() {
        ambiguous().deserialize::<serde::de::IgnoredAny>().unwrap();
        ambiguous()
            .deserialize::<Option<serde::de::IgnoredAny>>()
            .unwrap();
    }
}
