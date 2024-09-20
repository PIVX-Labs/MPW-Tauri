use super::types::Block;
use futures::stream::Stream;
use std::pin::Pin;

pub trait BlockSource {
    fn get_blocks(
        &mut self,
    ) -> crate::error::Result<Pin<Box<dyn Stream<Item = Block> + '_ + Send>>>;
}

#[cfg(test)]
pub mod test {
    use super::super::types::{Block, Tx};
    use super::*;
    pub struct MockBlockSource;
    impl BlockSource for MockBlockSource {
        fn get_blocks(
            &mut self,
        ) -> crate::error::Result<Pin<Box<dyn Stream<Item = Block> + '_ + Send>>> {
            Ok(Box::pin(futures::stream::iter(vec![
                Block {
                    txs: vec![Tx {
                        txid: "txid1".to_owned(),
                        addresses: vec!["address1".to_owned(), "address2".to_owned()],
                    }],
                },
                Block {
                    txs: vec![Tx {
                        txid: "txid2".to_owned(),
                        addresses: vec!["address1".to_owned(), "address4".to_owned()],
                    }],
                },
                Block {
                    txs: vec![Tx {
                        txid: "txid3".to_owned(),
                        addresses: vec!["address1".to_owned(), "address5".to_owned()],
                    }],
                },
            ])))
        }
    }
}
