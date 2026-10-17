//! Interpreting answers to questions.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::QuestionType;

/// An answer interpreted according to its [`QuestionType`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnswerValue {
    /// Short or long text. Line breaks are kept as sent (`\r\n`).
    Text(String),
    /// Web address as typed by the user. It may not be a valid URL.
    WebAddress(String),
    /// Email address as typed by the user.
    Email(String),
    /// Whole number.
    Integer(i64),
    /// Percentage from 0 to 100.
    Percentage(u8),
    /// Checkbox state.
    Checkbox(bool),
    /// Uploaded file.
    File(FileAnswer),
    /// Answer to a question of a type this version of the crate doesn't know.
    Unknown {
        /// The question type as sent by the API.
        question_type: String,
        /// The answer as sent by the API.
        value: String,
    },
}

impl AnswerValue {
    /// Interprets `value` according to `question_type`.
    ///
    /// `file_name` is the `answerExtra` field of the API: the original name of
    /// an uploaded file.
    ///
    /// # Errors
    ///
    /// Returns an error if the value doesn't match the question type, such as
    /// a non-numeric integer answer.
    ///
    /// ```
    /// # use sessionize::{AnswerValue, QuestionType};
    /// let value = AnswerValue::parse(&QuestionType::Integer, "12345", None).unwrap();
    /// assert_eq!(value, AnswerValue::Integer(12345));
    /// ```
    pub fn parse(
        question_type: &QuestionType,
        value: &str,
        file_name: Option<&str>,
    ) -> Result<Self, ParseAnswerError> {
        let parsed = match question_type {
            QuestionType::ShortText | QuestionType::LongText => Some(Self::Text(value.to_owned())),
            QuestionType::WebAddress => Some(Self::WebAddress(value.to_owned())),
            QuestionType::EmailAddress => Some(Self::Email(value.to_owned())),
            QuestionType::Integer => value.parse().ok().map(Self::Integer),
            QuestionType::Percentage => value
                .parse()
                .ok()
                .filter(|percentage| *percentage <= 100)
                .map(Self::Percentage),
            QuestionType::Checkbox => parse_bool(value).map(Self::Checkbox),
            QuestionType::FileUpload => Url::parse(value).ok().map(|url| {
                Self::File(FileAnswer {
                    url,
                    file_name: file_name.map(str::to_owned),
                })
            }),
            QuestionType::Unknown(question_type) => Some(Self::Unknown {
                question_type: question_type.clone(),
                value: value.to_owned(),
            }),
        };

        parsed.ok_or_else(|| ParseAnswerError {
            question_type: question_type.clone(),
            value: value.to_owned(),
        })
    }
}

pub(crate) fn parse_bool(value: &str) -> Option<bool> {
    if value.eq_ignore_ascii_case("true") {
        Some(true)
    } else if value.eq_ignore_ascii_case("false") {
        Some(false)
    } else {
        None
    }
}

/// An uploaded file.
///
/// Can also be used as the type of a file upload field when deserializing
/// [custom fields](crate#custom-fields).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FileAnswer {
    /// Download URL of the file.
    pub url: Url,
    /// Original name of the file.
    ///
    /// Only available from the Sessions and Speakers views; the `All` view
    /// doesn't include it.
    pub file_name: Option<String>,
}

/// Error returned when an answer doesn't match its question type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid answer {value:?} for a {question_type} question")]
pub struct ParseAnswerError {
    question_type: QuestionType,
    value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(question_type: &str, value: &str) -> Result<AnswerValue, ParseAnswerError> {
        AnswerValue::parse(&QuestionType::from(question_type), value, None)
    }

    #[test]
    fn parses_every_question_type() {
        assert_eq!(
            parse("Long_Text", "a\r\nb"),
            Ok(AnswerValue::Text("a\r\nb".to_owned()))
        );
        assert_eq!(
            parse("Web_address", "example.com"),
            Ok(AnswerValue::WebAddress("example.com".to_owned()))
        );
        assert_eq!(
            parse("Email_address", "mark@mark.com"),
            Ok(AnswerValue::Email("mark@mark.com".to_owned()))
        );
        assert_eq!(parse("Integer", "-12"), Ok(AnswerValue::Integer(-12)));
        assert_eq!(parse("Percentage", "67"), Ok(AnswerValue::Percentage(67)));
        assert_eq!(parse("Checkbox", "false"), Ok(AnswerValue::Checkbox(false)));
        assert_eq!(
            parse("Rating", "5"),
            Ok(AnswerValue::Unknown {
                question_type: "Rating".to_owned(),
                value: "5".to_owned()
            })
        );
    }

    #[test]
    fn parses_file_with_name() {
        let value = AnswerValue::parse(
            &QuestionType::FileUpload,
            "https://sessionize.com/download/x~logo.svg",
            Some("Logo.svg"),
        )
        .unwrap();

        let AnswerValue::File(file) = value else {
            panic!("{value:?}")
        };
        assert_eq!(
            file.url.as_str(),
            "https://sessionize.com/download/x~logo.svg"
        );
        assert_eq!(file.file_name.as_deref(), Some("Logo.svg"));
    }

    #[test]
    fn rejects_mismatching_answers() {
        for (question_type, value) in [
            ("Integer", "12.5"),
            ("Percentage", "101"),
            ("Checkbox", "yes"),
            ("File_upload", "logo.svg"),
        ] {
            let err = parse(question_type, value).unwrap_err();

            assert_eq!(
                err.to_string(),
                format!("invalid answer {value:?} for a {question_type} question")
            );
        }
    }
}
