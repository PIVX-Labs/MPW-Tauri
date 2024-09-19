use super::types::Tx;

pub trait Database {
    async fn get_address_txids(&self, address: &str) -> crate::error::Result<Vec<String>>;
    async fn store_tx(&mut self, tx: &Tx) -> crate::error::Result<()>;
    /**
     * Override if there is a more efficient way to store multiple txs at the same time
     */
    async fn store_txs<I>(&mut self, txs: I) -> crate::error::Result<()>
    where
        I: Iterator<Item = Tx>,
    {
        for tx in txs {
            self.store_tx(&tx).await?;
        }
        Ok(())
    }
}
