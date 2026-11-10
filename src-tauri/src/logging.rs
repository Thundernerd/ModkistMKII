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

/// Crates whose own warnings and errors stay in the log file. The app logs the
/// outcome it gets back from them and decides there whether to report it.
const UNREPORTED_TARGETS: &[&str] = &[
    // Logs `error` on every failed update check, with no status code or URL;
    // app_updater reports the returned error when it is worth reporting.
    "tauri_plugin_updater",
];

struct SentryLogWriter;

impl LogWriter for SentryLogWriter {
    fn write(&self, _now: &mut DeferredNow, record: &Record) -> std::io::Result<()> {
        let Some(level) = sentry_level(record) else {
            return Ok(());
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

/// The Sentry level a log record is reported at, or `None` to keep it out of Sentry.
fn sentry_level(record: &Record) -> Option<sentry::Level> {
    let target = record.target();
    let unreported = UNREPORTED_TARGETS.iter().any(|crate_name| {
        target
            .strip_prefix(crate_name)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with("::"))
    });
    if unreported {
        return None;
    }

    match record.level() {
        log::Level::Error => Some(sentry::Level::Error),
        log::Level::Warn => Some(sentry::Level::Warning),
        _ => None,
    }
}

fn log_directory(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resolve(LOG_SUBDIR, BaseDirectory::AppData)
        .map_err(|error| format!("Did not resolve app log directory: {error}"))
}

/// Initialize Rust logging to a rotating file under `{app_data_dir}/logs`, next to
/// the JSON config stores. Mirror info-level (and above) messages to stderr in debug
/// builds unconditionally (so `tauri dev` shows logs even when stderr isn't a TTY),
/// and in release builds only when stderr is a terminal, so GUI/Steam/Deckify launches
/// do not panic on a closed or piped stderr.
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

    let duplicate = if cfg!(debug_assertions) || io::stderr().is_terminal() {
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

/// Captures, per test thread, the log records `SentryLogWriter` would send to
/// Sentry, so tests can assert what gets reported, and every record at info
/// or above, so tests can assert what reaches the log file.
#[cfg(test)]
pub(crate) mod report_capture {
    use std::cell::RefCell;
    use std::sync::OnceLock;

    thread_local! {
        static REPORTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
        static LOG_LINES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    struct ReportCapture;

    impl log::Log for ReportCapture {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }

        fn log(&self, record: &log::Record) {
            let line = format!("{}: {}", record.level(), record.args());
            if record.level() <= log::Level::Info {
                LOG_LINES.with(|lines| lines.borrow_mut().push(line.clone()));
            }
            if super::sentry_level(record).is_some() {
                REPORTS.with(|reports| reports.borrow_mut().push(line));
            }
        }

        fn flush(&self) {}
    }

    /// Install the capture (once per process) and clear this thread's reports
    /// and log lines.
    pub(crate) fn capture_reports() {
        static INSTALLED: OnceLock<()> = OnceLock::new();
        INSTALLED.get_or_init(|| {
            log::set_boxed_logger(Box::new(ReportCapture)).unwrap();
            log::set_max_level(log::LevelFilter::Trace);
        });
        REPORTS.with(|reports| reports.borrow_mut().clear());
        LOG_LINES.with(|lines| lines.borrow_mut().clear());
    }

    /// The `LEVEL: message` lines this thread would have sent to Sentry.
    pub(crate) fn sentry_reports() -> Vec<String> {
        REPORTS.with(|reports| reports.borrow().clone())
    }

    /// The `LEVEL: message` lines at info or above this thread logged: what
    /// the default file filter (`DEFAULT_LOG_FILTER`) keeps.
    pub(crate) fn log_lines() -> Vec<String> {
        LOG_LINES.with(|lines| lines.borrow().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_level(level: log::Level, target: &str, message: &str) -> Option<sentry::Level> {
        sentry_level(
            &Record::builder()
                .level(level)
                .target(target)
                .module_path(Some(target))
                .args(format_args!("{message}"))
                .build(),
        )
    }

    #[test]
    fn the_updater_plugins_own_errors_are_not_reported_to_sentry() {
        // Logged by the plugin on every failed check, before it returns the
        // error that app_updater reports or keeps quiet.
        let level = record_level(
            log::Level::Error,
            "tauri_plugin_updater::updater",
            "update endpoint did not respond with a successful status code",
        );

        assert_eq!(level, None);
    }

    #[test]
    fn app_warnings_and_errors_are_still_reported_to_sentry() {
        let warn = record_level(log::Level::Warn, "modkistmkii_lib::app_updater", "App update check failed");
        let error = record_level(log::Level::Error, "modkistmkii_lib::modio_api", "mod.io API error");
        let info = record_level(log::Level::Info, "modkistmkii_lib::app_updater", "App update check failed");

        assert_eq!(warn, Some(sentry::Level::Warning));
        assert_eq!(error, Some(sentry::Level::Error));
        assert_eq!(info, None);
    }
}
