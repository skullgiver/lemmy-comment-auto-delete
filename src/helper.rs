use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Deserializer};

/// You can use this deserializer for any type that implements FromStr
/// and the FromStr::Err implements Display
///
/// Modified version of the helper from this StackOverflow example:
/// https://stackoverflow.com/questions/57614558/how-to-use-a-custom-serde-deserializer-for-chrono-timestamps/57623355#57623355
pub fn deserialize_date<'de, D>(deserializer: D) -> Result<DateTime<Utc>, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let format = "%Y-%m-%dT%H:%M:%S%.6f";
    NaiveDateTime::parse_and_remainder(&s, format)
        .map(|(local_date, _remainder)| {
            DateTime::from_naive_utc_and_offset(local_date, Utc)
        })
        .map_err(serde::de::Error::custom)
}