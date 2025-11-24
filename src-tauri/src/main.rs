// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::explorer::kill_running_pivxd;
use pivx::PIVXDefinition;

mod address_index;
mod binary;
mod error;
mod explorer;
mod pivx;

pub const RPC_PORT: u16 = 51473;
pub const RPC_USERNAME: &str = "username";
pub const RPC_PASSWORD: &str = "password";

fn main() {
    use explorer::auto_generated::*;

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
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
        ])
        .on_window_event(|event| {
            if let tauri::WindowEvent::Destroyed = event.event() {
                let window = event.window();
                if window.label() == "main" {
                    kill_running_pivxd(false).ok();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
