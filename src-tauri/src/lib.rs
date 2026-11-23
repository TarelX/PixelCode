mod adapters;
mod artifacts;
mod battle_report;
mod camps;
mod conversation;
mod db;
mod git;
mod plans;
mod process_tree;
mod roles;
mod scheduler;
mod settings;
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
            battle_report::export_battle_report,
            battle_report::battle_report_counts,
            camps::open_camp,
            camps::list_recent_camps,
            roles::list_roles,
            roles::list_skills,
            roles::save_role,
            roles::delete_role,
            supervisor::start_role_run,
            conversation::start_conversation,
            scheduler::approve_plan_execution,
            settings::get_locale,
            settings::set_locale,
            artifacts::read_artifact,
            artifacts::list_artifacts,
            plans::latest_plan,
            plans::reject_plan,
            plans::decide_delivery,
            supervisor::stop_role_run
        ])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            process_tree::reap_orphans(&data_dir);
            let connection = db::open(&data_dir)?;
            app.manage(artifacts::ArtifactRoot(data_dir.join("artifacts")));
            app.manage(Mutex::new(connection));
            app.manage(supervisor::RunSlot::default());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<supervisor::RunSlot>().terminate();
            }
        });
}
