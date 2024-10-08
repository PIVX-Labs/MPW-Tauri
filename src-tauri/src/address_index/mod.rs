pub mod address_extractor;
pub mod block_file_source;
pub mod block_source;
pub mod database;
pub mod pivx_rpc;
pub mod sql_lite;
pub mod types;

use block_source::{BlockSource, BlockSourceType};
use database::Database;
use futures::StreamExt;
use types::{Block, Vin};

#[derive(Clone)]
pub struct AddressIndex<D: Database> {
    database: D,
    block_source: BlockSourceType,
}

impl<D> AddressIndex<D>
where
    D: Database + Send,
{
    pub async fn sync(&mut self) -> crate::error::Result<()> {
        println!("Starting sync");
        match &self.block_source {
            BlockSourceType::Regular(block_source) => {
                let mut stream = block_source.get_blocks()?.chunks(500_000);
                while let Some(blocks) = stream.next().await {
                    Self::store_blocks(&mut self.database, blocks.into_iter()).await?;
                }
            }
            BlockSourceType::Indexed(block_source) => {
                let start = self.database.get_last_indexed_block().await?;
                let mut stream = block_source.get_blocks_indexed(start)?.chunks(10);
                while let Some(blocks) = stream.next().await {
                    let block_count = blocks.last().map(|(_, i)| *i);
                    Self::store_blocks(
                        &mut self.database,
                        blocks.into_iter().map(|(block, _)| block),
                    )
                    .await?;
                    if let Some(block_count) = block_count {
                        self.database.update_block_count(block_count).await?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn store_blocks(
        database: &mut D,
        blocks: impl Iterator<Item = Block>,
    ) -> crate::error::Result<()> {
        database
            .store_txs(blocks.flat_map(|block| block.txs.into_iter()))
            .await?;
        Ok(())
    }

    pub fn new<B>(database: D, block_source: B) -> Self
    where
        B: BlockSource + 'static + Send + Sync,
    {
        Self {
            database,
            block_source: block_source.instantiate(),
        }
    }
    pub async fn get_address_txids(&self, address: &str) -> crate::error::Result<Vec<String>> {
        self.database.get_address_txids(address).await
    }

    pub async fn get_txid_from_vin(&self, vin: &Vin) -> crate::error::Result<Option<String>> {
        self.database.get_txid_from_vin(vin).await
    }

    pub fn set_block_source<T>(&mut self, block_source: T)
    where
        T: BlockSource + Send + Sync + 'static,
    {
        self.block_source = block_source.instantiate();
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
