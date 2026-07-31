//! GameHub for Mac discovery: container store → game dir, virtual prefix, wine binary.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::game_path::{game_executable_in_dir, STEAM_APP_ID};

const APP_SUPPORT_REL: &str = "Library/Application Support/com.gamemac.www";
const CONTAINER_STORE_REL: &str = "gamehub/game_container_store.json";
const VIRTUAL_CONTAINERS_REL: &str = "wine-engine/container/wine_virtual_containers.json";
const BASE_CONTAINERS_REL: &str = "wine-engine/container/wine_containers.json";
const WINE_INSTALLATIONS_REL: &str = "wine-engine/container/wine_installations.json";
const OVERRIDE_VALUE: &str = "native,builtin";
const WINHTTP_KEYS: [&str; 2] = ["winhttp", "*winhttp"];

#[derive(Debug, Clone)]
pub struct GameHubInstall {
    pub game_dir: PathBuf,
    pub prefix: PathBuf,
    pub wine: PathBuf,
    pub label: String,
    pub virtual_container_id: String,
}

#[derive(Debug, Deserialize)]
struct ContainerStore {
    bindings: Vec<Binding>,
}

#[derive(Debug, Deserialize)]
struct Binding {
    game_name: Option<String>,
    platform_app_id: Option<String>,
    game_path: String,
    virtual_container_id: String,
    base_container_id: Option<String>,
}

pub fn support_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let dir = home.join(APP_SUPPORT_REL);
    dir.is_dir().then_some(dir)
}

/// All Zeepkist installs registered in GameHub's container store.
pub fn detect_zeepkist_installs() -> Vec<GameHubInstall> {
    let Some(support) = support_dir() else {
        return Vec::new();
    };
    detect_zeepkist_installs_in(&support)
}

pub fn detect_zeepkist_installs_in(support: &Path) -> Vec<GameHubInstall> {
    let store_path = support.join(CONTAINER_STORE_REL);
    let Ok(content) = fs::read_to_string(&store_path) else {
        return Vec::new();
    };
    let Ok(store) = serde_json::from_str::<ContainerStore>(&content) else {
        return Vec::new();
    };

    let mut installs = Vec::new();
    for binding in store.bindings {
        if !binding_is_zeepkist(&binding) {
            continue;
        }
        if let Some(install) = resolve_binding(support, &binding) {
            installs.push(install);
        }
    }
    installs
}

/// Resolve GameHub metadata for a configured game directory, if it belongs to GameHub.
pub fn find_for_game_dir(game_dir: &Path) -> Option<GameHubInstall> {
    let support = support_dir()?;
    find_for_game_dir_in(&support, game_dir)
}

pub fn find_for_game_dir_in(support: &Path, game_dir: &Path) -> Option<GameHubInstall> {
    let canonical = fs::canonicalize(game_dir).ok()?;
    detect_zeepkist_installs_in(support)
        .into_iter()
        .find(|install| {
            fs::canonicalize(&install.game_dir)
                .map(|path| path == canonical)
                .unwrap_or(false)
        })
}

/// Ensure virtual-container `dll_overrides` include winhttp for BepInEx.
/// Returns `Ok(true)` if the JSON was modified, `Ok(false)` if already configured.
pub fn ensure_winhttp_dll_override(virtual_container_id: &str) -> Result<bool, String> {
    let support = support_dir().ok_or_else(|| "GameHub support directory not found".to_string())?;
    ensure_winhttp_dll_override_in(&support, virtual_container_id)
}

pub fn ensure_winhttp_dll_override_in(
    support: &Path,
    virtual_container_id: &str,
) -> Result<bool, String> {
    let path = support.join(VIRTUAL_CONTAINERS_REL);
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let mut root: Value = serde_json::from_str(&content)
        .map_err(|error| format!("Could not parse {}: {error}", path.display()))?;

    let container = root
        .pointer_mut(&format!("/virtual_containers/{virtual_container_id}"))
        .ok_or_else(|| {
            format!("Virtual container {virtual_container_id} not found in GameHub config")
        })?;

    let overrides = container
        .as_object_mut()
        .ok_or_else(|| "Invalid virtual container object".to_string())?
        .entry("dll_overrides")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));

    let map = overrides
        .as_object_mut()
        .ok_or_else(|| "Invalid dll_overrides object".to_string())?;

    let mut changed = false;
    for key in WINHTTP_KEYS {
        let needs_update = match map.get(key).and_then(Value::as_str) {
            Some(value) => !override_includes_native(value),
            None => true,
        };
        if needs_update {
            map.insert(key.to_string(), Value::String(OVERRIDE_VALUE.to_string()));
            changed = true;
        }
    }

    if !changed {
        return Ok(false);
    }

    let serialized = serde_json::to_string_pretty(&root)
        .map_err(|error| format!("Could not serialize GameHub dll_overrides: {error}"))?;
    fs::write(&path, serialized)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    Ok(true)
}

