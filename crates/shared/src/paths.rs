use directories::ProjectDirs;
use serde::Deserialize;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "PathBuf")]
pub struct AbsolutePath(PathBuf);

impl TryFrom<PathBuf> for AbsolutePath {
    type Error = String;
    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        if path.is_absolute() {
            Ok(AbsolutePath(path))
        } else {
            Err(format!(
                "must be an absolute path, got `{}`",
                path.display()
            ))
        }
    }
}

impl AbsolutePath {
    #[must_use]
    pub fn into_inner(self) -> PathBuf {
        self.0
    }
}

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
    state_dir: Option<AbsolutePath>,
    runtime_dir: Option<AbsolutePath>,
}

pub struct Paths {
    pub state: PathBuf,
    pub runtime: PathBuf,
}

/// Where a directory came from, so an error can tell the user what to fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    ConfigFile,
    Systemd(&'static str),
    XdgDefault,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::ConfigFile => write!(f, "the config file"),
            Source::Systemd(var) => write!(f, "${var}"),
            Source::XdgDefault => write!(f, "the XDG default (check $HOME)"),
        }
    }
}

#[derive(Debug, Error)]
pub enum PathError {
    #[error("no {kind} directory could be determined")]
    NotFound { kind: DirKind },

    #[error("{kind} directory from {origin} must be absolute, got `{path}`")]
    NotAbsolute {
        kind: DirKind,
        origin: Source,
        path: PathBuf,
    },
}

/// Picks the first directory that is set, and remembers where it came from.
fn resolve_dir(
    explicit: Option<PathBuf>,
    systemd: Option<PathBuf>,
    env_var: &'static str,
    xdg: Option<PathBuf>,
) -> Option<(PathBuf, Source)> {
    explicit
        .map(|dir| (dir, Source::ConfigFile))
        .or(systemd.map(|dir| (dir, Source::Systemd(env_var))))
        .or(xdg.map(|dir| (dir, Source::XdgDefault)))
}

fn validate(kind: DirKind, found: Option<(PathBuf, Source)>) -> Result<PathBuf, PathError> {
    let (dir, origin) = found.ok_or(PathError::NotFound { kind })?;

    if !dir.is_absolute() {
        return Err(PathError::NotAbsolute {
            kind,
            origin,
            path: dir,
        });
    }
    Ok(dir)
}

fn find_dir(
    kind: DirKind,
    explicit: Option<PathBuf>,
    env_var: &'static str,
    xdg: Option<PathBuf>,
) -> Result<PathBuf, PathError> {
    let systemd = std::env::var_os(env_var).map(PathBuf::from);
    validate(kind, resolve_dir(explicit, systemd, env_var, xdg))
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
    pub fn resolve(self, app: App) -> Result<Paths, PathError> {
        Ok(Paths {
            state: state_dir(app, self.state_dir.map(AbsolutePath::into_inner))?,
            runtime: runtime_dir(app, self.runtime_dir.map(AbsolutePath::into_inner))?,
        })
    }
}

/// Where Axon keeps its persistent state
/// # Errors
/// `PathError::NotFound` if no directory can be determined,
/// `PathError::NotAbsolute` if the chosen path is relative.
fn state_dir(app: App, explicit: Option<PathBuf>) -> Result<PathBuf, PathError> {
    let xdg = ProjectDirs::from("", "", "axon")
        .and_then(|dirs| dirs.state_dir().map(|dir| dir.join(app.dir_name())));
    find_dir(DirKind::State, explicit, "STATE_DIRECTORY", xdg)
}

/// Where Axon keeps its runtime data
/// # Errors
/// `PathError::NotFound` if no directory can be determined,
/// `PathError::NotAbsolute` if the chosen path is relative.
fn runtime_dir(app: App, explicit: Option<PathBuf>) -> Result<PathBuf, PathError> {
    let xdg = ProjectDirs::from("", "", "axon")
        .and_then(|dirs| dirs.runtime_dir().map(|dir| dir.join(app.dir_name())));
    find_dir(DirKind::Runtime, explicit, "RUNTIME_DIRECTORY", xdg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const VAR: &str = "STATE_DIRECTORY";

    #[test]
    fn explicit() {
        let result = resolve_dir(
            Some(PathBuf::from("/a")),
            Some(PathBuf::from("/b")),
            VAR,
            Some(PathBuf::from("/c")),
        );
        assert_eq!(result, Some((PathBuf::from("/a"), Source::ConfigFile)));
    }

    #[test]
    fn systemd() {
        let result = resolve_dir(
            None,
            Some(PathBuf::from("/b")),
            VAR,
            Some(PathBuf::from("/c")),
        );
        assert_eq!(result, Some((PathBuf::from("/b"), Source::Systemd(VAR))));
    }

    #[test]
    fn xdg() {
        let result = resolve_dir(None, None, VAR, Some(PathBuf::from("/c")));
        assert_eq!(result, Some((PathBuf::from("/c"), Source::XdgDefault)));
    }

    #[test]
    fn none() {
        let result = resolve_dir(None, None, VAR, None);
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
    fn absolute_path_is_accepted() {
        let result = validate(
            DirKind::State,
            Some((PathBuf::from("/a"), Source::XdgDefault)),
        );
        assert!(matches!(result, Ok(p) if p == Path::new("/a")));
    }

    #[test]
    fn relative_path_names_where_it_came_from() {
        let result = validate(DirKind::State, Some((PathBuf::new(), Source::Systemd(VAR))));
        assert!(matches!(
            result,
            Err(PathError::NotAbsolute {
                origin: Source::Systemd(VAR),
                ..
            })
        ));
    }

    #[test]
    fn each_field_goes_to_its_own_path() {
        let config = RawPaths {
            state_dir: Some(AbsolutePath::try_from(PathBuf::from("/s")).unwrap()),
            runtime_dir: Some(AbsolutePath::try_from(PathBuf::from("/r")).unwrap()),
        };
        let paths = config.resolve(App::Daemon).unwrap();
        assert_eq!(paths.state, Path::new("/s"));
        assert_eq!(paths.runtime, Path::new("/r"));
    }

    #[test]
    fn explicit_config_file_is_used_as_given() {
        let result = config_file(App::Daemon, Some(PathBuf::from("my.toml")));
        assert!(matches!(result, Ok(p) if p == Path::new("my.toml")));
    }

    #[test]
    fn relative_path_in_file_is_refused() {
        let result = toml::from_str::<RawPaths>("state_dir = \"rel\"");
        assert!(result.is_err());
    }
}
