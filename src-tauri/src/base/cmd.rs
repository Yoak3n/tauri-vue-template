// Tauri command handlers — define `#[tauri::command]` functions here.

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}