fn binding_is_zeepkist(binding: &Binding) -> bool {
    if binding
        .platform_app_id
        .as_deref()
        .is_some_and(|id| id == STEAM_APP_ID)
    {
        return true;
    }
    binding
        .game_name
        .as_deref()
        .is_some_and(|name| name.eq_ignore_ascii_case("Zeepkist"))
}

fn resolve_binding(support: &Path, binding: &Binding) -> Option<GameHubInstall> {
    let game_dir = game_dir_from_binding(binding)?;
    let virtual_containers = read_json(support.join(VIRTUAL_CONTAINERS_REL))?;
    let container = virtual_containers
        .pointer(&format!(
            "/virtual_containers/{}",
            binding.virtual_container_id
        ))?
        .clone();

    let prefix = PathBuf::from(container.get("prefix_path")?.as_str()?);
    if !prefix.join("user.reg").is_file() {
        return None;
    }

    let label = container
        .get("name")
        .and_then(Value::as_str)
        .map(|name| format!("GameHub ({name})"))
        .unwrap_or_else(|| "GameHub".to_string());

    let wine_installation_id = wine_installation_id_for_binding(support, binding, &container)?;
    let wine = wine_binary(support, &wine_installation_id)?;

    Some(GameHubInstall {
        game_dir,
        prefix,
        wine,
        label,
        virtual_container_id: binding.virtual_container_id.clone(),
    })
}

fn game_dir_from_binding(binding: &Binding) -> Option<PathBuf> {
    let library = PathBuf::from(&binding.game_path);
    let steamapps = library.join("steamapps");
    let manifest = steamapps.join(format!("appmanifest_{STEAM_APP_ID}.acf"));
    if manifest.is_file() {
        if let Ok(content) = fs::read_to_string(&manifest) {
            if let Some(installdir) = installdir_from_manifest(&content) {
                let game_dir = steamapps.join("common").join(installdir);
                if game_executable_in_dir(&game_dir).is_some() {
                    return Some(game_dir);
                }
            }
        }
    }

    let fallback = steamapps.join("common").join("Zeepkist");
    game_executable_in_dir(&fallback).map(|_| fallback)
}

