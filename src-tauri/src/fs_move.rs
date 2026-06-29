use std::fs;
use std::io;
use std::path::Path;

fn is_cross_device_error(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(17) | Some(18))
        || error.kind() == io::ErrorKind::CrossesDevices
}

fn copy_dir_recursive(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest = to.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

/// Move a directory, falling back to copy-and-delete when rename crosses volumes.
pub fn move_dir(from: &Path, to: &Path) -> Result<(), String> {
    if !from.is_dir() {
        return Ok(());
    }

    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(error) if is_cross_device_error(&error) => {
            copy_dir_recursive(from, to).map_err(|copy_error| {
                format!(
                    "Could not copy {} to {}: {copy_error}",
                    from.display(),
                    to.display()
                )
            })?;
            fs::remove_dir_all(from).map_err(|remove_error| {
                format!(
                    "Could not remove {} after copying to {}: {remove_error}",
                    from.display(),
                    to.display()
                )
            })
        }
        Err(error) => Err(format!(
            "Could not move {} to {}: {error}",
            from.display(),
            to.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn move_dir_renames_on_same_volume() {
        let temp = tempfile::tempdir().expect("tempdir");
        let from = temp.path().join("from");
        let to = temp.path().join("to");
        fs::create_dir_all(from.join("nested")).unwrap();
        fs::write(from.join("nested/mod.dll"), b"test").unwrap();

        move_dir(&from, &to).unwrap();

        assert!(!from.exists());
        assert!(to.join("nested/mod.dll").is_file());
    }

    #[test]
    fn copy_dir_recursive_duplicates_tree() {
        let temp = tempfile::tempdir().expect("tempdir");
        let from = temp.path().join("from");
        let to = temp.path().join("to");
        fs::create_dir_all(from.join("nested")).unwrap();
        fs::write(from.join("nested/mod.dll"), b"test").unwrap();

        copy_dir_recursive(&from, &to).unwrap();

        assert!(from.join("nested/mod.dll").is_file());
        assert!(to.join("nested/mod.dll").is_file());
    }
}
