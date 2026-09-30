use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

mod commands;
mod error;
mod types;
mod utils;

use commands::agent_policy::*;
use commands::diagnostics::*;
use commands::key_import::*;
use commands::key_insights::*;
use commands::profiles::*;
use commands::repositories::*;
use commands::{app::*, known_hosts::*, launcher::*, ssh_config::*, ssh_keys::*};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Build tray menu
            let show_item = MenuItem::with_id(app, "show", "Show SSH GUI", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            TrayIconBuilder::new()
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/tray-icon.png"
                ))?)
                .icon_as_template(true)
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = app.run_on_main_thread(move || {
                                let _ = window.show();
                                let _ = window.set_focus();
                            });
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = app.run_on_main_thread(move || {
                                let _ = window.show();
                                let _ = window.set_focus();
                            });
                        }
                    }
                })
                .build(app)?;

            // Load preferred terminal from persisted config
            if let Ok(config) = get_app_config() {
                if let Some(term) = config.preferred_terminal {
                    let mut pref = PREFERRED_TERMINAL.lock().unwrap();
                    *pref = Some(term);
                }
            }

            // Start file watcher for ~/.ssh/
            let app_handle = app.handle().clone();
            if let Ok(ssh_dir) = crate::utils::ssh_dir::get_ssh_dir() {
                std::thread::spawn(move || {
                    use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
                    let (tx, rx) = std::sync::mpsc::channel::<DebounceEventResult>();
                    if let Ok(mut debouncer) =
                        new_debouncer(std::time::Duration::from_millis(600), tx)
                    {
                        let _ = debouncer
                            .watcher()
                            .watch(&ssh_dir, notify::RecursiveMode::Recursive);
                        for result in rx {
                            if let Ok(events) = result {
                                let mut changed = false;
                                let mut known_changed = false;
                                for event in events {
                                    let fname = event
                                        .path
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("");
                                    if fname.starts_with('.') || fname.ends_with(".ssh-gui.bak") {
                                        continue;
                                    }
                                    changed = true;
                                    known_changed |= fname == "known_hosts";
                                }
                                if changed {
                                    app_handle.emit("ssh-config-changed", ()).ok();
                                    app_handle.emit("ssh-keys-changed", ()).ok();
                                }
                                if known_changed {
                                    app_handle.emit("known-hosts-changed", ()).ok();
                                }
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide to tray instead of quitting
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            set_repository_roots,
            scan_repositories,
            get_key_usage,
            list_key_metadata,
            save_key_metadata,
            audit_ssh_keys,
            list_agent_enrollments,
            inspect_key_import,
            import_ssh_key,
            restore_public_key,
            diagnose_ssh,
            list_git_profiles,
            save_git_profile,
            delete_git_profile,
            // SSH Config
            get_ssh_config,
            add_host,
            update_host,
            delete_host,
            reorder_hosts,
            // SSH Keys
            list_ssh_keys,
            generate_ssh_key,
            get_public_key,
            delete_ssh_key,
            get_key_fingerprint,
            list_agent_keys,
            add_key_to_agent,
            remove_key_from_agent,
            // Known Hosts
            list_known_hosts,
            delete_known_hosts,
            verify_known_host,
            // Launcher
            launch_ssh_connection,
            get_detected_terminal,
            set_preferred_terminal,
            copy_key_to_server,
            // App
            get_workspace,
            list_backups,
            restore_backup,
            audit_permissions,
            fix_permissions,
            get_app_config,
            save_app_config,
            get_ssh_dir_path,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}

#[cfg(test)]
mod ipc_tests {
    use super::*;
    #[test]
    fn frontend_snake_case_arguments_reach_real_command_validation() {
        let app = tauri::test::mock_builder()
            .invoke_handler(tauri::generate_handler![launch_ssh_connection])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let response = tauri::test::get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: "launch_ssh_connection".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::Json(
                    serde_json::json!({ "host_alias": "-invalid-option" }),
                ),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.into(),
            },
        );
        let error = response
            .err()
            .expect("Invalid destination should be rejected");
        assert!(
            error.to_string().contains("Select a single SSH host alias"),
            "{error}"
        );
    }
    #[test]
    fn all_frontend_algorithm_names_deserialize() {
        for name in ["Ed25519", "Rsa2048", "Rsa4096", "EcdsaP256", "EcdsaP384"] {
            let params: crate::types::KeyGenParams = serde_json::from_value(serde_json::json!({
                "algorithm": name, "comment": "test", "filename": "test", "passphrase": null
            }))
            .unwrap();
            assert_eq!(serde_json::to_value(params.algorithm).unwrap(), name);
        }
    }
}

#[cfg(test)]
mod acceptance;
