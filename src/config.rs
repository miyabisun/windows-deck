//! `%LOCALAPPDATA%\windows-deck\config.yaml` (or `WINDOWS_DECK_CONFIG`):
//!
//! ```yaml
//! link: http://127.0.0.1:4730   # windows-link URL
//! monitor: JAPANNEXT MNT        # monitor id, name or display name from windows-link
//! ```
//!
//! Both keys are optional. A missing file means the defaults; a broken one also means the
//! defaults, with the reason passed to the panel so it can say so.

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const DEFAULT_LINK: &str = "http://127.0.0.1:4730";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    link: Option<String>,
    monitor: Option<String>,
}

/// What the panel receives from `deck_config`.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Loaded {
    pub link: String,
    pub monitor: Option<String>,
    pub error: Option<String>,
    /// The file read, for the panel to name when it asks for a monitor.
    pub path: String,
}

impl Loaded {
    fn defaults(error: Option<String>) -> Self {
        Self {
            link: DEFAULT_LINK.into(),
            monitor: None,
            error,
            path: String::new(),
        }
    }
}

pub fn default_path() -> PathBuf {
    if let Some(path) = std::env::var_os("WINDOWS_DECK_CONFIG") {
        return path.into();
    }
    let base = std::env::var_os("LOCALAPPDATA").map_or_else(|| PathBuf::from("."), PathBuf::from);
    base.join("windows-deck").join("config.yaml")
}

pub fn load(path: &Path) -> Loaded {
    let loaded = match std::fs::read_to_string(path) {
        Ok(text) => parse(&text)
            .unwrap_or_else(|err| Loaded::defaults(Some(format!("{}: {err}", path.display())))),
        Err(err) if err.kind() == ErrorKind::NotFound => Loaded::defaults(None),
        Err(err) => Loaded::defaults(Some(format!("{}: {err}", path.display()))),
    };
    Loaded {
        path: path.display().to_string(),
        ..loaded
    }
}

fn parse(text: &str) -> Result<Loaded, String> {
    let file: File = if text.trim().is_empty() {
        File::default()
    } else {
        serde_norway::from_str(text).map_err(|err| err.to_string())?
    };
    let link = match file.link {
        Some(link) if link.starts_with("http://") || link.starts_with("https://") => {
            link.trim_end_matches('/').to_owned()
        }
        Some(link) => return Err(format!("link must start with http:// or https://: {link}")),
        None => DEFAULT_LINK.into(),
    };
    let monitor = file.monitor.filter(|m| !m.trim().is_empty());
    Ok(Loaded {
        link,
        monitor,
        error: None,
        path: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{DEFAULT_LINK, load, parse};

    #[test]
    fn reads_the_link_and_monitor() {
        let loaded = parse("link: http://192.168.1.10:4730/\nmonitor: JAPANNEXT MNT\n").unwrap();
        assert_eq!(loaded.link, "http://192.168.1.10:4730");
        assert_eq!(loaded.monitor.as_deref(), Some("JAPANNEXT MNT"));
        assert_eq!(loaded.error, None);
    }

    #[test]
    fn an_empty_or_missing_file_means_the_defaults() {
        let empty = parse("").unwrap();
        assert_eq!((empty.link.as_str(), empty.monitor), (DEFAULT_LINK, None));
        let blank_monitor = parse("monitor: ''").unwrap();
        assert_eq!(blank_monitor.monitor, None);
        let missing = load(Path::new("Z:/no/such/windows-deck/config.yaml"));
        assert_eq!((missing.link.as_str(), missing.error), (DEFAULT_LINK, None));
        // The panel names the file to write when it asks for a monitor.
        assert_eq!(missing.path, "Z:/no/such/windows-deck/config.yaml");
    }

    #[test]
    fn rejects_unknown_keys_and_links_that_are_not_http() {
        assert!(parse("lnk: http://127.0.0.1:4730").is_err());
        assert!(parse("link: 127.0.0.1:4730").is_err());
        assert!(parse("link: [").is_err());
    }

    #[test]
    fn a_broken_file_falls_back_to_the_defaults_with_the_reason() {
        let dir = std::env::temp_dir().join(format!("windows-deck-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.yaml");
        std::fs::write(&path, "link: ftp://example\n").unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.link, DEFAULT_LINK);
        assert!(loaded.error.unwrap().contains("link must start with http"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
