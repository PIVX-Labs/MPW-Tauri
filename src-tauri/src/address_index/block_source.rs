use super::types::Block;
use futures::stream::Stream;
use std::pin::Pin;

pub trait BlockSource {
    fn get_blocks(
        &mut self,
    ) -> crate::error::Result<Pin<Box<dyn Stream<Item = Block> + '_ + Send>>>;
}
