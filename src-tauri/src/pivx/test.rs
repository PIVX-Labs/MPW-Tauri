use super::*;
use std::fs::File;
use tempdir::TempDir;

mod pivx_fetch {
    use super::*;
    #[tokio::test]
    async fn fetches_the_binary_correctly() -> Result<(), PIVXErrors> {
        let data_dir = PIVX::get_data_dir()?;
        let mut server = mockito::Server::new_async().await;
        let m1 = server
            .mock("GET", "/")
            .with_body("PIVX Source code")
            .create_async()
            .await;
        PIVX::fetch(&server.url(), &data_dir).await?;

        let content = std::fs::read_to_string(data_dir.join("pivxd.tar.gz"))?;
        assert_eq!(content, "PIVX Source code");
        Ok(())
    }

    #[tokio::test]
    async fn returns_error_when_server_returns_404() -> Result<(), PIVXErrors> {
        let data_dir = PIVX::get_data_dir()?;
        let mut server = mockito::Server::new_async().await;
        let m1 = server
            .mock("GET", "/")
            .with_status(500)
            .with_body("Internal server error.")
            .create_async()
            .await;
        match PIVX::fetch(&server.url(), &data_dir).await {
            Err(x) => {}
            Ok(_) => panic!("Shuold return error"),
        };

        Ok(())
    }

    #[test]
    fn correctly_uncompresses_archive_linux() -> Result<(), PIVXErrors> {
        let data_dir = PIVX::get_data_dir()?;
        let mut file = File::create(data_dir.join("pivxd.tar.gz"))?;
        // This is a simple gzipped tar archive
        let data: [u8; 111] = [
            0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0xed, 0xce, 0x21, 0x0e,
            0x02, 0x31, 0x14, 0x04, 0xd0, 0x7f, 0x94, 0xde, 0x80, 0x5f, 0xa0, 0xed, 0x79, 0x36,
            0xa9, 0xc1, 0x40, 0x02, 0xec, 0xfd, 0x01, 0x41, 0x82, 0x21, 0xa8, 0x45, 0xbd, 0x67,
            0x46, 0xcc, 0x88, 0x59, 0x76, 0xb1, 0xb9, 0x7c, 0x1a, 0xad, 0xbd, 0xb2, 0x8e, 0x96,
            0x9f, 0xf9, 0x16, 0xf5, 0xd8, 0x7b, 0xcf, 0xec, 0xa3, 0x1f, 0x22, 0x6b, 0xad, 0x6d,
            0x1f, 0xa5, 0x6d, 0x7f, 0x2d, 0x62, 0xbd, 0xdd, 0x97, 0x6b, 0x29, 0x31, 0xd7, 0x39,
            0x4f, 0xe7, 0xcb, 0xd7, 0xdd, 0xaf, 0x1e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0xfe, 0xe8, 0x01, 0x83, 0xad, 0x18, 0xb3, 0x00, 0x28, 0x00, 0x00,
        ];
        file.write_all(&data)?;
        PIVX::decompress_archive(&data_dir)?;

        let dirs: Vec<_> = std::fs::read_dir(data_dir)?
            .filter_map(|d| {
                Some(
                    d.ok()?
                        .path()
                        .to_string_lossy()
                        .split(std::path::MAIN_SEPARATOR)
                        .last()?
                        .to_string(),
                )
            })
            .collect();
        assert_eq!(dirs, vec!["pivxd.tar.gz", "a"]);

        Ok(())
    }
}
