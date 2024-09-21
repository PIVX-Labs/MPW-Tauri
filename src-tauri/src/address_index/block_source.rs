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
    use super::super::types::{test::get_test_blocks, Block};
    use super::*;

    pub struct MockBlockSource;

    impl BlockSource for MockBlockSource {
        fn get_blocks(
            &mut self,
        ) -> crate::error::Result<Pin<Box<dyn Stream<Item = Block> + '_ + Send>>> {
            Ok(Box::pin(futures::stream::iter(get_test_blocks())))
        }
    }
}
