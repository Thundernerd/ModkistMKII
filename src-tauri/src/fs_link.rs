use std::fs;
use std::path::Path;

/// Create a symbolic link at `dest` pointing at `source`.
///
/// The source path is canonicalized to an absolute path so the link stays valid
/// even if the working directory changes.
pub fn symlink_file(source: &Path, dest: &Path) -> Result<(), String> {
    let absolute_source = fs::canonicalize(source).map_err(|e| {
        format!(
            "Could not resolve source path {}: {e}",
            source.display()
        )
    })?;

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&absolute_source, dest).map_err(|e| {
            format!(
                "Could not link {} to {}: {e}",
                absolute_source.display(),
                dest.display()
            )
        })?;
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(&absolute_source, dest).map_err(|e| {
            format!(
                "Could not link {} to {}: {e}",
                absolute_source.display(),
                dest.display()
            )
        })?;
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (absolute_source, dest);
        return Err("Symbolic links are not supported on this platform.".into());
    }

    Ok(())
}