fn wine_installation_id_for_binding(
    support: &Path,
    binding: &Binding,
    virtual_container: &Value,
) -> Option<String> {
    if let Some(id) = virtual_container
        .get("wine_installation_id")
        .and_then(Value::as_str)
    {
        return Some(id.to_string());
    }

    let base_id = binding
        .base_container_id
        .clone()
        .or_else(|| {
            virtual_container
                .get("base_container_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })?;

    let base_containers = read_json(support.join(BASE_CONTAINERS_REL))?;
    base_containers
        .pointer(&format!("/containers/{base_id}/wine_installation_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn wine_binary(support: &Path, wine_installation_id: &str) -> Option<PathBuf> {
    let installations = read_json(support.join(WINE_INSTALLATIONS_REL))?;
    let install = installations.pointer(&format!(
        "/wine_installations/{wine_installation_id}"
    ))?;

    if let Some(path) = install
        .pointer("/metadata/wine_executable")
        .and_then(Value::as_str)
    {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    let install_path = install.get("install_path")?.as_str()?;
    let candidate = PathBuf::from(install_path).join("bin/wine");
    candidate.is_file().then_some(candidate)
}

fn read_json(path: PathBuf) -> Option<Value> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn installdir_from_manifest(content: &str) -> Option<String> {
    let needle = "\"installdir\"";
    let index = content.find(needle)?;
    let after_key = &content[index + needle.len()..];
    let value_start = after_key.find('"')?;
    let rest = &after_key[value_start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn override_includes_native(value: &str) -> bool {
    value
        .split(',')
        .map(str::trim)
        .any(|part| part.eq_ignore_ascii_case("native"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_fixture(root: &Path) {
        let game_library = root.join("GameLibrary");
        let game_dir = game_library.join("steamapps/common/Zeepkist");
        fs::create_dir_all(&game_dir).unwrap();
        fs::write(game_dir.join("Zeepkist.exe"), b"").unwrap();
        fs::write(
            game_library.join("steamapps/appmanifest_1440670.acf"),
            "\"AppState\"\n{\n\t\"installdir\"\t\t\"Zeepkist\"\n}\n",
        )
        .unwrap();

        let support = root.join("support");
        fs::create_dir_all(support.join("gamehub")).unwrap();
        fs::create_dir_all(support.join("wine-engine/container")).unwrap();
        fs::create_dir_all(support.join("wine-engine/containers/virtual_containers/1")).unwrap();
        fs::create_dir_all(support.join("wine-engine/containers/wine_installations/10000073/bin"))
            .unwrap();

        let prefix = support.join("wine-engine/containers/virtual_containers/1");
        fs::write(prefix.join("user.reg"), "WINE REGISTRY Version 2\n").unwrap();

        let wine_bin = support.join("wine-engine/containers/wine_installations/10000073/bin/wine");
        fs::write(&wine_bin, b"").unwrap();

        let store = format!(
            r#"{{
  "schema_version": 2,
  "bindings": [
    {{
      "id": 1,
      "game_name": "Zeepkist",
      "platform": "steam",
      "platform_app_id": "1440670",
      "game_path": "{}",
      "virtual_container_id": "1",
      "base_container_id": "1"
    }}
  ]
}}"#,
            game_library.display()
        );
        fs::write(support.join(CONTAINER_STORE_REL), store).unwrap();

        let virtual_containers = format!(
            r#"{{
  "virtual_containers": {{
    "1": {{
      "id": "1",
      "base_container_id": "1",
      "prefix_path": "{}",
      "name": "gamehub-1",
      "dll_overrides": {{
        "mscoree": "native,builtin"
      }}
    }}
  }}
}}"#,
            prefix.display()
        );
        fs::write(support.join(VIRTUAL_CONTAINERS_REL), virtual_containers).unwrap();

        let base_containers = r#"{
  "containers": {
    "1": {
      "id": "1",
      "wine_installation_id": "10000073"
    }
  }
}"#;
        fs::write(support.join(BASE_CONTAINERS_REL), base_containers).unwrap();

        let installations = format!(
            r#"{{
  "wine_installations": {{
    "10000073": {{
      "id": "10000073",
      "install_path": "{}",
      "metadata": {{
        "wine_executable": "{}"
      }}
    }}
  }}
}}"#,
            support
                .join("wine-engine/containers/wine_installations/10000073")
                .display(),
            wine_bin.display()
        );
        fs::write(support.join(WINE_INSTALLATIONS_REL), installations).unwrap();
    }

    #[test]
    fn detects_zeepkist_from_container_store() {
        let root = tempfile::tempdir().unwrap();
        write_fixture(root.path());
        let support = root.path().join("support");

        let installs = detect_zeepkist_installs_in(&support);
        assert_eq!(installs.len(), 1);
        let install = &installs[0];
        assert!(install.game_dir.ends_with("common/Zeepkist"));
        assert!(install.prefix.ends_with("virtual_containers/1"));
        assert!(install.wine.ends_with("bin/wine"));
        assert_eq!(install.virtual_container_id, "1");
        assert!(install.label.contains("GameHub"));
    }

    #[test]
    fn finds_install_for_game_dir() {
        let root = tempfile::tempdir().unwrap();
        write_fixture(root.path());
        let support = root.path().join("support");
        let game_dir = root.path().join("GameLibrary/steamapps/common/Zeepkist");

        let install = find_for_game_dir_in(&support, &game_dir).expect("GameHub install");
        assert_eq!(install.virtual_container_id, "1");
    }

    #[test]
    fn patches_dll_overrides_winhttp() {
        let root = tempfile::tempdir().unwrap();
        write_fixture(root.path());
        let support = root.path().join("support");

        assert!(ensure_winhttp_dll_override_in(&support, "1").unwrap());
        assert!(!ensure_winhttp_dll_override_in(&support, "1").unwrap());

        let content =
            fs::read_to_string(support.join(VIRTUAL_CONTAINERS_REL)).unwrap();
        let value: Value = serde_json::from_str(&content).unwrap();
        let overrides = value
            .pointer("/virtual_containers/1/dll_overrides")
            .unwrap();
        assert_eq!(
            overrides.get("winhttp").and_then(Value::as_str),
            Some("native,builtin")
        );
        assert_eq!(
            overrides.get("*winhttp").and_then(Value::as_str),
            Some("native,builtin")
        );
        assert_eq!(
            overrides.get("mscoree").and_then(Value::as_str),
            Some("native,builtin")
        );
    }
}
