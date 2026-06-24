use std::fs::{self, File};
use std::io::{copy, Read};
use std::path::{Component, Path, PathBuf};

use zip::read::ZipArchive;

pub fn safe_join(base: &Path, entry: &str) -> Result<PathBuf, String> {
    let entry_path = Path::new(entry);
    if entry_path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!("Unsafe zip entry path: {entry}"));
    }

    if entry_path.is_absolute() {
        return Err(format!("Absolute zip entry path: {entry}"));
    }

    Ok(base.join(entry_path))
}

pub fn sanitize_filename(filename: &str) -> String {
    Path::new(filename)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("modfile")
        .to_string()
}

pub fn looks_like_zip(path: &Path) -> Result<bool, String> {
    let mut file =
        File::open(path).map_err(|e| format!("Could not open downloaded file: {e}"))?;
    let mut header = [0u8; 4];
    let read = file
        .read(&mut header)
        .map_err(|e| format!("Could not read downloaded file: {e}"))?;

    Ok(read >= 2 && header[0] == b'P' && header[1] == b'K')
}

pub fn validate_download_payload(path: &Path) -> Result<(), String> {
    let metadata =
        fs::metadata(path).map_err(|e| format!("Could not read downloaded file: {e}"))?;

    if metadata.len() == 0 {
        return Err(
            "Downloaded mod file is empty. You may need to sign in to mod.io to download this mod."
                .into(),
        );
    }

    let bytes = fs::read(path).map_err(|e| format!("Could not read downloaded file: {e}"))?;
    if bytes.starts_with(b"<") {
        return Err(
            "Download failed: received HTML instead of a mod file. Check your mod.io login and API configuration."
                .into(),
        );
    }

    if bytes.starts_with(b"{") || bytes.starts_with(b"[") {
        let preview = String::from_utf8_lossy(&bytes[..bytes.len().min(300)]);
        return Err(format!(
            "Download failed: server returned an error instead of a mod file. {preview}"
        ));
    }

    Ok(())
}

pub fn extract_zip(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let file = File::open(archive_path)
        .map_err(|e| format!("Could not open downloaded archive: {e}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| {
            let size = fs::metadata(archive_path)
                .map(|meta| meta.len())
                .unwrap_or(0);
            format!(
                "Could not read zip archive ({size} bytes on disk): {e}. If this keeps happening, sign in to mod.io and try again."
            )
        })?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("Could not read zip entry: {e}"))?;
        let entry_name = entry.name().to_string();

        if entry_name.ends_with('/') {
            let dir_path = safe_join(destination, entry_name.trim_end_matches('/'))?;
            fs::create_dir_all(&dir_path)
                .map_err(|e| format!("Could not create directory {}: {e}", dir_path.display()))?;
            continue;
        }

        let out_path = safe_join(destination, &entry_name)?;
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!("Could not create parent directory {}: {e}", parent.display())
            })?;
        }

        let mut out_file = File::create(&out_path)
            .map_err(|e| format!("Could not create file {}: {e}", out_path.display()))?;
        copy(&mut entry, &mut out_file)
            .map_err(|e| format!("Could not extract {}: {e}", out_path.display()))?;
    }

    Ok(())
}

pub fn install_downloaded_mod(
    download_path: &Path,
    destination: &Path,
    original_filename: &str,
) -> Result<(), String> {
    validate_download_payload(download_path)?;

    if looks_like_zip(download_path)? {
        return extract_zip(download_path, destination);
    }

    let filename = sanitize_filename(original_filename);
    let dest_path = safe_join(destination, &filename)?;
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Could not create parent directory {}: {e}",
                parent.display()
            )
        })?;
    }

    fs::copy(download_path, &dest_path).map_err(|e| {
        format!(
            "Could not install mod file to {}: {e}",
            dest_path.display()
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn detects_zip_header() {
        let dir = std::env::temp_dir().join("modkist-zip-test");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test.zip");
        let mut file = File::create(&path).unwrap();
        file.write_all(&[b'P', b'K', 3, 4]).unwrap();
        assert!(looks_like_zip(&path).unwrap());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_json_payload() {
        let dir = std::env::temp_dir().join("modkist-zip-test");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("error.json");
        fs::write(&path, br#"{"error":"unauthorized"}"#).unwrap();
        assert!(validate_download_payload(&path).is_err());
        let _ = fs::remove_file(path);
    }
}
