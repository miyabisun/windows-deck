//! A Stream Deck style touch panel for windows-link's buttons, drawn with GPUI.

pub mod config;
pub mod link;
pub mod logging;
pub mod model;
pub mod update;

use std::time::Duration;

/// Run with this argument while the panel is running to make it check for an update now.
pub const CHECK_UPDATE: &str = "--check-update";

/// The process id after `--wait-for-pid`, which an update passes to the new process.
pub fn pid_to_wait_for(args: &[String]) -> Option<u32> {
    let at = args
        .iter()
        .position(|arg| arg == update::swap::WAIT_FOR_PID)?;
    args.get(at + 1)?.parse().ok()
}

/// Wait (up to 15 seconds) for the previous panel to exit after an update.
pub fn wait_for_exit(pid: u32) {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    };
    let timeout = u32::try_from(Duration::from_secs(15).as_millis()).unwrap_or(u32::MAX);
    // SAFETY: the process handle is used only when `OpenProcess` returned one and is closed exactly
    // once after the wait.
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
        // SAFETY: AttachConsole only takes the predefined parent-process id.
        unsafe {
            windows_sys::Win32::System::Console::AttachConsole(
                windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::pid_to_wait_for;

    #[test]
    fn reads_the_process_to_wait_for() {
        let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(pid_to_wait_for(&args(&["--wait-for-pid", "42"])), Some(42));
        assert_eq!(pid_to_wait_for(&args(&["--wait-for-pid"])), None);
        assert_eq!(pid_to_wait_for(&args(&["--wait-for-pid", "x"])), None);
        assert_eq!(pid_to_wait_for(&args(&[])), None);
    }
}
