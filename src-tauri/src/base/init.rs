use tauri::{Builder, Manager, Runtime,generate_handler};
use crate::base::cmd::*;


pub fn generate_handlers() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static{
    generate_handler![greet]
}


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
        let _ = crate::base::tray::create_tray_icon(app, false);

        app.manage(crate::base::state::AppState::default());
        crate::base::lightweight::add_window_listeners(crate::base::window::schema::WindowType::Main);

        Ok(())
    })
}
