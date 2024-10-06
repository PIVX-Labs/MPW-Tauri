#[cfg(test)]
mod test {
    use super::super::json_rpc::*;
    use jsonrpsee::rpc_params;
    use mockito::Server as MockServer;

    #[tokio::test]
    async fn fetches_block_number_correctly() -> Result<(), Box<dyn std::error::Error>> {
        let mut server = MockServer::new_async().await;

        // Mock the response for a JSON RPC method call
        server
            .mock("POST", "/")
            .match_header("content-type", "application/json")
            .match_body(
                r#"{"jsonrpc":"2.0","method":"getblockhash","params":["coolblock"],"id":1}"#,
            )
            .with_status(200)
            .with_body(r#"{"result":"coolhash","error":null,"id":1}"#)
            .create_async()
            .await;

        // Create a new JSON RPC client with the mock server's URL
        let client = HttpClientBuilder::new().build(server.url())?;

        // Call the JSON RPC method
        let block_number: String = client
            .request::<_, (), _>("getblockhash", rpc_params!["coolblock"])
            .await?;

        // Assert that the response matches what we expect
        assert_eq!(block_number, "coolhash");

        Ok(())
    }

    #[tokio::test]
    async fn returns_error_when_rpc_call_fails() -> Result<(), Box<dyn std::error::Error>> {
        let mut server = MockServer::new_async().await;

        // Mock the response to simulate an error
        server
            .mock("POST", "/")
            .match_header("content-type", "application/json")
	    .match_body(r#"{"jsonrpc":"2.0","method":"getblockhash","params":["coolblock"],"id":1}"#)
            .with_status(500)
	    .with_body(r#"{"jsonrpc":"2.0","error": { "code": 404, "data": "damn", "message": "errored out" },"id":1}"#)
            .create_async()
            .await;

        // Create a new JSON RPC client with the mock server's URL
        let client = HttpClientBuilder::new().build(server.url())?;

        // Attempt to make the RPC call and expect an error
        let result = client
            .request::<(), String, _>("getblockhash", rpc_params!["coolblock"])
            .await;
        assert!(result.is_err());
        let result = result.unwrap_err();
        match result {
            Error::JSONRpc(e) => {
                assert_eq!(&e.data, "damn");
                assert_eq!(e.code, 404);
                assert_eq!(&e.message, "errored out");
            }
            _ => panic!("Invalid error"),
        }

        Ok(())
    }
}
