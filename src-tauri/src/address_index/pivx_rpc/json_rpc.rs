use derive_more::derive::Display;
use jsonrpsee::core::traits::ToRpcParams;
use reqwest::{header::HeaderMap, ClientBuilder};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::value::RawValue;
use thiserror::Error;

#[derive(Deserialize, Debug)]
#[allow(unused)]
pub struct JSONRpcResponse<T, E> {
    result: Option<T>,
    error: Option<JSONRpcError<E>>,
    id: Option<i32>,
}

#[derive(Deserialize, Debug, Display)]
#[allow(unused)]
#[display("{} {} {:?}", code, message, data)]
pub struct JSONRpcError<E> {
    pub code: i32,
    pub message: String,
    pub data: E,
}

#[derive(Error, Debug)]
pub enum Error<E> {
    #[error("JSON Rpc error")]
    JSONRpc(JSONRpcError<E>),
    #[error("Failed to fetch")]
    Fetch(#[from] reqwest::Error),
    #[error("Invalid response")]
    InvalidResponse,
    #[error("Invalid params")]
    InvalidParams,
}

pub struct HttpClientBuilder {
    headers: Option<HeaderMap>,
}

impl HttpClientBuilder {
    pub fn new() -> Self {
        HttpClientBuilder { headers: None }
    }

    pub fn set_headers(mut self, headers: HeaderMap) -> Self {
        self.headers = Some(headers);
        self
    }

    pub fn build<T>(self, url: T) -> crate::error::Result<HttpClient>
    where
        T: Into<String>,
    {
        let mut client_builder = ClientBuilder::new();
        if let Some(headers) = self.headers {
            client_builder = client_builder.default_headers(headers);
        }
        Ok(HttpClient {
            client: client_builder.build()?,
            url: url.into(),
        })
    }
}

#[derive(Serialize, Debug)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'static str,
    method: &'a str,
    params: Option<Box<RawValue>>,
    id: i32,
}

#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
    url: String,
}

impl HttpClient {
    pub fn builder() -> HttpClientBuilder {
        HttpClientBuilder::new()
    }

    pub async fn request<T, E, P>(&self, rpc: &str, params: P) -> Result<T, Error<E>>
    where
        P: ToRpcParams + Send,
        T: DeserializeOwned,
        E: DeserializeOwned,
    {
        /*println!("{}", serde_json::json!(&JsonRpcRequest {
                jsonrpc: "2.0",
                method: rpc,
                params: params.to_rpc_params().map_err(|_| Error::InvalidParams)?,
        id: 1,
            }));*/
        let response: JSONRpcResponse<T, E> = self
            .client
            .post(&self.url)
            .json(&JsonRpcRequest {
                jsonrpc: "2.0",
                method: rpc,
                params: params.to_rpc_params().map_err(|_| Error::InvalidParams)?,
                id: 1,
            })
            .send()
            .await?
            .json()
            .await?;

        if let Some(res) = response.result {
            Ok(res)
        } else if let Some(err) = response.error {
            Err(Error::JSONRpc(err))
        } else {
            Err(Error::InvalidResponse)
        }
    }
}
