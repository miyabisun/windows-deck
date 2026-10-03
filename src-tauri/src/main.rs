// The release build has no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let context = tauri::generate_context!();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version") {
        windows_deck_lib::attach_console();
        println!("windows-deck {}", context.package_info().version);
        return;
    }
    if let Some(pid) = windows_deck_lib::pid_to_wait_for(&args) {
        windows_deck_lib::wait_for_exit(pid);
    }
    windows_deck_lib::run(context);
}
