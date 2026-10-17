//! Schedule times.

use std::fmt;
use std::str::FromStr;

use chrono::{
    DateTime, MappedLocalTime, NaiveDate, NaiveDateTime, NaiveTime, SecondsFormat, TimeZone, Utc,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// A point in time in an event's schedule, such as the start of a session.
///
/// Sessionize sends schedule times as local wall-clock time
/// (`2025-04-24T09:00:00`), or in UTC (`2023-09-16T09:00:00Z`) when the
/// endpoint has "Use UTC timezone for schedule" enabled.
///
/// The API doesn't expose the event's time zone, so a local time can only be
/// turned into an instant by supplying the time zone yourself with
/// [`ScheduleTime::with_timezone`].
///
/// Values serialize back to the same string format they were parsed from.
///
/// ```
/// # use sessionize::ScheduleTime;
/// let local: ScheduleTime = "2025-04-24T09:00:00".parse().unwrap();
/// assert!(!local.is_utc());
/// assert_eq!(local.to_utc(), None);
///
/// let utc: ScheduleTime = "2023-09-16T09:00:00Z".parse().unwrap();
/// assert!(utc.is_utc());
/// assert_eq!(utc.to_string(), "2023-09-16T09:00:00Z");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScheduleTime {
    /// Wall-clock time in the event's time zone, which the API doesn't provide.
    Local(NaiveDateTime),
    /// An instant in UTC.
    Utc(DateTime<Utc>),
}

impl ScheduleTime {
    /// Returns the date and time without time zone information.
    ///
    /// For [`ScheduleTime::Local`] this is the wall-clock time at the event,
    /// for [`ScheduleTime::Utc`] the time in UTC.
    #[must_use]
    pub fn naive(self) -> NaiveDateTime {
        match self {
            Self::Local(time) => time,
            Self::Utc(time) => time.naive_utc(),
        }
    }

    /// Returns the instant if the time is in UTC, or `None` for local times.
    #[must_use]
    pub fn to_utc(self) -> Option<DateTime<Utc>> {
        match self {
            Self::Local(_) => None,
            Self::Utc(time) => Some(time),
        }
    }

    /// Returns `true` if the time is in UTC.
    #[must_use]
    pub fn is_utc(self) -> bool {
        matches!(self, Self::Utc(_))
    }

    /// Returns the date part of [`ScheduleTime::naive`].
    #[must_use]
    pub fn date(self) -> NaiveDate {
        self.naive().date()
    }

    /// Returns the time part of [`ScheduleTime::naive`].
    #[must_use]
    pub fn time(self) -> NaiveTime {
        self.naive().time()
    }

    /// Returns the time in the given time zone.
    ///
    /// A local time is interpreted as wall-clock time in `tz`, so pass the
    /// event's time zone. The result is ambiguous or missing for local times
    /// that fall into a daylight saving transition.
    ///
    /// A UTC time is converted to `tz`, which always has a single result.
    ///
    /// ```
    /// # use sessionize::ScheduleTime;
    /// use chrono::FixedOffset;
    ///
    /// let budapest = FixedOffset::east_opt(2 * 3600).unwrap();
    /// let start: ScheduleTime = "2025-04-24T09:00:00".parse().unwrap();
    ///
    /// let start = start.with_timezone(&budapest).single().unwrap();
    /// assert_eq!(start.to_rfc3339(), "2025-04-24T09:00:00+02:00");
    /// ```
    pub fn with_timezone<Tz: TimeZone>(self, tz: &Tz) -> MappedLocalTime<DateTime<Tz>> {
        match self {
            Self::Local(time) => tz.from_local_datetime(&time),
            Self::Utc(time) => MappedLocalTime::Single(time.with_timezone(tz)),
        }
    }
}

impl From<NaiveDateTime> for ScheduleTime {
    fn from(time: NaiveDateTime) -> Self {
        Self::Local(time)
    }
}

impl From<DateTime<Utc>> for ScheduleTime {
    fn from(time: DateTime<Utc>) -> Self {
        Self::Utc(time)
    }
}

impl fmt::Display for ScheduleTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(time) => write!(f, "{}", time.format("%Y-%m-%dT%H:%M:%S%.f")),
            Self::Utc(time) => f.write_str(&time.to_rfc3339_opts(SecondsFormat::AutoSi, true)),
        }
    }
}

