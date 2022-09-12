use std::time::Duration;

use serde::{Deserialize, Deserializer};

pub fn deserialize_duration_secs_from_u64<'de, D>(d: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    u64::deserialize(d).map(Duration::from_secs)
}

pub fn deserialize_duration_ms_from_u64<'de, D>(d: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    u64::deserialize(d).map(Duration::from_millis)
}
