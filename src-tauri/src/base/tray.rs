use anyhow::Result;
use tauri::{
    AppHandle, Manager, Runtime, Wry, menu::{CheckMenuItem, Menu, MenuItem}, tray::{MouseButton, TrayIconBuilder, TrayIconEvent}
};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

use super::handle::Handle;

pub fn create_tray_icon<R: Runtime>(app: &tauri::App<R>, visible: bool) -> Result<()> {
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let show_i = MenuItem::with_id(app, "show", if visible { "Hide" } else { "Show" }, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &quit_i])?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = app.handle().plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ));
        let auto_i = CheckMenuItem::with_id(
            app,
            "autostart",
            "AutoStart",
            true,
            app.autolaunch().is_enabled().unwrap_or(false),
            None::<&str>,
        )?;
        // menu = Menu::with_items(app, &[&auto_i,&show_i, &quit_i])?;
        menu.insert_items(&[&auto_i], 0)?;
    }
    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
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

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            "auto" => {
                let autostart_manager = app.autolaunch();
                let currently_enabled = autostart_manager.is_enabled().unwrap_or(false);
                let new_state = if currently_enabled {
                    autostart_manager.disable().is_ok() && false
                } else {
                    autostart_manager.enable().is_ok()
                };
                if let Some(item) = app.menu().and_then(|m| m.get("autostart")) {
                    if let Some(check_item) = item.as_check_menuitem() {
                        let _ = check_item.set_checked(new_state);
                    }
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let is_visible = window.is_visible().unwrap_or(false);
                    if is_visible {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;
    Ok(())
}


pub fn update_menu_visible(visible: bool) {
    let app = Handle::global();
    let app_handle = app.app_handle().unwrap();
    let tray = app_handle.tray_by_id("main").unwrap();
    tray.set_menu(Some(create_tray_menu(&app_handle, visible).unwrap()))
        .unwrap();
}


fn create_tray_menu(app_handle: &AppHandle, visiable: bool) -> Result<Menu<Wry>> {
    let quit_i = MenuItem::with_id(app_handle, "quit", "Quit", true, None::<&str>)?;
    let show_i = MenuItem::with_id(app_handle, "show", if visiable { "Hide" } else { "Show" }, true, None::<&str>)?;
    let menu = Menu::with_items(app_handle, &[&show_i, &quit_i])?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = app_handle.plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ));
        let auto_i = CheckMenuItem::with_id(
            app_handle,
            "autostart",
            "AutoStart",
            true,
            app_handle.autolaunch().is_enabled().unwrap_or(false),
            None::<&str>,
        )?;
        // menu = Menu::with_items(app, &[&auto_i,&show_i, &quit_i])?;
        menu.insert_items(&[&auto_i], 0)?;
    }
    Ok(menu)
}