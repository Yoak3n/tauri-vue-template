pub mod base;

use base::init;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init::configure(tauri::Builder::default())
        .invoke_handler(init::generate_handlers())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
