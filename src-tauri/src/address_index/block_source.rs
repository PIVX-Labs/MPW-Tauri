use super::types::Block;
use futures::stream::Stream;
use futures::Future;
use serde_json::value::Index;
use std::{ops::Deref, pin::Pin, sync::Arc};

pub type PinnedStream<'a, T> = Pin<Box<dyn Stream<Item = T> + 'a + Send>>;
pub type BS = Arc<dyn BlockSource + 'static + Send + Sync>;
pub type IBS = Arc<dyn IndexedBlockSource + 'static + Send + Sync>;

#[derive(Clone)]
pub enum BlockSourceType {
    Regular(BS),
    Indexed(IBS),
}

impl Deref for BlockSourceType {
    type Target = dyn BlockSource;

    fn deref<'a>(&'a self) -> &'a Self::Target {
        match self {
            Self::Regular(ref d) => return d.as_ref(),
            Self::Indexed(ref d) => return d.as_ref().as_block_source(),
        }
    }
}

pub trait BlockSource {
    fn get_blocks(&self) -> crate::error::Result<PinnedStream<'_, Block>>;

    // IndexedBlockSource must override this.
    fn instantiate(self) -> BlockSourceType
    where
        Self: Sized + 'static + Send + Sync,
    {
        BlockSourceType::Regular(Arc::new(self))
    }
}

pub trait IndexedBlockSource: BlockSource {
    /**
     * Returns a stream of blocks with associated block count.
     * Stream must be sorted by block count
     */
    fn get_blocks_indexed(
        &self,
        start_from: u64,
    ) -> crate::error::Result<PinnedStream<'_, (Block, u64)>>;
    fn as_block_source(&self) -> &(dyn BlockSource + Send + Sync + 'static);
}

#[cfg(test)]
pub mod test {
    use super::super::types::{test::get_test_blocks, Block};
    use super::*;

    pub struct MockBlockSource;

    impl BlockSource for MockBlockSource {
        fn get_blocks(
            &self,
        ) -> crate::error::Result<Pin<Box<dyn Stream<Item = Block> + '_ + Send>>> {
            Ok(Box::pin(futures::stream::iter(get_test_blocks())))
        }
    }
}
