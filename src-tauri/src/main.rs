// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use address_index::{block_file_source::BlockFileSource, sql_lite::SqlLite, AddressIndex};
use pivx::PIVXDefinition;

mod address_index;
mod binary;
mod error;
mod explorer;
mod pivx;

pub const RPC_PORT: u16 = 51473;
pub const RPC_USERNAME: &str = "username";
pub const RPC_PASSWORD: &str = "password";

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
async fn greet() -> Result<String, ()> {
    let _pivx_definition = PIVXDefinition;
    /*let pivx = binary::Binary::new_by_fetching(&pivx_definition)
    .await
    .expect("Failed to run PIVX");*/
    let now = tokio::time::Instant::now();
    let mut address_index = AddressIndex::new(
        SqlLite::new(PathBuf::from("/home/duddino/test.sqlite"))
            .await
            .unwrap(),
        /*PIVXRpc::new(&format!("http://127.0.0.1:{}", RPC_PORT))
            .await
        .unwrap(),*/
        BlockFileSource::new("/home/duddino/.local/share/pivx-rust/.pivx/blocks/"),
    );
    //tokio::time::sleep(Duration::from_secs(60)).await;
    // Leaking for now to bypass Drop
    //Box::leak(Box::new(pivx));
    address_index.sync().await.unwrap();
    println!("elapsed {:?}", now.elapsed());
    Ok("PIVX Started succesfully".into())
}

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod test {
    use super::*;
    // Uncomment to start manual testing without having to run MPW
    // #[tokio::test]
    #[allow(unused)]
    async fn main_test() {
        greet().await.unwrap();
    }
}
