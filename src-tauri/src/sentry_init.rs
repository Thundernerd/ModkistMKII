use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Initialize Sentry when `SENTRY_DSN` is set. Returns a guard that must stay alive for the
/// lifetime of the application.
pub fn init() -> sentry::ClientInitGuard {
    let dsn = std::env::var("SENTRY_DSN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

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
            ..Default::default()
        },
    ))
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}
