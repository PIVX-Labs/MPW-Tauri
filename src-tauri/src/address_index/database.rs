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

#[cfg(test)]
pub mod test {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    pub struct MockDB {
        address_map: HashMap<String, Vec<String>>,
    }

    impl Database for MockDB {
        async fn get_address_txids(&self, address: &str) -> crate::error::Result<Vec<String>> {
            Ok(self.address_map.get(address).unwrap_or(&vec![]).clone())
        }

        async fn store_tx(&mut self, tx: &Tx) -> crate::error::Result<()> {
            for address in &tx.addresses {
                self.address_map
                    .entry(address.clone())
                    .and_modify(|vec| vec.push(tx.txid.clone()))
                    .or_insert(vec![tx.txid.clone()]);
            }
            Ok(())
        }
    }
}
