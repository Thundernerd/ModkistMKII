use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn runtime_var(name: &str) -> Option<String> {
    non_empty(std::env::var(name).ok())
}

fn sentry_dsn() -> Option<String> {
    runtime_var("SENTRY_DSN")
}

/// Initialize Sentry when `SENTRY_DSN` is set. Returns a guard that must stay alive for the
/// lifetime of the application.
pub fn init() -> sentry::ClientInitGuard {
    let dsn = sentry_dsn();

    ENABLED.store(dsn.is_some(), Ordering::Relaxed);

    sentry::init((
        dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(
                if cfg!(debug_assertions) {
                    "development"
                } else {
                    "production"
                }
                .into(),
            ),
            auto_session_tracking: true,
            ..Default::default()
        },
    ))
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn client() -> Option<std::sync::Arc<sentry::Client>> {
    sentry::Hub::current().client()
}
