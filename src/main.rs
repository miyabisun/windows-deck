// The release build has no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use windows_deck::{
    app, attach_console, logging, login, pid_to_wait_for,
    platform::{self, Instance},
    wait_for_exit,
};

fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version") {
        attach_console();
        println!("windows-deck {version}");
        return;
    }
    // A shop's login window, opened by the running panel in a process of its own.
    if let Some(at) = args.iter().position(|arg| arg == login::LOGIN) {
        logging::init();
        let value = |at: Option<usize>| at.and_then(|at| args.get(at + 1)).map(String::as_str);
        let shop = value(Some(at)).unwrap_or_default();
        let link = value(args.iter().position(|arg| arg == login::LINK))
            .unwrap_or(windows_deck::config::DEFAULT_LINK);
        std::process::exit(login::run(shop, link));
    }
    if let Some(pid) = pid_to_wait_for(&args) {
        wait_for_exit(pid);
    }
    // Only one panel runs at a time: a later start hands its arguments to the running one.
    if let Instance::Second = platform::single_instance(&args) {
        return;
    }
    app::run(version);
}
