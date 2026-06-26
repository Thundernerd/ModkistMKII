use std::path::PathBuf;

const EMBEDDED_MODIO_KEYS: &[&str] = &["MODIO_API_KEY", "MODIO_GAME_ID"];

fn main() {
    embed_modio_env_for_release();
    tauri_build::build();
}

fn embed_modio_env_for_release() {
    if std::env::var("PROFILE").as_deref() != Ok("release") {
        return;
    }

    let env_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../.env");
    println!("cargo:rerun-if-changed={}", env_path.display());

    if env_path.is_file() {
        let _ = dotenvy::from_path(&env_path);
    }

    for key in EMBEDDED_MODIO_KEYS {
        match std::env::var(key) {
            Ok(value) if !value.trim().is_empty() => {
                println!("cargo:rustc-env={key}={}", value.trim());
            }
            Ok(_) => {
                println!("cargo:warning={key} is empty; mod.io will not work in release builds");
            }
            Err(_) => {
                println!(
                    "cargo:warning={key} is not set for release build (use .env or environment variables)"
                );
            }
        }
    }
}
