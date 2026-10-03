//! The panel window: configuration for the page and filling the chosen monitor.

pub mod config;

use tauri::WebviewWindow;

#[tauri::command]
fn deck_config() -> config::Loaded {
    config::load(&config::default_path())
}

/// Index of the monitor whose display name (`\\.\DISPLAYn`) is `wanted`.
fn pick(names: &[Option<String>], wanted: Option<&str>) -> Option<usize> {
    let wanted = wanted?;
    names.iter().position(|name| {
        name.as_deref()
            .is_some_and(|n| n.eq_ignore_ascii_case(wanted))
    })
}

/// Fill `monitor` (a display name), or the primary monitor when it is `None` or not
/// connected, and show the window. Returns the display name used.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri hands command arguments over by value.
fn place_window(window: WebviewWindow, monitor: Option<String>) -> Result<String, String> {
    let fail = |err: tauri::Error| err.to_string();
    let monitors = window.available_monitors().map_err(fail)?;
    let names: Vec<Option<String>> = monitors.iter().map(|m| m.name().cloned()).collect();
    let target = match pick(&names, monitor.as_deref()) {
        Some(index) => Some(monitors[index].clone()),
        None => window.primary_monitor().map_err(fail)?,
    };
    if let Some(target) = &target {
        // Fullscreen fills the monitor the window is on, so move there first.
        window.set_fullscreen(false).map_err(fail)?;
        window.set_position(*target.position()).map_err(fail)?;
        window.set_fullscreen(true).map_err(fail)?;
    }
    window.show().map_err(fail)?;
    window.set_focus().map_err(fail)?;
    Ok(target.and_then(|m| m.name().cloned()).unwrap_or_default())
}

/// This window's handle, which windows-link pins to every virtual desktop.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri hands command arguments over by value.
fn window_handle(window: WebviewWindow) -> Result<isize, String> {
    window
        .hwnd()
        .map(|hwnd| hwnd.0 as isize)
        .map_err(|err| err.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            deck_config,
            place_window,
            window_handle
        ])
        .run(tauri::generate_context!())
        .expect("windows-deck failed to start");
}

#[cfg(test)]
mod tests {
    use super::pick;

    #[test]
    fn picks_a_monitor_by_display_name_ignoring_case() {
        let names = [
            Some(r"\\.\DISPLAY1".to_owned()),
            None,
            Some(r"\\.\DISPLAY2".to_owned()),
        ];
        assert_eq!(pick(&names, Some(r"\\.\display2")), Some(2));
        assert_eq!(pick(&names, Some(r"\\.\DISPLAY9")), None);
        assert_eq!(pick(&names, None), None);
    }
}
