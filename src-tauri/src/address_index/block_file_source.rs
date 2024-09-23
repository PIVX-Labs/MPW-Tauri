use super::block_source::BlockSource;
use super::types::Block;
use crate::address_index::address_extractor::AddressExtractor;
use crate::error::PIVXErrors;
use futures::stream;
use futures::stream::Stream;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::prelude::*;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::pin::Pin;

pub struct BlockFileSource {
    db_path: PathBuf,
}

impl BlockFileSource {
    pub fn new<T>(db_path: T) -> Self
    where
        T: Into<PathBuf>,
    {
        Self {
            db_path: db_path.into(),
        }
    }
}

struct BlockFileIterator {
    db_path: PathBuf,
    open_file: Option<File>,
    counter: i32,
}

impl BlockFileIterator {
    pub fn new(db_path: &Path) -> Self {
        BlockFileIterator {
            db_path: db_path.into(),
            open_file: None,
            counter: 0,
            //counter: 0,
        }
    }
}

impl Iterator for BlockFileIterator {
    type Item = Block;

    fn next(&mut self) -> Option<Self::Item> {
        let mut file = match &self.open_file {
            Some(file) => file,
            None => {
                println!(
                    "opening file {:?}...",
                    self.db_path.join(format!("blk{:0>5}.dat", self.counter))
                );
                self.open_file = Some(
                    File::open(self.db_path.join(format!("blk{:0>5}.dat", self.counter))).ok()?,
                );
                self.counter += 1;
                &self.open_file.as_ref().unwrap()
            }
        };
        let block = AddressExtractor::get_addresses_from_block(&mut file);
        match block {
            Ok(block) => Some(block),
            Err(PIVXErrors::InvalidBlock) => self.next(),
            Err(_) => {
                self.open_file = None;
                self.next()
            }
        }
    }
}

impl BlockSource for BlockFileSource {
    fn get_blocks(
        &mut self,
    ) -> crate::error::Result<Pin<Box<dyn Stream<Item = Block> + '_ + Send>>> {
        let block_file_iterator = BlockFileIterator::new(&self.db_path);
        Ok(Box::pin(stream::iter(block_file_iterator)))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn temp_test() -> crate::error::Result<()> {
        let mut b = BlockFileSource::new("/home/duddino/.local/share/pivx-rust/.pivx/blocks/");
        let mut stream = b.get_blocks()?;
        let mut counter = 0;
        while let Some(block) = stream.next().await {
            counter += 1;
        }
        println!("slurped {} blocks", counter);
        Ok(())
    }
}
