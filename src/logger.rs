use std::path::PathBuf;

use anyhow::Result;
use chrono::Local;
use flexi_logger::{Age, Cleanup, Criterion, DeferredNow, Duplicate, FileSpec, Logger, Naming, WriteMode};
use log::{kv::source::as_map, Level, Log, Record};
use sentry::ClientInitGuard;
use serde::Deserialize;

pub fn init_logger(logger_settings: LoggerSettings) -> Result<Option<ClientInitGuard>> {
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

    let format_function = if logger_settings.gclogs {
        gclogs_format
    } else {
        output_format
    };

    let logger = logger.use_utc().format(format_function);

    let (logger, sentry_guard): (Box<dyn Log>, _) = if let Some(sentry_url) = logger_settings.sentry_server {
        (
            Box::new(sentry_log::SentryLogger::with_dest(logger.build()?.0)),
            Some(sentry::init((sentry_url, sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            }))),
        )
    } else {
        (logger.build()?.0, None)
    };

    log::set_boxed_logger(logger).unwrap();

    Ok(sentry_guard)
}

fn output_format(
    w: &mut dyn std::io::Write,
    _clock: &mut DeferredNow,
    record: &Record<'_>,
) -> Result<(), std::io::Error> {
    w.write_fmt(format_args!(
        "[{}][{}][{}]: {}",
        Local::now(),
        record.level(),
        record.target(),
        &record.args()
    ))
}

fn gclogs_format(
    w: &mut dyn std::io::Write,
    clock: &mut DeferredNow,
    record: &Record<'_>,
) -> Result<(), std::io::Error> {
    let message = record.args().to_string();
    let now = clock.now();
    let level = match record.level() {
        Level::Error => "ERROR",
        Level::Warn => "WARNING",
        Level::Info => "INFO",
        Level::Debug | Level::Trace => "DEBUG",
    };
    let module = record.module_path();
    let file = record.file();
    let line = record.line();
    let labels = as_map(record.key_values());
    let json = serde_json::json!({
        "severity": level,
        "message": message,
        "timestamp": {
            "seconds": now.unix_timestamp(),
            "nanos": now.nanosecond(),
        },
        "logging.googleapis.com/sourceLocation": {
            "file": file,
            "line": line,
        },
        "logging.googleapis.com/operation": {
            "producer": module,
        },
        "logging.googleapi.com/labels": labels,
    });
    serde_json::to_writer(w, &json)?;
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

    #[serde(default)]
    pub gclogs: bool,

    #[serde(default)]
    pub sentry_server: Option<String>,
}

impl Default for LoggerSettings {
    fn default() -> Self {
        Self {
            spec: default_spec(),
            path: None,
            keep_log_for_days: default_keep_log_for_days(),
            gclogs: false,
            sentry_server: None,
        }
    }
}

fn default_spec() -> String {
    "info".into()
}

const fn default_keep_log_for_days() -> usize {
    7
}
