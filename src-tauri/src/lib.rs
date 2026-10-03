//! The panel window: configuration for the page, filling the chosen monitor, and
//! automatic updates.

pub mod config;
pub mod logging;
pub mod update;

use std::{sync::Arc, time::Duration};

use tauri::{Manager, WebviewWindow};
use tracing::info;
use update::{Updater, swap};

/// Run with this argument while the panel is running to make it check for an update now.
pub const CHECK_UPDATE: &str = "--check-update";

/// The process id after `--wait-for-pid`, which an update passes to the new process.
pub fn pid_to_wait_for(args: &[String]) -> Option<u32> {
    let at = args.iter().position(|arg| arg == swap::WAIT_FOR_PID)?;
    args.get(at + 1)?.parse().ok()
}

/// Wait (up to 15 seconds) for the previous panel to exit after an update.
pub fn wait_for_exit(pid: u32) {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    };
    let timeout = u32::try_from(Duration::from_secs(15).as_millis()).unwrap_or(u32::MAX);
    unsafe {
        let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if !process.is_null() {
            WaitForSingleObject(process, timeout);
            CloseHandle(process);
        }
    }
}

/// Reattach to the launching terminal so `--version` is visible in the release build.
pub fn attach_console() {
    if !cfg!(debug_assertions) {
        unsafe {
            windows_sys::Win32::System::Console::AttachConsole(
                windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS,
            );
        }
    }
}

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
    // The window of a freshly updated panel is up: the previous exe is no longer needed.
    if let Some(exe) = window.state::<Arc<Updater>>().installed_exe() {
        swap::remove_old_later(exe);
    }
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

pub fn run(context: tauri::Context) {
    logging::init();
    let version = context.package_info().version.to_string();
    info!(%version, "starting");
    let updater = Arc::new(Updater::new(&version));
    tauri::Builder::default()
        // A second start hands its arguments to the running panel and exits.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if argv.iter().any(|arg| arg == CHECK_UPDATE) {
                let updater = app.state::<Arc<Updater>>().inner().clone();
                std::thread::spawn(move || updater.check_and_apply());
            } else if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .manage(updater.clone())
        .setup(move |_| {
            update::run_periodically(updater);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            deck_config,
            place_window,
            window_handle
        ])
        .run(context)
        .expect("windows-deck failed to start");
}

#[cfg(test)]
mod tests {
    use super::{pick, pid_to_wait_for};

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

    #[test]
    fn reads_the_process_to_wait_for() {
        let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(pid_to_wait_for(&args(&["--wait-for-pid", "42"])), Some(42));
        assert_eq!(pid_to_wait_for(&args(&["--wait-for-pid"])), None);
        assert_eq!(pid_to_wait_for(&args(&["--wait-for-pid", "x"])), None);
        assert_eq!(pid_to_wait_for(&args(&[])), None);
    }
}
