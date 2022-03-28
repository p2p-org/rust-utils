use std::{path::PathBuf, str::FromStr};

use anyhow::Result;
use chrono::Local;
use flexi_logger::{
    Age, Cleanup, Criterion, Duplicate, FileSpec, Level, Logger, Naming, WriteMode,
};
use serde::{de::Error, Deserialize, Deserializer};

pub fn init_logger(logger_settings: LoggerSettings) -> Result<()> {
    let file_spec = FileSpec::try_from(logger_settings.path.clone())?;

    let _ = Logger::try_with_str(logger_settings.level.as_str())?
        .log_to_file(file_spec.clone())
        .write_mode(WriteMode::Direct) // if high performance mode is required need to use WriteMode::BufferAndFlush
        .duplicate_to_stdout(Duplicate::Info)
        .use_utc()
        .format(|out, _, record| {
            out.write_fmt(format_args!(
                "[{}][{}][{}]: {}",
                Local::now(),
                record.level(),
                record.target(),
                &record.args()
            ))
        })
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(logger_settings.keep_log_for_days),
        )
        .append()
        .start()?;

    log::info!(
        "Logger has been successfully initialized. All logs will be stored in the file: {}",
        logger_settings.path.display()
    );
    Ok(())
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct LoggerSettings {
    #[serde(deserialize_with = "deserialize_log_level")]
    pub level: Level,
    pub path: PathBuf,
    pub keep_log_for_days: usize,
}

fn deserialize_log_level<'de, D>(deserializer: D) -> Result<Level, D::Error>
where
    D: Deserializer<'de>,
{
    let level = String::deserialize(deserializer)?;
    Level::from_str(&level).map_err(Error::custom)
}

impl Default for LoggerSettings {
    fn default() -> Self {
        Self {
            level: Level::Debug,
            path: PathBuf::default().join("log.txt"),
            keep_log_for_days: 7,
        }
    }
}
