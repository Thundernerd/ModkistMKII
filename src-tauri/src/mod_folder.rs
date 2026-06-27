use std::fs;
use std::path::Path;

const MAX_MOD_NAME_SUFFIX_LEN: usize = 80;

pub fn sanitize_mod_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut sanitized = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => sanitized.push('_'),
            c if c.is_control() => {}
            c => sanitized.push(c),
        }
    }

    let sanitized = sanitized
        .trim_end_matches(['.', ' '])
        .trim()
        .to_string();
    truncate_mod_name_suffix(&sanitized)
}

fn truncate_mod_name_suffix(name: &str) -> String {
    if name.chars().count() <= MAX_MOD_NAME_SUFFIX_LEN {
        return name.to_string();
    }

    name.chars()
        .take(MAX_MOD_NAME_SUFFIX_LEN)
        .collect::<String>()
        .trim_end()
        .to_string()
}

pub fn install_folder_name(mod_id: u64, file_id: u64, mod_name: &str) -> String {
    let sanitized = sanitize_mod_name(mod_name);
    if sanitized.is_empty() {
        format!("{mod_id}_{file_id}")
    } else {
        format!("{mod_id}_{file_id}_{sanitized}")
    }
}

pub fn parse_install_folder_name(name: &str) -> Option<(u64, u64)> {
    let (mod_part, rest) = name.split_once('_')?;
    let mod_id = mod_part.parse().ok()?;
    let (file_part, _) = match rest.split_once('_') {
        Some((file_part, _suffix)) => (file_part, _suffix),
        None => (rest, ""),
    };
    let file_id = file_part.parse().ok()?;
    Some((mod_id, file_id))
}

pub fn is_valid_install_folder_name(name: &str) -> bool {
    parse_install_folder_name(name).is_some()
}

pub fn is_legacy_install_folder_name(name: &str) -> bool {
    let Some((mod_id, file_id)) = parse_install_folder_name(name) else {
        return false;
    };

    name == format!("{mod_id}_{file_id}")
}

pub fn rename_install_folder(parent_dir: &Path, from_name: &str, to_name: &str) -> Result<(), String> {
    if from_name == to_name {
        return Ok(());
    }

    let from = parent_dir.join(from_name);
    let to = parent_dir.join(to_name);

    if !from.is_dir() {
        return Ok(());
    }

    if to.exists() {
        fs::remove_dir_all(&to).map_err(|e| {
            format!(
                "Could not replace existing mod folder {}: {e}",
                to.display()
            )
        })?;
    }

    fs::rename(&from, &to).map_err(|e| {
        format!(
            "Could not rename mod folder {} to {}: {e}",
            from.display(),
            to.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_and_named_folder_names() {
        assert_eq!(parse_install_folder_name("12345_67890"), Some((12345, 67890)));
        assert_eq!(
            parse_install_folder_name("12345_67890_Cool Mod"),
            Some((12345, 67890))
        );
        assert_eq!(
            parse_install_folder_name("12345_67890_Cool_Mod_Name"),
            Some((12345, 67890))
        );
        assert_eq!(parse_install_folder_name("invalid"), None);
    }

    #[test]
    fn builds_named_install_folder() {
        assert_eq!(install_folder_name(1, 2, "Cool Mod"), "1_2_Cool Mod");
        assert_eq!(install_folder_name(1, 2, "  "), "1_2");
        assert_eq!(
            install_folder_name(1, 2, "Bad/Name:Here"),
            "1_2_Bad_Name_Here"
        );
    }

    #[test]
    fn detects_legacy_folder_names() {
        assert!(is_legacy_install_folder_name("12345_67890"));
        assert!(!is_legacy_install_folder_name("12345_67890_My Mod"));
    }

    #[test]
    fn renames_install_folder() {
        let root = std::env::temp_dir().join("modkist-mod-folder-rename");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(root.join("1_2")).unwrap();

        rename_install_folder(&root, "1_2", "1_2_Cool Mod").unwrap();

        assert!(!root.join("1_2").exists());
        assert!(root.join("1_2_Cool Mod").is_dir());

        let _ = fs::remove_dir_all(&root);
    }
}
