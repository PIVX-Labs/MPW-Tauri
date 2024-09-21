// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use address_index::{pivx_rpc::PIVXRpc, sql_lite::SqlLite, AddressIndex};
use pivx::PIVXDefinition;

mod address_index;
mod binary;
mod error;
mod pivx;

pub const RPC_PORT: u16 = 51473;
pub const RPC_USERNAME: &str = "username";
pub const RPC_PASSWORD: &str = "password";

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
async fn greet() -> Result<String, ()> {
    let pivx_definition = PIVXDefinition;
    let pivx = binary::Binary::new_by_fetching(&pivx_definition)
        .await
        .expect("Failed to run PIVX");

    let mut address_index = AddressIndex::new(
        SqlLite::new(PathBuf::from("/home/duddino/test.sqlite"))
            .await
            .unwrap(),
        PIVXRpc::new(&format!("http://127.0.0.1:{}", RPC_PORT))
            .await
            .unwrap(),
    );
    //tokio::time::sleep(Duration::from_secs(60)).await;
    // Leaking for now to bypass Drop
    Box::leak(Box::new(pivx));
    address_index.sync().await.unwrap();
    Ok("PIVX Started succesfully".into())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
