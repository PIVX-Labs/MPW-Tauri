use super::*;
use std::fs::File;
use tempdir::TempDir;

struct TestBinary {
    url: String,
}

impl BinaryDefinition for TestBinary {
    fn get_url(&self) -> &str {
        &self.url
    }
    fn get_sha256sum(&self) -> &str {
        "398e8a1a206f898139947a2003bf738c0f39b63f5d9a3116a68d6f483421b0b5"
    }
    fn get_archive_name(&self) -> &str {
        "a.tar.gz"
    }
    fn decompress_archive(&self, dir: &PathBuf) -> Result<(), PIVXErrors> {
        Ok(())
    }
    fn get_binary_path(&self, base_dir: &PathBuf) -> PathBuf {
        unimplemented!()
    }
    fn get_binary_args(&self, _: &PathBuf) -> Result<Vec<String>, PIVXErrors> {
        unimplemented!()
    }
}
mod pivx_fetch {
    use super::*;
    #[tokio::test]
    async fn fetches_the_binary_correctly() -> Result<(), PIVXErrors> {
        let data_dir = Binary::get_data_dir()?;
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/")
            .with_body("PIVX Source code")
            .create_async()
            .await;
        let binary_definition = TestBinary { url: server.url() };
        Binary::fetch(&data_dir, &binary_definition).await?;

        let content = std::fs::read_to_string(data_dir.join("a.tar.gz"))?;
        assert_eq!(content, "PIVX Source code");
        Ok(())
    }

    #[tokio::test]
    async fn returns_error_when_server_returns_404() -> Result<(), PIVXErrors> {
        let data_dir = Binary::get_data_dir()?;
        let mut server = mockito::Server::new_async().await;
        let m1 = server
            .mock("GET", "/")
            .with_status(500)
            .with_body("Internal server error.")
            .create_async()
            .await;
        let binary_definition = TestBinary { url: server.url() };
        match Binary::fetch(&data_dir, &binary_definition).await {
            Err(x) => {}
            Ok(_) => panic!("Shuold return error"),
        };

        Ok(())
    }
}
