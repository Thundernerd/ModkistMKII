fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn runtime_var(name: &str) -> Option<String> {
    non_empty(std::env::var(name).ok())
}

#[cfg(not(debug_assertions))]
fn baked_api_key() -> Option<String> {
    non_empty(option_env!("MODIO_API_KEY").map(str::to_string))
}

#[cfg(debug_assertions)]
fn baked_api_key() -> Option<String> {
    None
}

#[cfg(not(debug_assertions))]
fn baked_game_id() -> Option<String> {
    non_empty(option_env!("MODIO_GAME_ID").map(str::to_string))
}

#[cfg(debug_assertions)]
fn baked_game_id() -> Option<String> {
    None
}

pub fn modio_api_key() -> Option<String> {
    baked_api_key().or_else(|| runtime_var("MODIO_API_KEY"))
}

pub fn modio_game_id() -> Option<u64> {
    baked_game_id()
        .or_else(|| runtime_var("MODIO_GAME_ID"))
        .and_then(|value| value.parse().ok())
}
