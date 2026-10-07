use directories::ProjectDirs;
use serde::Deserialize;
use std::path::PathBuf;
use thiserror::Error;

/// Each binary gets its own subfolder under the XDG defaults, so their files never mix.
#[derive(Debug, Clone, Copy)]
pub enum App {
    Daemon,
    Orchestrator,
    Desk,
    Cli,
}

impl App {
    fn dir_name(self) -> &'static str {
        match self {
            App::Daemon => "daemon",
            App::Orchestrator => "orchestrator",
            App::Desk => "desk",
            App::Cli => "cli",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DirKind {
    State,
    Runtime,
    Config,
}

impl std::fmt::Display for DirKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DirKind::State => write!(f, "state"),
            DirKind::Runtime => write!(f, "runtime"),
            DirKind::Config => write!(f, "config"),
        }
    }
}

/// What the user may write in `config.toml`. Every field is optional.
#[derive(Deserialize, Default, Debug)]
#[serde(deny_unknown_fields)]
pub struct RawPaths {
    state_dir: Option<PathBuf>,
    runtime_dir: Option<PathBuf>,
}

pub struct Paths {
    pub state: PathBuf,
    pub runtime: PathBuf,
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

/// Where a binary's config file is: `explicit` as given (the `--config` flag),
/// else `config.toml` in the systemd directory, else in `~/.config/axon/<app>/`.
/// # Errors
/// `PathError::NotFound` if no directory can be determined,
/// `PathError::NotAbsolute` if the systemd directory is relative.
pub fn config_file(app: App, explicit: Option<PathBuf>) -> Result<PathBuf, PathError> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    let xdg = ProjectDirs::from("", "", "axon").map(|dirs| dirs.config_dir().join(app.dir_name()));
    let dir = find_dir(DirKind::Config, None, "CONFIGURATION_DIRECTORY", xdg)?;
    Ok(dir.join("config.toml"))
}

impl RawPaths {
    /// Resolves every directory (explicit → systemd → XDG) into checked, absolute paths.
    /// Only the XDG default gets the per-app subfolder: explicit and systemd paths are already specific.
    /// # Errors
    /// The first `PathError` from any directory.
    pub fn resolve(&self, app: App) -> Result<Paths, PathError> {
        Ok(Paths {
            state: self.state_dir(app)?,
            runtime: self.runtime_dir(app)?,
        })
    }

    /// Where Axon keeps its persistent state
    /// # Errors
    /// `PathError::NotFound` if no directory can be determined,
    /// `PathError::NotAbsolute` if the chosen path is relative.
    fn state_dir(&self, app: App) -> Result<PathBuf, PathError> {
        let xdg = ProjectDirs::from("", "", "axon")
            .and_then(|dirs| dirs.state_dir().map(|dir| dir.join(app.dir_name())));
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
    fn runtime_dir(&self, app: App) -> Result<PathBuf, PathError> {
        let xdg = ProjectDirs::from("", "", "axon")
            .and_then(|dirs| dirs.runtime_dir().map(|dir| dir.join(app.dir_name())));
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
    use std::path::Path;

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
        let result = toml::from_str::<RawPaths>("stat_dir = \"/x\"");
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
        let config = RawPaths {
            state_dir: Some(PathBuf::from("/s")),
            runtime_dir: Some(PathBuf::from("/r")),
        };
        assert_eq!(config.state_dir(App::Daemon).unwrap(), Path::new("/s"));
        assert_eq!(config.runtime_dir(App::Daemon).unwrap(), Path::new("/r"));
    }

    #[test]
    fn explicit_config_file_is_used_as_given() {
        let result = config_file(App::Daemon, Some(PathBuf::from("my.toml")));
        assert!(matches!(result, Ok(p) if p == Path::new("my.toml")));
    }
}
