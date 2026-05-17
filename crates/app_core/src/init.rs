use tauri::{Builder, Manager, Runtime};

pub fn configure<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    let builder = builder.plugin(tauri_plugin_opener::init());

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = {
        use tauri_plugin_autostart::MacosLauncher;
        builder.plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
    };

    builder.setup(|app| {
        let _ = crate::tray::create_tray_icon(app, false);

        app.manage(crate::state::AppState::default());
        crate::lightweight::add_window_listeners(crate::window::schema::WindowType::Main);

        Ok(())
    })
}
