use std::path::PathBuf;

use anyhow::Result;
use chrono::Local;
use flexi_logger::{
    filter::{LogLineFilter, LogLineWriter},
    Age, Cleanup, Criterion, DeferredNow, Duplicate, FileSpec, Level, Logger, Naming, WriteMode,
};
use serde::Deserialize;

pub struct CratesFilter;
impl LogLineFilter for CratesFilter {
    fn write(
        &self,
        now: &mut DeferredNow,
        record: &log::Record,
        log_line_writer: &dyn LogLineWriter,
    ) -> std::io::Result<()> {
        if let Some(module_path) = record.module_path() {
            if !module_path.contains("sqlx") && record.level() < Level::Info {
                log_line_writer.write(now, record)?;
            }
        }

        Ok(())
    }
}

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
            .filter(Box::new(CratesFilter {}))
            .append()
    };

    logger
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
        .start()?;

    log::info!("Logger has been successfully initialized.");
    if let Some(path) = &logger_settings.path {
        log::info!("All logs will be stored in the file: {}", path.display());
    }

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
