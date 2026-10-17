//! Types of the `GridSmart` view: the schedule as a grid of rooms and time slots.

use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

use crate::{RoomId, Session};

/// A day of the schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Day {
    /// Date of the day.
    ///
    /// Serializes as a midnight datetime without a time zone suffix, even if
    /// the API sent one.
    #[serde(with = "date")]
    pub date: NaiveDate,
    /// Whether this is the default day to show.
    pub is_default: bool,
    /// Rooms with their sessions on this day.
    #[serde(default)]
    pub rooms: Vec<Room>,
    /// Time slots of the day, each listing the sessions starting at that time.
    #[serde(default)]
    pub time_slots: Vec<TimeSlot>,
}

/// A room with its sessions on a day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Room {
    /// Identifier of the room.
    pub id: RoomId,
    /// Name of the room.
    pub name: String,
    /// Sessions in the room on this day.
    #[serde(default)]
    pub sessions: Vec<Session>,
    /// Whether all sessions in the room on this day are plenum sessions.
    pub has_only_plenum_sessions: bool,
}

/// Sessions starting at the same time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TimeSlot {
    /// Start time of the slot.
    #[serde(rename = "slotStart")]
    pub start: NaiveTime,
    /// Rooms with a session starting in this slot.
    #[serde(default)]
    pub rooms: Vec<TimeSlotRoom>,
}

/// A room with the session starting in a [`TimeSlot`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TimeSlotRoom {
    /// Identifier of the room.
    pub id: RoomId,
    /// Name of the room.
    pub name: String,
    /// The session starting in this slot.
    pub session: Session,
    /// Position of the room among the day's rooms.
    pub index: u32,
}

/// The API sends dates as midnight datetimes (`2025-04-24T00:00:00`, with a
/// `Z` suffix when the endpoint uses UTC).
mod date {
    use chrono::NaiveDate;
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::ScheduleTime;

    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "signature required by serde"
    )]
    pub(super) fn serialize<S: Serializer>(
        date: &NaiveDate,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&date.format("%Y-%m-%dT00:00:00"))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<NaiveDate, D::Error> {
        ScheduleTime::deserialize(deserializer).map(ScheduleTime::date)
    }
}
