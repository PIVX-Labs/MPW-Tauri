// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
