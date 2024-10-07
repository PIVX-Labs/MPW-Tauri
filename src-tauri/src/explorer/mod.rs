use crate::error::PIVXErrors;
use jsonrpsee::rpc_params;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::sync::OnceCell;

use crate::address_index::{
    database::Database, pivx_rpc::PIVXRpc, sql_lite::SqlLite, types::Vin, AddressIndex,
};
use crate::binary::Binary;
use crate::{PIVXDefinition, RPC_PORT};
use global_function_macro::generate_global_functions;

type TxHexWithBlockCount = (String, u64, u64);

#[derive(Clone)]
pub struct Explorer<D>
where
    D: Database,
{
    address_index: AddressIndex<D>,
    pivx_rpc: PIVXRpc,
}

type DefaultExplorer = Explorer<SqlLite>;

impl<D> Explorer<D>
where
    D: Database + Send + Clone,
{
    fn new(address_index: AddressIndex<D>, rpc: PIVXRpc) -> Self {
        Self {
            address_index,
            pivx_rpc: rpc,
        }
    }
}

static EXPLORER: OnceCell<DefaultExplorer> = OnceCell::const_new();

async fn get_explorer() -> &'static DefaultExplorer {
    EXPLORER
        .get_or_init(|| async {
            let pivx_definition = PIVXDefinition;
            let mut pivx = Binary::new_by_fetching(&pivx_definition)
                .await
                .expect("Failed to run PIVX");
            pivx.wait_for_load(&pivx_definition).await.unwrap();
            let pivx_rpc = PIVXRpc::new(&format!("http://127.0.0.1:{}", RPC_PORT), pivx)
                .await
                .unwrap();
            // FIXME: refactor this to accept HOME
            let address_index = AddressIndex::new(
                SqlLite::new(PathBuf::from("/home/duddino/test.sqlite"))
                    .await
                    .unwrap(),
                pivx_rpc.clone(),
            );

            let explorer = Explorer::new(address_index, pivx_rpc);
            // Cloning is very cheap, it's just a Pathbuf and some Arcs
            let explorer_clone = explorer.clone();
            tokio::spawn(async move {
                if let Err(err) = explorer_clone.sync().await {
                    eprintln!("Warning: Syncing failed with error {}", err);
                }
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
        let block_hash: String = self
            .pivx_rpc
            .call("getblockhash", rpc_params![block_height])
            .await?;
        let json: serde_json::Value = self
            .pivx_rpc
            .call("getblock", rpc_params![block_hash, 2])
            .await?;
        Ok(json.to_string())
    }

    pub async fn get_block_count(&self) -> crate::error::Result<u64> {
        self.pivx_rpc.call("getblockcount", rpc_params![]).await
    }

    /// Gets all raw transactions containing one of `address`
    pub async fn get_txs(
        &self,
        addresses: Vec<&str>,
    ) -> crate::error::Result<Vec<TxHexWithBlockCount>> {
        let mut txs = vec![];
        for address in addresses {
            for txid in self.address_index.get_address_txids(address).await? {
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
        let txid = self.address_index.get_txid_from_vin(&vin).await?;
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

        let TxResponse {
            hex,
            blockhash,
            confirmations,
        } = self
            .pivx_rpc
            .call("getrawtransaction", rpc_params![txid, true])
            .await?;
        if confirmations == 0 {
            return Err(PIVXErrors::InvalidResponse);
        }
        let BlockResponse { height, time } = self
            .pivx_rpc
            .call("getblock", rpc_params![blockhash])
            .await?;
        Ok((hex, height, time))
    }

    pub async fn send_transaction(&self, transaction: &str) -> crate::error::Result<String> {
        self.pivx_rpc
            .call("sendrawtransaction", rpc_params![transaction])
            .await
    }

    pub async fn sync(&self) -> crate::error::Result<()> {
        self.address_index.clone().sync().await
    }
}
