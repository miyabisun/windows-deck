// The release build has no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {}
