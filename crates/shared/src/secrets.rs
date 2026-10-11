use std::fs::{DirBuilder, File, OpenOptions, metadata};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use thiserror::Error;

const GROUP_AND_OTHERS: u32 = 0o077;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("{path} does not exist")]
    NotFound { path: PathBuf },

    #[error("{path} is too open (mode {mode:o}), group and others must have no access")]
    TooOpen { path: PathBuf, mode: u32 },

    #[error("cannot access {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

fn check_private(path: &Path, mode: u32) -> Result<(), SecretError> {
    if mode & GROUP_AND_OTHERS != 0 {
        return Err(SecretError::TooOpen {
            path: path.to_path_buf(),
            mode: mode & 0o7777,
        });
    }
    Ok(())
}

/// Creates `path` (and missing parents) as 0700, and checks that an existing folder is private too.
/// # Errors
/// `TooOpen` if the folder already exists with group or others access, `Io` if it can't be created or inspected.
pub fn create_private_dir(path: &Path) -> Result<(), SecretError> {
    let io = |source| SecretError::Io {
        path: path.to_path_buf(),
        source,
    };
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(io)?;
    let mode: u32 = metadata(path).map_err(io)?.permissions().mode();
    check_private(path, mode)
}

/// Creates a new file as 0600 in one step and writes `contents` into it.
/// # Errors
/// `Io` if the file already exists or can't be written.
pub fn create_secret(path: &Path, contents: &[u8]) -> Result<(), SecretError> {
    let io = |source| SecretError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut file: File = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(io)?;
    file.write_all(contents).map_err(io)
}

/// Reads a secret, refusing it if group or others have any access.
/// # Errors
/// `NotFound` if the file is missing, `TooOpen` if its mode is too open, `Io` for any other failure.
pub fn load_secret(path: &Path) -> Result<Vec<u8>, SecretError> {
    let io = |source| SecretError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut file: File = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|source| match source.kind() {
            ErrorKind::NotFound => SecretError::NotFound {
                path: path.to_path_buf(),
            },
            _ => SecretError::Io {
                path: path.to_path_buf(),
                source,
            },
        })?;
    let mode: u32 = file.metadata().map_err(io)?.permissions().mode();
    check_private(path, mode)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).map_err(io)?;
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{Permissions, set_permissions};

    fn write_with_mode(path: &Path, contents: &[u8], mode: u32) {
        std::fs::write(path, contents).unwrap();
        set_permissions(path, Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn read_secret_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret");
        write_with_mode(&path, b"test", 0o600);

        assert_eq!(load_secret(&path).unwrap(), b"test");
    }

    #[test]
    fn group_readable_secret_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret");
        write_with_mode(&path, b"test", 0o640);

        assert!(matches!(
            load_secret(&path),
            Err(SecretError::TooOpen { .. })
        ));
    }

    #[test]
    fn world_readable_secret_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret");
        write_with_mode(&path, b"test", 0o644);

        assert!(matches!(
            load_secret(&path),
            Err(SecretError::TooOpen { .. })
        ));
    }

    #[test]
    fn missing_file_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let file_path: PathBuf = dir.path().join("my-temporary-note.txt");
        assert!(matches!(
            load_secret(&file_path),
            Err(SecretError::NotFound { path }) if path == file_path
        ));
    }

    #[test]
    fn create_file_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret");

        create_secret(&path, b"test").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"test");
        let mode = metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn create_refuses_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret");
        write_with_mode(&path, b"old", 0o644);

        assert!(matches!(
            create_secret(&path, b"new"),
            Err(SecretError::Io { .. })
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"old");
    }

    #[test]
    fn new_dir_is_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state");

        create_private_dir(&path).unwrap();

        let mode = metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[test]
    fn existing_open_dir_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state");
        std::fs::create_dir(&path).unwrap();
        set_permissions(&path, Permissions::from_mode(0o755)).unwrap();

        assert!(matches!(
            create_private_dir(&path),
            Err(SecretError::TooOpen { .. })
        ));
    }
}
