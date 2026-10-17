#![cfg_attr(feature = "client", doc = include_str!("../README.md"))]
#![cfg_attr(
    not(feature = "client"),
    doc = "Typed data model of the [Sessionize API](https://sessionize.com/playbook/api).\n\nEnable the `rustls` or `native-tls` feature for the HTTP client and the full documentation."
)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod all;
mod answer;
#[cfg(feature = "client")]
mod client;
mod common;
mod custom_fields;
mod endpoint;
#[cfg(feature = "client")]
mod error;
pub mod event;
pub mod grid;
mod id;
mod session;
mod speaker;
mod time;

pub use all::All;
pub use answer::{AnswerValue, FileAnswer, ParseAnswerError};
#[cfg(feature = "client")]
pub use client::{Client, ClientBuilder, Endpoint, EventOptions};
pub use common::{
    Category, CategoryItem, CategoryType, Link, LinkType, Question, QuestionType, Room,
    SessionStatus,
};
pub use custom_fields::CustomFieldsError;
pub use endpoint::{DEFAULT_BASE_URL, EndpointId, InvalidEndpointId, View};
#[cfg(feature = "client")]
pub use error::{BuildError, Error};
pub use event::{Event, TypedEvent, TypedEventError};
pub use id::{CategoryId, CategoryItemId, GroupId, QuestionId, RoomId, SessionId, SpeakerId};
pub use session::{
    CategoryItemRef, QuestionAnswer, SelectedCategory, Session, SessionGroup, SpeakerRef,
};
pub use speaker::{SessionRef, Speaker, WallSpeaker};
pub use time::{ParseScheduleTimeError, ScheduleTime};