/// Parses the API format: RFC 3339 with an offset becomes [`ScheduleTime::Utc`]
/// (other offsets are converted to UTC), without an offset
/// [`ScheduleTime::Local`].
impl FromStr for ScheduleTime {
    type Err = ParseScheduleTimeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(time) = DateTime::parse_from_rfc3339(s) {
            return Ok(Self::Utc(time.with_timezone(&Utc)));
        }

        s.parse()
            .map(Self::Local)
            .map_err(|reason| ParseScheduleTimeError {
                input: s.to_owned(),
                reason,
            })
    }
}

impl Serialize for ScheduleTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ScheduleTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = ScheduleTime;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a date and time such as 2025-04-24T09:00:00")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                v.parse().map_err(E::custom)
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

/// Error returned when a string isn't a valid [`ScheduleTime`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid schedule time {input:?}: {reason}")]
pub struct ParseScheduleTimeError {
    input: String,
    reason: chrono::ParseError,
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, NaiveDate};

    use super::*;

    fn naive(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, mo, d)
            .unwrap()
            .and_hms_opt(h, mi, s)
            .unwrap()
    }

    #[test]
    fn parses_local_time() {
        let time: ScheduleTime = "2025-04-24T09:30:00".parse().unwrap();

        assert_eq!(time, ScheduleTime::Local(naive(2025, 4, 24, 9, 30, 0)));
    }

    #[test]
    fn parses_utc_time() {
        let time: ScheduleTime = "2023-09-16T09:00:00Z".parse().unwrap();

        assert_eq!(
            time,
            ScheduleTime::Utc(naive(2023, 9, 16, 9, 0, 0).and_utc())
        );
    }

    #[test]
    fn converts_other_offsets_to_utc() {
        let time: ScheduleTime = "2023-09-16T11:00:00+02:00".parse().unwrap();

        assert_eq!(time.to_string(), "2023-09-16T09:00:00Z");
    }

    #[test]
    fn displays_api_format() {
        for input in [
            "2025-04-24T09:00:00",
            "2025-04-24T09:00:00.500",
            "2023-09-16T09:00:00Z",
            "2023-09-16T09:00:00.250Z",
        ] {
            let time: ScheduleTime = input.parse().unwrap();

            assert_eq!(time.to_string(), input);
        }
    }

    #[test]
    fn rejects_invalid_input() {
        let err = "2025-04-24".parse::<ScheduleTime>().unwrap_err();

        assert!(
            err.to_string()
                .starts_with(r#"invalid schedule time "2025-04-24": "#),
            "{err}"
        );
    }

    #[test]
    fn serde_round_trips() {
        let json = r#"["2025-04-24T09:00:00","2023-09-16T09:00:00Z"]"#;
        let times: Vec<ScheduleTime> = serde_json::from_str(json).unwrap();

        assert!(!times[0].is_utc());
        assert!(times[1].is_utc());
        assert_eq!(serde_json::to_string(&times).unwrap(), json);
    }

    #[test]
    fn deserialize_reports_invalid_input() {
        let err = serde_json::from_str::<ScheduleTime>(r#""tomorrow""#).unwrap_err();

        assert!(err.to_string().contains("invalid schedule time"), "{err}");
    }

    #[test]
    fn accessors() {
        let local = ScheduleTime::Local(naive(2025, 4, 24, 9, 30, 0));
        let utc = ScheduleTime::Utc(naive(2023, 9, 16, 9, 0, 0).and_utc());

        assert_eq!(local.naive(), naive(2025, 4, 24, 9, 30, 0));
        assert_eq!(local.date(), NaiveDate::from_ymd_opt(2025, 4, 24).unwrap());
        assert_eq!(local.time(), NaiveTime::from_hms_opt(9, 30, 0).unwrap());
        assert_eq!(local.to_utc(), None);

        assert_eq!(utc.naive(), naive(2023, 9, 16, 9, 0, 0));
        assert_eq!(utc.to_utc(), Some(naive(2023, 9, 16, 9, 0, 0).and_utc()));
    }

    #[test]
    fn with_timezone_interprets_local_and_converts_utc() {
        let tz = FixedOffset::east_opt(2 * 3600).unwrap();
        let local = ScheduleTime::Local(naive(2025, 4, 24, 9, 0, 0));
        let utc = ScheduleTime::Utc(naive(2023, 9, 16, 9, 0, 0).and_utc());

        assert_eq!(
            local.with_timezone(&tz).single().unwrap().to_rfc3339(),
            "2025-04-24T09:00:00+02:00"
        );
        assert_eq!(
            utc.with_timezone(&tz).single().unwrap().to_rfc3339(),
            "2023-09-16T11:00:00+02:00"
        );
    }
}
