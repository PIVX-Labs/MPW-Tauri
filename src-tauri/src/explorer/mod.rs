use crate::address_index::block_file_source::BlockFileSource;
use crate::error::PIVXErrors;
use async_compression::tokio::bufread::GzipDecoder;
use futures::TryStreamExt;
use jsonrpsee::rpc_params;
use read_progress_stream::{ProgressHandler, ReadProgressStream};
use serde::Deserialize;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{ProcessesToUpdate, Signal, System};
use tokio::fs::File;
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncSeekExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::{OnceCell, RwLock};
use tokio::time::sleep;
use tokio_util::io::StreamReader;

use crate::address_index::{
    database::Database, pivx_rpc::PIVXRpc, sql_lite::SqlLite, types::Vin, AddressIndex,
};
use crate::binary::Binary;
use crate::{PIVXDefinition, RPC_PORT};
use global_function_macro::generate_global_functions;

type TxHexWithBlockCount = (String, u64, u64);

#[derive(PartialEq)]
enum ExplorerState {
    DownloadingCheckpoint,
    SyncingBlocks,
    IndexingBlocks,
    Synced,
}

#[derive(Clone)]
pub struct Explorer<D>
where
    D: Database,
{
    address_index: Arc<RwLock<AddressIndex<D>>>,
    pivx_rpc: Arc<RwLock<Option<PIVXRpc>>>,
    indexed_blocks: Arc<RwLock<u64>>,
    state: Arc<RwLock<ExplorerState>>,
    checkpoint_download_progress: Arc<RwLock<f64>>,
}

#[derive(Deserialize)]
struct ChainInfo {
    verificationprogress: f64,
    initial_block_downloading: bool,
}

type DefaultExplorer = Explorer<SqlLite>;

impl<D> Explorer<D>
where
    D: Database + Send + Clone,
{
    fn new(address_index: AddressIndex<D>, rpc: Option<PIVXRpc>) -> Self {
        let indexed_blocks = address_index.indexed_blocks.clone();
        Self {
            address_index: Arc::new(RwLock::new(address_index)),
            pivx_rpc: Arc::new(RwLock::new(rpc)),
            indexed_blocks,
            state: Arc::new(RwLock::new(ExplorerState::DownloadingCheckpoint)),
            checkpoint_download_progress: Arc::new(RwLock::new(0.0)),
        }
    }
}

static EXPLORER: OnceCell<DefaultExplorer> = OnceCell::const_new();
static PIVX_RPC: OnceCell<PIVXRpc> = OnceCell::const_new();
// If more than `LAST_BLOCK_GAP` are left to sync, prefer BlockFileSource
const LAST_BLOCK_GAP: u64 = 10_000;
const CHECKPOINT_URL: &'static str = "https://snapshot.rockdev.org/PIVXsnapshotLatest.tgz";

pub fn kill_running_pivxd(wait: bool) -> crate::error::Result<usize> {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let mut killed = 0;

    for (_pid, process) in system.processes() {
        let name = process.name();
        if name == "pivxd" {
            if wait {
                if let Ok(_) = process.kill_with_and_wait(Signal::Term) {
                    killed += 1;
                }
            } else {
                process.kill_with(Signal::Term);
                killed += 1;
            }
        }
    }

    Ok(killed)
}

async fn get_pivx_rpc() -> &'static PIVXRpc {
    PIVX_RPC
        .get_or_init(|| async {
            loop {
                let pivx_definition = PIVXDefinition;
                let mut pivx = Binary::new_by_fetching(&pivx_definition)
                    .await
                    .expect("Failed to run PIVX");
                let result = pivx.wait_for_load(&pivx_definition).await;
                match result {
                    Err(PIVXErrors::PivxdAlreadyRunning) => {
                        if let Ok(killed) = kill_running_pivxd(true) {
                            if killed == 0 {
                                panic!("Lock in .pivx folder, but no daemon is running")
                            }
                            continue;
                        } else {
                            panic!("Failed to kill pivx process")
                        }
                    }
                    Err(e) => {
                        panic!("{}", e)
                    }
                    Ok(()) => {}
                }
                return PIVXRpc::new(&format!("http://127.0.0.1:{}", RPC_PORT), pivx)
                    .await
                    .unwrap();
            }
        })
        .await
}

