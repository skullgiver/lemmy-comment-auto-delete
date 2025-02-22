use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Deserializer};

// You can use this deserializer for any type that implements FromStr
// and the FromStr::Err implements Display
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