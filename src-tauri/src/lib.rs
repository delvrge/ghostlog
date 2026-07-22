//! GHLG native backend: tray, watcher, session storage, Tauri IPC commands.
//! Zero network ports by design — all frontend communication is Tauri IPC,
//! all extension communication will be Native Messaging (stdio).

mod agents;
mod ai;
mod commands;
mod state;
mod storage;
mod tasks;
mod tray;
mod watcher;

use state::AppState;
use std::io::{Read, Write};
use std::path::Path;
use tauri::Manager;

/// Entry point for the `--ghlg-git-commit <repo>` CLI mode (see main.rs).
/// Runs on a small dedicated runtime since this path is a bare subprocess
/// with no Tauri/async runtime already in place.
pub fn capture_from_git_commit_cli(repo: &Path) -> Result<(), String> {
    let canonical = repo.canonicalize().map_err(|e| e.to_string())?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?
        .block_on(storage::capture_from_git_commit(&canonical))
}

/// Entry point for `ghlg --ghlg-task <subcommand> ...` (see main.rs) — lets
/// any external tool, including a coding agent the user is driving (e.g.
/// Claude Code), manage the Kanban board by shelling out to this binary,
/// the same way the git hook already does for capture. Subcommands:
///   list <project>
///   add <project> <title> [--column todo|doing|done] [--tag task|bug|chore|feature] [--description "..."]
///   move <project> <id> <column>
///   delete <project> <id>
/// Prints JSON to stdout on success; errors go to stderr with a non-zero exit.
pub fn task_cli(args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first() else {
        return Err("usage: --ghlg-task <list|add|move|delete> ...".into());
    };
    let rest = &args[1..];

    match subcommand.as_str() {
        "list" => {
            let project = rest.first().ok_or("usage: --ghlg-task list <project>")?;
            let cards = tasks::list_tasks(project)?;
            println!("{}", serde_json::to_string_pretty(&cards).map_err(|e| e.to_string())?);
        }
        "add" => {
            let project = rest.first().ok_or("usage: --ghlg-task add <project> <title> [--column c] [--tag t] [--description d]")?;
            let mut title = None;
            let mut column = "todo".to_string();
            let mut tag = "task".to_string();
            let mut description = String::new();
            let mut i = 1;
            while i < rest.len() {
                match rest[i].as_str() {
                    "--column" => { column = rest.get(i + 1).cloned().unwrap_or(column); i += 2; }
                    "--tag" => { tag = rest.get(i + 1).cloned().unwrap_or(tag); i += 2; }
                    "--description" => { description = rest.get(i + 1).cloned().unwrap_or_default(); i += 2; }
                    other => { title = Some(other.to_string()); i += 1; }
                }
            }
            let title = title.ok_or("missing <title>")?;
            let card = tasks::create_task(project, &title, &description, &tag, &column, "cli")?;
            println!("{}", serde_json::to_string_pretty(&card).map_err(|e| e.to_string())?);
        }
        "move" => {
            let project = rest.first().ok_or("usage: --ghlg-task move <project> <id> <column>")?;
            let id = rest.get(1).ok_or("usage: --ghlg-task move <project> <id> <column>")?;
            let column = rest.get(2).ok_or("usage: --ghlg-task move <project> <id> <column>")?;
            tasks::move_task(project, id, column, "cli")?;
        }
        "delete" => {
            let project = rest.first().ok_or("usage: --ghlg-task delete <project> <id>")?;
            let id = rest.get(1).ok_or("usage: --ghlg-task delete <project> <id>")?;
            tasks::delete_task(project, id)?;
        }
        other => return Err(format!("unknown --ghlg-task subcommand: {other}")),
    }
    Ok(())
}

/// Entry point for the `--ghlg-shell-error <command> <exit_code>` CLI mode
/// (see main.rs), invoked by the shell hook installed via Settings.
pub fn capture_from_shell_error_cli(command: &str, exit_code: &str) -> Result<(), String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?
        .block_on(storage::capture_from_shell_error(command, exit_code))
}

