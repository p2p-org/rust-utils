use std::path::PathBuf;

use anyhow::Result;
use chrono::Local;
use flexi_logger::{Age, Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming, WriteMode};
use serde::Deserialize;

pub fn init_logger(logger_settings: LoggerSettings) -> Result<()> {
    let mut logger = Logger::try_with_str(&logger_settings.spec)?;

    if let Some(path) = &logger_settings.path {
        let file_spec = FileSpec::try_from(path.clone())?;
        logger = logger
            .log_to_file(file_spec)
            .duplicate_to_stdout(Duplicate::All)
            .write_mode(WriteMode::Direct) // if high performance mode is required need to use WriteMode::BufferAndFlush
            .rotate(
                Criterion::Age(Age::Day),
                Naming::Timestamps,
                Cleanup::KeepLogFiles(logger_settings.keep_log_for_days),
            )
            .append()
    };

    let logger = logger.use_utc().format(|out, _, record| {
        out.write_fmt(format_args!(
            "[{}][{}][{}]: {}",
            Local::now(),
            record.level(),
            record.target(),
            &record.args()
        ))
    });

    let logger = sentry_log::SentryLogger::with_dest(logger.build()?.0);

    log::set_boxed_logger(Box::new(logger)).unwrap();

    Ok(())
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct LoggerSettings {
    #[serde(default = "default_spec")]
    pub spec: String,

    #[serde(default)]
    pub path: Option<PathBuf>,

    #[serde(default = "default_keep_log_for_days")]
    pub keep_log_for_days: usize,
}

impl Default for LoggerSettings {
    fn default() -> Self {
        Self {
            spec: default_spec(),
            path: None,
            keep_log_for_days: default_keep_log_for_days(),
        }
    }
}

fn default_spec() -> String {
    "info".into()
}

const fn default_keep_log_for_days() -> usize {
    7
}
