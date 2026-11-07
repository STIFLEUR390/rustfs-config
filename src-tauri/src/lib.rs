#[cfg(feature = "gui")]
mod window_effects;

pub mod admin;
pub mod backend;
#[cfg(feature = "gui")]
pub mod commands;
pub mod config;
pub mod error;
pub mod s3;

#[cfg(feature = "gui")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .invoke_handler(tauri::generate_handler![
            commands::test_connection,
            commands::list_buckets,
            commands::head_bucket,
            commands::list_objects,
            commands::put_object,
            commands::get_object,
            commands::delete_object,
            commands::list_users,
            commands::get_user,
            commands::add_user,
            commands::set_user_status,
            commands::remove_user,
            commands::list_policies,
            commands::get_policy,
            commands::add_policy,
            commands::remove_policy,
            commands::attach_policy,
            commands::detach_policy,
            commands::policy_entities,
        ]);

    #[cfg(desktop)] let builder = builder.plugin(tauri_plugin_decorum::init()); // decorum (1/2)

    builder
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Desktop window effects: comment out a line to disable it.
            #[cfg(desktop)]
            {
                use tauri::Manager;
                let main = app.get_webview_window("main").unwrap();
                window_effects::decorum(&main)?; // decorum (2/2)
                // window_effects::vibrancy(&main)?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
