pub mod base;


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    base::init::configure(tauri::Builder::default())
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}