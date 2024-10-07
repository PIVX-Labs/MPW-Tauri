pub mod json_rpc;
mod test;

use crate::error::PIVXErrors;

use super::block_source::{BlockSource, BlockSourceType, IndexedBlockSource, PinnedStream};
use super::types::Block;
use base64::prelude::*;
use futures::stream::Stream;
use futures::StreamExt;
use json_rpc::HttpClient;
use jsonrpsee::core::traits::ToRpcParams;
use jsonrpsee::rpc_params;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

#[derive(Clone)]
pub struct PIVXRpc {
    client: HttpClient,
}

type BlockStreamFuture = Pin<Box<dyn Future<Output = Option<(Block, u64)>> + Send>>;

struct BlockStream {
    client: HttpClient,
    current_block: u64,
    current_future: Option<BlockStreamFuture>,
}

impl BlockStream {
    async fn get_next_block(client: HttpClient, current_block: u64) -> Option<(Block, u64)> {
        println!("current block: {}", current_block);
        let hash: String = client
            .request::<_, (), _>("getblockhash", rpc_params![current_block])
            .await
            .unwrap();
        let block: Result<Block, _> = client
            .request::<_, (), _>("getblock", rpc_params![hash, 2])
            .await;
        if let Err(ref err) = &block {
            eprintln!("{}", err);
        }
        block.ok().map(|b| (b, current_block))
    }

    pub fn new(client: HttpClient) -> Self {
        Self {
            client,
            current_block: 0,
            current_future: None,
        }
    }

    pub fn with_starting_block(client: HttpClient, starting_block: u64) -> Self {
        Self {
            client,
            current_block: starting_block,
            current_future: None,
        }
    }
}

impl Stream for BlockStream {
    type Item = (Block, u64);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(ref mut future) = &mut self.current_future {
            let poll = Pin::as_mut(future).poll(cx);
            match poll {
                Poll::Ready(i) => {
                    self.current_future = None;
                    Poll::Ready(i)
                }
                Poll::Pending => Poll::Pending,
            }
        } else {
            self.as_mut().current_block = self.current_block + 1;
            let new_future = Box::pin(Self::get_next_block(
                self.client.clone(),
                self.current_block,
            ));
            self.current_future = Some(new_future);
            self.poll_next(cx)
        }
    }
}

impl PIVXRpc {
    pub async fn new(url: &str) -> crate::error::Result<Self> {
        let mut headers = HeaderMap::new();
        let credentials = format!("{}:{}", crate::RPC_USERNAME, crate::RPC_PASSWORD);
        headers.insert(
            "Authorization",
            // TODO: remove unwrap
            HeaderValue::from_str(&format!("Basic {}", BASE64_STANDARD.encode(credentials)))
                .unwrap(),
        );
        Ok(PIVXRpc {
            client: HttpClient::builder().set_headers(headers).build(url)?,
        })
    }

    pub async fn call<T, P>(&self, rpc: &str, params: P) -> crate::error::Result<T>
    where
        P: ToRpcParams + Send,
        T: DeserializeOwned,
    {
        let res = self.client.request::<_, (), _>(rpc, params).await;
        match res {
            Ok(res) => Ok(res),
            Err(_) => Err(PIVXErrors::InvalidResponse),
        }
    }
}

impl BlockSource for PIVXRpc {
    fn get_blocks(&self) -> crate::error::Result<Pin<Box<dyn Stream<Item = Block> + Send + '_>>> {
        Ok(Box::pin(self.get_blocks_indexed(0)?.map(|(b, _)| b)))
    }

    fn instantiate(self) -> BlockSourceType {
        BlockSourceType::Indexed(Arc::new(self))
    }
}

impl IndexedBlockSource for PIVXRpc {
    fn get_blocks_indexed(
        &self,
        start_from: u64,
    ) -> crate::error::Result<PinnedStream<'_, (Block, u64)>> {
        let block_stream = BlockStream::with_starting_block(self.client.clone(), start_from);

        Ok(Box::pin(block_stream))
    }

    fn as_block_source(&self) -> &(dyn BlockSource + Send + Sync + 'static) {
        self
    }
}
