use std::io::{self, IsTerminal};
use std::path::PathBuf;

use flexi_logger::writers::LogWriter;
use flexi_logger::{Cleanup, Criterion, DeferredNow, Duplicate, ErrorChannel, FileSpec, Logger, Naming};
use log::Record;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

use crate::sentry_init;

const DEFAULT_LOG_FILTER: &str = "info,modkistmkii_lib=info";
const LOG_BASENAME: &str = "modkist";
const LOG_SUBDIR: &str = "logs";
const LOGGER_ERROR_BASENAME: &str = "flexi_logger-errors.log";
const MAX_LOG_FILE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_LOG_FILES: usize = 5;

struct SentryLogWriter;

impl LogWriter for SentryLogWriter {
    fn write(&self, _now: &mut DeferredNow, record: &Record) -> std::io::Result<()> {
        let level = match record.level() {
            log::Level::Error => sentry::Level::Error,
            log::Level::Warn => sentry::Level::Warning,
            _ => return Ok(()),
        };

        let message = record.args().to_string();
        sentry::with_scope(
            |scope| {
                if let Some(module) = record.module_path() {
                    scope.set_tag("log.module", module);
                }
                if let Some(target) = record.target().strip_prefix("modkistmkii_lib::") {
                    scope.set_tag("log.target", target);
                }
            },
            || {
                sentry::capture_message(&message, level);
            },
        );

        Ok(())
    }

    fn flush(&self) -> std::io::Result<()> {
        Ok(())
    }

    fn max_log_level(&self) -> log::LevelFilter {
        log::LevelFilter::Warn
    }
}

fn log_directory(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resolve(LOG_SUBDIR, BaseDirectory::AppData)
        .map_err(|error| format!("Did not resolve app log directory: {error}"))
}

/// Initialize Rust logging to a rotating file under `{app_data_dir}/logs`, next to
/// the JSON config stores. Mirror info-level (and above) messages to stderr only
/// when stderr is a terminal, so GUI/Steam/Deckify launches do not panic on a
/// closed or piped stderr.
///
/// Filter via `RUST_LOG`, e.g. `RUST_LOG=modkistmkii_lib=debug` for verbose output.
pub fn init(app: &AppHandle) -> Result<PathBuf, String> {
    let log_dir = log_directory(app)?;
    std::fs::create_dir_all(&log_dir)
        .map_err(|error| format!("Did not create log directory {}: {error}", log_dir.display()))?;

    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_LOG_FILTER.into());

    let file_spec = FileSpec::default()
        .directory(&log_dir)
        .basename(LOG_BASENAME)
        .suffix("log");

    let mut logger = Logger::try_with_str(filter).map_err(|error| format!("Invalid log filter: {error}"))?;

    logger = if sentry_init::is_enabled() {
        logger.log_to_file_and_writer(file_spec, Box::new(SentryLogWriter))
    } else {
        logger.log_to_file(file_spec)
    };

    let duplicate = if io::stderr().is_terminal() {
        Duplicate::Info
    } else {
        Duplicate::None
    };

    logger
        .rotate(
            Criterion::Size(MAX_LOG_FILE_BYTES),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(MAX_LOG_FILES),
        )
        .duplicate_to_stderr(duplicate)
        .error_channel(ErrorChannel::File(log_dir.join(LOGGER_ERROR_BASENAME)))
        .panic_if_error_channel_is_broken(false)
        .format_for_files(flexi_logger::detailed_format)
        .format_for_writer(flexi_logger::colored_default_format)
        .start()
        .map_err(|error| format!("Did not start logger: {error}"))?;

    Ok(log_dir)
}

#[tauri::command]
pub fn log_directory_path(app: AppHandle) -> Result<String, String> {
    log_directory(&app).map(|path| path.display().to_string())
}
