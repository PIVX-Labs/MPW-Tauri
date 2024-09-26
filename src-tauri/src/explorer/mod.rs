use jsonrpsee::rpc_params;
use futures::{StreamExt, TryStreamExt};
use tokio::sync::OnceCell;
use std::path::PathBuf;
use crate::*;

use crate::address_index::{
    block_source::BlockSource, database::Database, pivx_rpc::PIVXRpc, AddressIndex,
    sql_lite::SqlLite,
};
use global_function_macro::generate_global_functions;



pub struct Explorer<D, B>
where
    D: Database,
    B: BlockSource,
{
    address_index: AddressIndex<D, B>,
    pivx_rpc: PIVXRpc,
}

type DefaultExplorer = Explorer<SqlLite, PIVXRpc>;

impl<D, B> Explorer<D, B>
where
    D: Database + Send,
    B: BlockSource + Send,
{
    fn new(address_index: AddressIndex<D, B>, rpc: PIVXRpc) -> Self {
        Self {
            address_index,
            pivx_rpc: rpc,
        }
    }
}

static EXPLORER: OnceCell<DefaultExplorer> = OnceCell::const_new();

async fn get_explorer() -> &'static DefaultExplorer {
    EXPLORER.get_or_init(|| async {
	        let pivx_definition = PIVXDefinition;
let pivx = binary::Binary::new_by_fetching(&pivx_definition)
            .await
            .expect("Failed to run PIVX");
        let pivx_rpc = PIVXRpc::new(&format!("http://127.0.0.1:{}", RPC_PORT))
            .await
            .unwrap();
        let mut address_index = AddressIndex::new(
            SqlLite::new(PathBuf::from("/home/duddino/test.sqlite"))
                .await
                .unwrap(),
            pivx_rpc.clone(),
        );
	std::mem::forget(pivx);
	Explorer::new(address_index, pivx_rpc)
    }).await
}

#[generate_global_functions]
impl<D, B> Explorer<D, B>
where
    D: Database + Send,
    B: BlockSource + Send,
{
    pub async fn get_block(&self, block_height: u64) -> crate::error::Result<String> {
        let block_hash: String = self
            .pivx_rpc
            .call("getblockhash", rpc_params![block_height])
            .await?;
        let json: serde_json::Value = self
            .pivx_rpc
            .call("getblock", rpc_params![block_hash])
            .await?;
	Ok(json.to_string())
    }

    pub async fn get_block_count(&self) -> crate::error::Result<u64> {
	Ok(self.pivx_rpc.call("getblockcount", rpc_params![]).await?)
    }

    /// Gets all raw transactions containing one of `address`
    /// TODO: add also spent txs
    pub async fn get_txs(&self, addresses: Vec<&str>) -> crate::error::Result<Vec<String>> {
	let mut txs = vec![];
	for address in addresses {
	    for txid in self.address_index.get_address_txids(address).await? {
		txs.push(self.get_transaction(&txid).await?);
	    }
	}
	Ok(txs)
    }

    /// Gets raw transaction in hex format
    pub async fn get_transaction(&self, txid: &str) -> crate::error::Result<String> {

	Ok(self.pivx_rpc.call("getrawtransaction", rpc_params![txid]).await?)
    }

    pub async fn send_transaction(&self, transaction: &str) -> crate::error::Result<String> {
	Ok(self.pivx_rpc.call("sendtransaction", rpc_params![transaction]).await?)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::time::Duration;
    #[tokio::test]
    async fn temp_test() -> crate::error::Result<()> {
	use std::path::PathBuf;
	use crate::*;
        let pivx_definition = PIVXDefinition;
        let pivx = binary::Binary::new_by_fetching(&pivx_definition)
            .await
            .expect("Failed to run PIVX");
        let pivx_rpc = PIVXRpc::new(&format!("http://127.0.0.1:{}", RPC_PORT))
            .await
            .unwrap();
        let mut address_index = AddressIndex::new(
            SqlLite::new(PathBuf::from("/home/duddino/test.sqlite"))
                .await
                .unwrap(),
            pivx_rpc.clone(),
        );
	let explorer = Explorer::new(address_index, pivx_rpc);
	tokio::time::sleep(Duration::from_secs(60)).await;
	std::mem::forget(pivx);
	
	println!("{}", explorer.get_block(123).await?);
	Ok(())
    }

}
