pub mod block_source;
pub mod database;
pub mod pivx_rpc;
pub mod sql_lite;
pub mod types;

use block_source::BlockSource;
use database::Database;
use futures::StreamExt;

pub struct AddressIndex<D: Database, B: BlockSource> {
    database: D,
    block_source: B,
}

impl<D: Database + Send, B: BlockSource + Send> AddressIndex<D, B> {
    pub async fn sync(&mut self) -> crate::error::Result<()> {
        println!("Starting sync");
        let mut stream = self.block_source.get_blocks()?.chunks(10_000);
        while let Some(blocks) = stream.next().await {
            self.database
                .store_txs(blocks.into_iter().flat_map(|block| block.txs.into_iter()))
                .await?;
        }
        Ok(())
    }
    pub fn new(database: D, block_source: B) -> Self {
        Self {
            database,
            block_source,
        }
    }
    async fn get_address_txids(&self, address: &str) -> crate::error::Result<Vec<String>> {
        self.database.get_address_txids(address).await
    }
}

#[cfg(test)]
mod test {
    use super::block_source::test::MockBlockSource;
    use super::database::test::MockDB;
    use super::*;

    #[tokio::test]
    async fn syncs_correctly() -> crate::error::Result<()> {
        let mock_db = MockDB::default();
        let block_source = MockBlockSource;
        let mut address_index = AddressIndex::new(mock_db, block_source);
        address_index.sync().await?;
        assert_eq!(
            address_index.get_address_txids("address1").await?,
            vec!["txid1", "txid2", "txid3"]
        );
        assert_eq!(
            address_index.get_address_txids("address2").await?,
            vec!["txid1"]
        );
        assert_eq!(
            address_index.get_address_txids("address4").await?,
            vec!["txid2"]
        );
        assert_eq!(
            address_index.get_address_txids("address5").await?,
            vec!["txid3"]
        );
        assert_eq!(
            address_index.get_address_txids("address6").await?,
            Vec::<String>::new()
        );
        Ok(())
    }
}
