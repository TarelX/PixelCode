mod adapters;
mod camps;
mod db;
mod git;
mod roles;
mod scheduler;
mod supervisor;
mod themes;

use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            adapters::detect_clis,
            camps::open_camp,
            camps::list_recent_camps
        ])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let connection = db::open(&data_dir)?;
            app.manage(Mutex::new(connection));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
