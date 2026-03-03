use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

mod commands;
mod error;
mod types;
mod utils;

use commands::{app::*, known_hosts::*, launcher::*, ssh_config::*, ssh_keys::*};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // Build tray menu
            let show_item = MenuItem::with_id(app, "show", "Show SSH GUI", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
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
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Open DevTools automatically in debug builds to surface JS errors
            #[cfg(debug_assertions)]
            if let Some(win) = app.get_webview_window("main") {
                win.open_devtools();
            }

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
                    if let Ok(mut debouncer) = new_debouncer(
                        std::time::Duration::from_millis(600), tx
                    ) {
                        let _ = debouncer.watcher().watch(
                            &ssh_dir, notify::RecursiveMode::NonRecursive
                        );
                        for result in rx {
                            if let Ok(events) = result {
                                for event in events {
                                    let fname = event.path.file_name()
                                        .and_then(|n| n.to_str()).unwrap_or("");
                                    match fname {
                                        "config" => { app_handle.emit("ssh-config-changed", ()).ok(); }
                                        "known_hosts" => { app_handle.emit("known-hosts-changed", ()).ok(); }
                                        n if !n.ends_with(".pub") && !n.starts_with('.') => {
                                            app_handle.emit("ssh-keys-changed", ()).ok();
                                        }
                                        _ => {}
                                    }
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
            delete_known_host,
            delete_known_host_by_hostname,
            verify_known_host,
            // Launcher
            launch_ssh_connection,
            get_detected_terminal,
            set_preferred_terminal,
            copy_key_to_server,
            // App
            get_app_config,
            save_app_config,
            get_ssh_dir_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
