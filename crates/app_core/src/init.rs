use tauri::{Builder, Runtime};

pub fn configure<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    let builder = builder.plugin(tauri_plugin_opener::init());
    #[cfg(all(
        feature = "autostart",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    let builder = builder.plugin(tauri_plugin_autostart::Builder::new().build());
    builder.setup(|app| {
        let _ = crate::tray::create_tray_icon(app,false);
        Ok(())
    })
}
