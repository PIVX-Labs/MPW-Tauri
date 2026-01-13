// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "full-node")]
use crate::explorer::kill_running_pivxd;

#[cfg(feature = "full-node")]
use pivx::PIVXDefinition;

mod error;

#[cfg(feature = "full-node")]
mod address_index;
#[cfg(feature = "full-node")]
mod binary;
#[cfg(feature = "full-node")]
mod explorer;
#[cfg(feature = "full-node")]
mod pivx;

pub const RPC_PORT: u16 = 51473;
pub const RPC_USERNAME: &str = "username";
pub const RPC_PASSWORD: &str = "password";

#[tauri::command]
fn is_full_node() -> bool {
    cfg!(feature = "full-node")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(feature = "full-node")]
    use explorer::auto_generated::*;

    let tauri = tauri::Builder::default().plugin(tauri_plugin_shell::init());
    let tauri = tauri.invoke_handler(tauri::generate_handler![is_full_node]);
    #[cfg(feature = "full-node")]
    let tauri = tauri.invoke_handler(tauri::generate_handler![
	is_full_node,
        explorer_get_block,
        explorer_get_block_count,
        explorer_get_txs,
        explorer_get_transaction,
        explorer_send_transaction,
        explorer_get_tx_from_vin,
        explorer_sync,
        explorer_switch_to_rpc_source,
        explorer_switch_to_blockfile_source,
        explorer_is_initial_sync,
        explorer_get_sync_progress,
        explorer_get_index_progress,
        explorer_index_is_done,
        explorer_is_downloading_checkpoint,
        explorer_get_checkpoint_download_progress,
        explorer_get_ntp_date,
    ]);
    tauri
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                if window.label() == "main" {
                    #[cfg(feature = "full-node")]
                    kill_running_pivxd(false).ok();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
