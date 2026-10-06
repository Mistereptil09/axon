use directories::ProjectDirs;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Copy)]
pub enum DirKind {
    State,
    Runtime,
}

impl std::fmt::Display for DirKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DirKind::State => write!(f, "state"),
            DirKind::Runtime => write!(f, "runtime"),
        }
    }
}

/// What the user may write in `config.toml`. Every field is optional.
#[derive(Deserialize, Default, Debug)]
#[serde(deny_unknown_fields)]
pub struct PathsConfig {
    pub state_dir: Option<PathBuf>,
    pub runtime_dir: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub enum PathError {
    #[error("no {kind} directory could be determined")]
    NotFound { kind: DirKind },

    #[error("{kind} directory must be absolute: {path}")]
    NotAbsolute { kind: DirKind, path: PathBuf },
}

fn resolve_dir(
    explicit: Option<PathBuf>,
    systemd: Option<PathBuf>,
    xdg: Option<PathBuf>,
) -> Option<PathBuf> {
    explicit.or(systemd).or(xdg)
}

fn validate(kind: DirKind, dir: Option<PathBuf>) -> Result<PathBuf, PathError> {
    let dir = dir.ok_or(PathError::NotFound { kind })?;

    if !dir.is_absolute() {
        return Err(PathError::NotAbsolute { kind, path: dir });
    }
    Ok(dir)
}

fn find_dir(
    kind: DirKind,
    explicit: Option<PathBuf>,
    env_var: &str,
    xdg: Option<PathBuf>,
) -> Result<PathBuf, PathError> {
    let systemd = std::env::var_os(env_var).map(PathBuf::from);
    validate(kind, resolve_dir(explicit, systemd, xdg))
}

impl PathsConfig {
    /// Where Axon keeps its persistent state
    /// # Errors
    /// `PathError::NotFound` if no directory can be determined,
    /// `PathError::NotAbsolute` if the chosen path is relative.
    pub fn state_dir(&self) -> Result<PathBuf, PathError> {
        let xdg = ProjectDirs::from("", "", "axon")
            .and_then(|dirs| dirs.state_dir().map(Path::to_path_buf));
        find_dir(
            DirKind::State,
            self.state_dir.clone(),
            "STATE_DIRECTORY",
            xdg,
        )
    }

    /// Where Axon keeps its runtime data
    /// # Errors
    /// `PathError::NotFound` if no directory can be determined,
    /// `PathError::NotAbsolute` if the chosen path is relative.
    pub fn runtime_dir(&self) -> Result<PathBuf, PathError> {
        let xdg = ProjectDirs::from("", "", "axon")
            .and_then(|dirs| dirs.runtime_dir().map(Path::to_path_buf));
        find_dir(
            DirKind::Runtime,
            self.runtime_dir.clone(),
            "RUNTIME_DIRECTORY",
            xdg,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit() {
        let result = resolve_dir(
            Some(PathBuf::from("/a")),
            Some(PathBuf::from("/b")),
            Some(PathBuf::from("/c")),
        );
        assert_eq!(result, Some(PathBuf::from("/a")));
    }

    #[test]
    fn systemd() {
        let result = resolve_dir(None, Some(PathBuf::from("/b")), Some(PathBuf::from("/c")));
        assert_eq!(result, Some(PathBuf::from("/b")));
    }

    #[test]
    fn xdg() {
        let result = resolve_dir(None, None, Some(PathBuf::from("/c")));
        assert_eq!(result, Some(PathBuf::from("/c")));
    }

    #[test]
    fn none() {
        let result = resolve_dir(None, None, None);
        assert_eq!(result, None);
    }

    #[test]
    fn unknown_field_is_refused() {
        let result = toml::from_str::<PathsConfig>("stat_dir = \"/x\"");
        assert!(result.is_err());
    }

    #[test]
    fn nothing_found_is_an_error() {
        let result = validate(DirKind::State, None);
        assert!(matches!(result, Err(PathError::NotFound { .. })));
    }

    #[test]
    fn relative_path_is_refused() {
        let result = validate(DirKind::State, Some(PathBuf::from("data")));
        assert!(matches!(result, Err(PathError::NotAbsolute { .. })));
    }

    #[test]
    fn absolute_path_is_accepted() {
        let result = validate(DirKind::State, Some(PathBuf::from("/a")));
        assert!(matches!(result, Ok(p) if p == Path::new("/a")));
    }

    #[test]
    fn each_method_uses_its_own_field() {
        let config = PathsConfig {
            state_dir: Some(PathBuf::from("/s")),
            runtime_dir: Some(PathBuf::from("/r")),
        };
        assert_eq!(config.state_dir().unwrap(), Path::new("/s"));
        assert_eq!(config.runtime_dir().unwrap(), Path::new("/r"));
    }
}