/// Entry point for `ghlg --ghlg-native-host` (see main.rs). Chrome launches
/// this as a short-lived subprocess per `connectNative()` call and speaks
/// its Native Messaging stdio protocol: each message is a 4-byte
/// little-endian length prefix followed by that many bytes of UTF-8 JSON,
/// in both directions. Reads until the extension disconnects (stdin EOF),
/// which is the normal, expected way this process ends.
pub fn run_native_host_cli() -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;

    let stdin = std::io::stdin();
    let mut stdin = stdin.lock();
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    loop {
        let mut len_buf = [0u8; 4];
        if stdin.read_exact(&mut len_buf).is_err() {
            return Ok(()); // extension disconnected — normal shutdown
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut msg_buf = vec![0u8; len];
        stdin.read_exact(&mut msg_buf).map_err(|e| e.to_string())?;

        let msg: serde_json::Value = serde_json::from_slice(&msg_buf).map_err(|e| e.to_string())?;
        let note = msg.get("note").and_then(|v| v.as_str()).map(str::to_string);
        // Present on browser-error captures: a data:image/...;base64 URL of
        // the visible localhost tab at the moment of the error.
        let screenshot = msg.get("screenshot").and_then(|v| v.as_str()).map(str::to_string);

        let result = rt.block_on(storage::capture_from_native_host(note, screenshot));
        let response = match result {
            Ok(()) => serde_json::json!({ "ok": true }),
            Err(e) => serde_json::json!({ "ok": false, "error": e }),
        };
        let response_bytes = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
        stdout
            .write_all(&(response_bytes.len() as u32).to_le_bytes())
            .map_err(|e| e.to_string())?;
        stdout.write_all(&response_bytes).map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())?;
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(AppState::default())
        .setup(|app| {
            tray::init(app.handle())?;
            // Capture data (entries, screenshots) lives in a user-visible
            // folder, not the hidden OS app-data dir the static asset-scope
            // config in tauri.conf.json points at — grant it at runtime
            // (data_root() also handles first-run default + migration off
            // the old hidden location).
            let data_root = storage::data_root()?;
            let _ = app.asset_protocol_scope().allow_directory(&data_root, true);
            // Restore the previously chosen watched folder, if any.
            storage::load_config(app.handle());
            // Re-stamp the git-commit hook in every watched repo with
            // wherever THIS exe actually is. Idempotent, so it's cheap to
            // run on every launch — and it's the only thing that catches a
            // hook left pointing at a previous install location (e.g. a
            // mounted DMG that's since been ejected) before it silently
            // drops every commit capture in that repo.
            {
                let state = app.state::<AppState>();
                let paths = state.watched_paths.lock().unwrap().clone();
                storage::refresh_git_hooks(&paths);
            }
            // Watching starts automatically: combined with launch-at-login
            // the app is "perpetually on" — no start button to remember.
            if let Err(e) = watcher::start(app.handle()) {
                eprintln!("Ghostlog: auto-start watching skipped: {e}");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the review window hides it and keeps the app running
            // in the tray, unless the user turned that off in Settings.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if storage::run_in_background_enabled() {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_watching,
            commands::stop_watching,
            commands::get_watch_state,
            commands::get_last_event,
            commands::add_watched_folder,
            commands::remove_watched_folder,
            commands::get_watched_folders,
            commands::manual_capture,
            commands::list_session_dates,
            commands::list_sessions,
            commands::read_session,
            commands::search_entries,
            commands::update_entry,
            commands::delete_entry,
            commands::delete_session,
            commands::cleanup_empty,
            commands::delete_date,
            commands::is_git_hook_enabled,
            commands::set_git_hook_enabled,
            commands::get_extension_status,
            commands::is_native_host_installed,
            commands::install_native_host,
            commands::uninstall_native_host,
            commands::is_shell_hook_installed,
            commands::install_shell_hook,
            commands::uninstall_shell_hook,
            commands::get_agent_capture_source,
            commands::set_agent_capture_source,
            commands::poll_agent_prompts,
            commands::get_ai_config,
            commands::set_ai_config,
            commands::ai_compile,
            commands::get_output_folder,
            commands::set_output_folder,
            commands::export_document,
            commands::get_run_in_background,
            commands::set_run_in_background,
            commands::get_data_root,
            commands::set_data_root,
            commands::list_tasks,
            commands::list_all_tasks,
            commands::create_task,
            commands::move_task,
            commands::update_task,
            commands::delete_task,
            commands::sync_board_with_ai,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
