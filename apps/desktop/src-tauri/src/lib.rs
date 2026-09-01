//! Tauri 2 desktop shell for FPS.
//!
//! This is not a remote-website wrapper. `cargo tauri dev` loads the shared
//! Vite UI from `apps/web` (`http://127.0.0.1:47880`). Production bundles
//! `apps/web/dist`. Session tokens are stored via [`vault`].

mod vault;

use std::path::PathBuf;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::vault::Vault;

fn vault_from_app(app: &AppHandle) -> Result<Vault, String> {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("fps");
    Ok(Vault::new(dir))
}

#[tauri::command]
fn vault_store_session(app: AppHandle, token: String) -> Result<(), String> {
    vault_from_app(&app)?
        .store_session_token(&token)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn vault_load_session(app: AppHandle) -> Result<Option<String>, String> {
    vault_from_app(&app)?
        .load_session_token()
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn vault_delete_session(app: AppHandle) -> Result<(), String> {
    vault_from_app(&app)?
        .delete_session_token()
        .map_err(|e| e.to_string())
}

fn install_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("FPS")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                app.exit(0);
            }
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
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
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    let _tray = builder.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if let Err(err) = install_tray(app) {
                // Tray APIs are stubbed so the window still opens if the host
                // lacks a working indicator stack.
                eprintln!("desktop tray stub skipped: {err}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            vault_store_session,
            vault_load_session,
            vault_delete_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running FPS desktop");
}
