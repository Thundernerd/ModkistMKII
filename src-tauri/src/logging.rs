/// Initialize Rust logging for the desktop app.
///
/// Filter via `RUST_LOG`, e.g. `RUST_LOG=modkistmkii_lib=debug` for verbose output.
pub fn init() {
    let _ = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("modkistmkii_lib=info"),
    )
    .format_timestamp_secs()
    .try_init();
}
