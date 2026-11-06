//! Logs go to `%LOCALAPPDATA%\windows-deck\windows-deck.log` in the release build (it has
//! no console) and to stderr in development.

use std::{fs::OpenOptions, path::PathBuf, sync::Mutex};

use tracing_subscriber::filter::LevelFilter;

pub fn log_file() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map_or_else(|| PathBuf::from("."), PathBuf::from);
    base.join("windows-deck").join("windows-deck.log")
}

pub fn init() {
    let builder = tracing_subscriber::fmt().with_max_level(LevelFilter::INFO);
    if cfg!(debug_assertions) {
        builder.with_writer(std::io::stderr).init();
        return;
    }
    let path = log_file();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => builder
            .with_ansi(false)
            .with_writer(Mutex::new(file))
            .init(),
        Err(_) => builder.with_writer(std::io::stderr).init(),
    }
}
