#[cfg(feature = "gui")]
mod window_effects;

pub mod admin;
pub mod backend;
pub mod browser;
#[cfg(feature = "gui")]
pub mod commands;
pub mod config;
pub mod error;
pub mod iam;
pub mod instances;
pub mod lifecycle;
pub mod policy;
pub mod qualify;
pub mod s3;
pub mod secrets;
pub mod store;
pub mod verify;

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
            commands::list_objects_page,
            commands::create_bucket,
            commands::delete_bucket,
            commands::put_object,
            commands::put_object_from_path,
            commands::get_object,
            commands::get_object_to_path,
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
            commands::list_instances,
            commands::test_instance,
            commands::save_instance,
            commands::delete_instance,
            commands::list_instance_buckets,
            commands::list_instance_objects,
            commands::create_instance_bucket,
            commands::delete_instance_bucket,
            commands::upload_instance_object,
            commands::download_instance_object,
            commands::delete_instance_object,
            commands::list_credentials,
            commands::create_credential,
            commands::adopt_credential,
            commands::rotate_credential,
            commands::revoke_credential,
            commands::verify_isolation,
            commands::bucket_state,
            commands::plan_bucket_policy,
            commands::apply_bucket_policy,
            commands::plan_bucket_config,
            commands::apply_bucket_config,
        ]);

    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_decorum::init()); // decorum (1/2)

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
