// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

fn main() {
    // Lightweight CLI mode used by the git post-commit hook: capture and
    // exit immediately, without launching the tray/GUI process. This lets
    // git-commit capture work even when GHLG isn't already running.
    let mut args = std::env::args().skip(1);
    if let Some(flag) = args.next() {
        if flag == "--ghlg-git-commit" {
            let repo = args.next().unwrap_or_else(|| ".".to_string());
            if let Err(e) = ghlg_lib::capture_from_git_commit_cli(&PathBuf::from(repo)) {
                eprintln!("GHLG git-commit capture failed: {e}");
                std::process::exit(1);
            }
            return;
        }
        if flag == "--ghlg-native-host" {
            if let Err(e) = ghlg_lib::run_native_host_cli() {
                eprintln!("GHLG native host failed: {e}");
                std::process::exit(1);
            }
            return;
        }
        if flag == "--ghlg-shell-error" {
            let command = args.next().unwrap_or_default();
            let exit_code = args.next().unwrap_or_default();
            if let Err(e) = ghlg_lib::capture_from_shell_error_cli(&command, &exit_code) {
                eprintln!("GHLG shell-error capture failed: {e}");
                std::process::exit(1);
            }
            return;
        }
    }

    ghlg_lib::run()
}