async fn download_checkpoint(
    data_dir: &Path,
    mut progress: Box<dyn FnMut(f64) + Send + Sync + 'static>,
) -> crate::error::Result<()> {
    println!("Downloading checkpoint");
    let mut request = reqwest::get(CHECKPOINT_URL).await?;
    if !request.status().is_success() {
        return Err(PIVXErrors::ServerError);
    }
    // Default to 20GB if there is no content length
    let content_length = request.content_length().unwrap_or(20_000_000_000.0);
    for name in ["blocks", "chainstate", "sporks", "zerocoin"] {
        let p = data_dir.join(name);
        if p.is_dir() {
            tokio::fs::remove_dir_all(p).await?;
        }
    }

    for name in ["banlist.dat", "peers.dat"] {
        let p = data_dir.join(name);
        if p.is_file() {
            tokio::fs::remove_file(p).await?;
        }
    }

    let reader = StreamReader::new(ReadProgressStream::new(
        request
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)),
        Box::new(move |bytes_read, _| {
            progress((bytes_read as f64) / (content_length as f64));
        }),
    ));

    let gzip = GzipDecoder::new(reader);

    let mut archive = tokio_tar::Archive::new(gzip);
    std::fs::create_dir_all(data_dir)?;
    archive.unpack(data_dir).await?;
    Ok(())
}

async fn get_explorer() -> &'static DefaultExplorer {
    EXPLORER
        .get_or_init(|| async {
            let dir = dirs::data_dir()
                .ok_or(PIVXErrors::NoDataDir)
                .unwrap()
                .join("pivx-rust");

            let block_file_source = BlockFileSource::new(&dir.join(".pivx").join("blocks"));

            let address_index = AddressIndex::new(
                SqlLite::new(dir.join("test.sqlite")).await.unwrap(),
                block_file_source,
            );

            let explorer = Explorer::new(address_index, None);
            // Cloning is very cheap, it's just a Pathbuf and some Arcs
            let explorer_clone = explorer.clone();
            tokio::spawn(async move {
                *explorer_clone.state.write().await = ExplorerState::DownloadingCheckpoint;
                let explorer = explorer_clone.clone();
                download_checkpoint(
                    &dir.join(".pivx"),
                    Box::new(move |progress| {
                        let explorer = explorer.clone();
                        tokio::spawn(async move {
                            *explorer.checkpoint_download_progress.write().await += progress;
                        });
                    }),
                )
                .await
                .ok();
                let pivx_rpc = get_pivx_rpc().await;
                explorer_clone
                    .pivx_rpc
                    .write()
                    .await
                    .replace(pivx_rpc.clone());
                if let Ok(true) = explorer_clone.is_initial_sync().await {
                    *explorer_clone.state.write().await = ExplorerState::SyncingBlocks;
                }
                while match explorer_clone.is_initial_sync().await {
                    Ok(is_initial_sync) => is_initial_sync,
                    Err(_) => true,
                } {}
                *explorer_clone.state.write().await = ExplorerState::IndexingBlocks;

                if let Err(err) = explorer_clone.sync().await {
                    eprintln!("Warning: Syncing failed with error {}", err);
                }
                *explorer_clone.state.write().await = ExplorerState::Synced;
            });

            explorer
        })
        .await
}

