use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

const RESERVATION_ATTEMPTS: u8 = 32;

/// Replaces a file only after the complete new payload is written and synced.
/// Until the rename succeeds, any previous readable file remains untouched.
pub fn replace_file_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let committed = replace_file_atomically_if(path, bytes, || true)?;
    debug_assert!(committed);
    Ok(())
}

/// Writes and syncs a complete replacement, then asks the caller for one
/// final commit decision immediately before the atomic rename.
pub fn replace_file_atomically_if(
    path: &Path,
    bytes: &[u8],
    should_commit: impl FnOnce() -> bool,
) -> io::Result<bool> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path has no file name: {}", path.display()),
        )
    })?;

    for attempt in 0..RESERVATION_ATTEMPTS {
        let temporary_path = parent.join(format!(
            ".{}.{}.{}.tmp",
            file_name.to_string_lossy(),
            std::process::id(),
            attempt
        ));
        let mut temporary = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };

        if let Err(error) = temporary
            .write_all(bytes)
            .and_then(|()| temporary.sync_all())
        {
            drop(temporary);
            let _ = fs::remove_file(&temporary_path);
            return Err(error);
        }
        drop(temporary);
        if !should_commit() {
            fs::remove_file(&temporary_path)?;
            return Ok(false);
        }
        if let Err(error) = fs::rename(&temporary_path, path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(error);
        }
        fs::File::open(parent)?.sync_all()?;
        return Ok(true);
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "could not reserve a temporary file after {RESERVATION_ATTEMPTS} attempts: {}",
            path.display()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn replacement_is_complete_and_leaves_no_temporary_file() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("camera-man-atomic-{}-{nonce}", std::process::id()));
        let path = directory.join("state.json");
        fs::create_dir_all(&directory).unwrap();
        fs::write(&path, b"old").unwrap();

        replace_file_atomically(&path, b"complete new state").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"complete new state");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejected_commit_preserves_the_previous_file_and_cleans_up() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("camera-man-cancel-{}-{nonce}", std::process::id()));
        let path = directory.join("state.json");
        fs::create_dir_all(&directory).unwrap();
        fs::write(&path, b"old").unwrap();

        assert!(!replace_file_atomically_if(&path, b"new", || false).unwrap());

        assert_eq!(fs::read(&path).unwrap(), b"old");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }
}
