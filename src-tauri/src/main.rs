// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod pivx;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
async fn greet() -> String {
    let pivx = pivx::PIVX::new_by_fetching()
        .await
        .expect("Failed to run PIVX");
    // Leaking for now to bypass Drop
    Box::leak(Box::new(pivx));
    "PIVX Started succesfully".into()
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