#[generate_global_functions]
impl<D> Explorer<D>
where
    D: Database + Send + Clone,
{
    pub async fn get_block(&self, block_height: u64) -> crate::error::Result<String> {
        let rpc = self.pivx_rpc.read().await;
        let rpc = rpc.as_ref().ok_or(PIVXErrors::PivxdNotRunning)?;
        let block_hash: String = rpc.call("getblockhash", rpc_params![block_height]).await?;
        let json: serde_json::Value = rpc.call("getblock", rpc_params![block_hash, 2]).await?;
        Ok(json.to_string())
    }

    pub async fn get_block_count(&self) -> crate::error::Result<u64> {
        self.pivx_rpc
            .read()
            .await
            .as_ref()
            .ok_or(PIVXErrors::PivxdNotRunning)?
            .call("getblockcount", rpc_params![])
            .await
    }

    /// Gets all raw transactions containing one of `address`
    pub async fn get_txs(
        &self,
        addresses: Vec<&str>,
    ) -> crate::error::Result<Vec<TxHexWithBlockCount>> {
        let mut txs = vec![];
        for address in addresses {
            for txid in self
                .address_index
                .read()
                .await
                .get_address_txids(address)
                .await?
            {
                if let Ok(tx) = self.get_transaction(&txid).await {
                    txs.push(tx);
                }
            }
        }
        Ok(txs)
    }

    pub async fn get_tx_from_vin(
        &self,
        vin: Vin,
    ) -> crate::error::Result<Option<TxHexWithBlockCount>> {
        let txid = self
            .address_index
            .read()
            .await
            .get_txid_from_vin(&vin)
            .await?;
        if let Some(txid) = txid {
            Ok(self.get_transaction(&txid).await.ok())
        } else {
            Ok(None)
        }
    }

    /// Gets raw transaction in hex format
    pub async fn get_transaction(&self, txid: &str) -> crate::error::Result<TxHexWithBlockCount> {
        use serde::Deserialize;
        #[derive(Deserialize)]
        struct TxResponse {
            hex: String,
            blockhash: String,
            confirmations: u64,
        }
        #[derive(Deserialize)]
        struct BlockResponse {
            height: u64,
            time: u64,
        }
        let rpc = self.pivx_rpc.read().await;
        let rpc = rpc.as_ref().ok_or(PIVXErrors::PivxdNotFound)?;

        let TxResponse {
            hex,
            blockhash,
            confirmations,
        } = rpc
            .call("getrawtransaction", rpc_params![txid, true])
            .await?;
        if confirmations == 0 {
            return Err(PIVXErrors::InvalidResponse);
        }
        let BlockResponse { height, time } = rpc.call("getblock", rpc_params![blockhash]).await?;
        Ok((hex, height, time))
    }

    pub async fn send_transaction(&self, transaction: &str) -> crate::error::Result<String> {
        self.pivx_rpc
            .read()
            .await
            .as_ref()
            .ok_or(PIVXErrors::PivxdNotRunning)?
            .call("sendrawtransaction", rpc_params![transaction])
            .await
    }

    pub async fn sync(&self) -> crate::error::Result<()> {
        let current_block = self.get_block_count().await?;
        let last_indexed_block = self
            .address_index
            .read()
            .await
            .get_last_indexed_block()
            .await?;

        if current_block - last_indexed_block >= LAST_BLOCK_GAP {
            self.switch_to_blockfile_source().await?;
        }

        self.address_index.write().await.sync().await?;
        self.address_index
            .write()
            .await
            // Leave 100 blocks as buffer
            .update_block_count(self.get_block_count().await? - 100)
            .await?;
        self.switch_to_rpc_source().await?;
        *self.state.write().await = ExplorerState::Synced;
        loop {
            sleep(Duration::from_secs(60)).await;
            self.address_index.write().await.sync().await?;
        }
    }

    pub async fn switch_to_rpc_source(&self) -> crate::error::Result<()> {
        self.address_index
            .write()
            .await
            .set_block_source(get_pivx_rpc().await.clone());
        Ok(())
    }

    pub async fn switch_to_blockfile_source(&self) -> crate::error::Result<()> {
        let dir = dirs::data_dir()
            .ok_or(PIVXErrors::NoDataDir)?
            .join("pivx-rust")
            .join(".pivx")
            .join("blocks");
        let block_file_source = BlockFileSource::new(dir);
        self.address_index
            .write()
            .await
            .set_block_source(block_file_source);
        Ok(())
    }

    pub async fn is_initial_sync(&self) -> crate::error::Result<bool> {
        let chain_info: ChainInfo = self
            .pivx_rpc
            .read()
            .await
            .as_ref()
            .ok_or(PIVXErrors::PivxdNotRunning)?
            .call("getblockchaininfo", rpc_params![])
            .await?;
        Ok(chain_info.initial_block_downloading)
    }

    pub async fn get_sync_progress(&self) -> crate::error::Result<f64> {
        let chain_info: ChainInfo = self
            .pivx_rpc
            .read()
            .await
            .as_ref()
            .ok_or(PIVXErrors::PivxdNotRunning)?
            .call("getblockchaininfo", rpc_params![])
            .await?;
        Ok(chain_info.verificationprogress)
    }

    pub async fn get_index_progress(&self) -> crate::error::Result<f64> {
        Ok((*self.indexed_blocks.read().await as f64) / (self.get_block_count().await? as f64))
    }

    pub async fn is_downloading_checkpoint(&self) -> crate::error::Result<bool> {
        Ok(*self.state.read().await == ExplorerState::DownloadingCheckpoint)
    }

    pub async fn get_checkpoint_download_progress(&self) -> crate::error::Result<f64> {
        Ok(*self.checkpoint_download_progress.read().await)
    }

    pub async fn index_is_done(&self) -> crate::error::Result<bool> {
        Ok(*self.state.read().await == ExplorerState::Synced)
    }
}
