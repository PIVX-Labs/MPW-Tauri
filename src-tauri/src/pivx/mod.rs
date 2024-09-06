#[cfg(test)]
mod test;

use flate2::read::GzDecoder;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command};
use tar::Archive;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PIVXErrors {
    #[error("Failed to fetch data")]
    FetchError(#[from] reqwest::Error),

    #[error("Server returned a non-ok status code")]
    ServerError,

    #[error("No data directory found")]
    NoDataDir,

    #[error("Failed to create file")]
    CreateFileError(#[from] std::io::Error),

    #[error("Pivxd not found")]
    PivxdNotFound,

    #[error("Invalid sha256 sum")]
    WrongSha256Sum(Option<std::io::Error>),
}

pub struct PIVX {
    handle: Child,
}

impl Drop for PIVX {
    fn drop(&mut self) {
        // This sends SIGKILL so this should be refactored to send SIGTERM
        self.handle.kill().expect("Failed to kill pivxd");
        self.handle.wait().expect("Failed to wait");
    }
}

impl PIVX {
    /**
     * Fetches pivxd and copies it into $XDG_DATA_HOME/pivx-rust or equivalent based on OS
     */
    async fn fetch(pivxd_url: &str, dir: &PathBuf) -> Result<(), PIVXErrors> {
        let mut request = reqwest::get(pivxd_url.to_string()).await?;
        if !request.status().is_success() {
            return Err(PIVXErrors::ServerError);
        }
        std::fs::create_dir_all(&dir)?;
        let file_path = dir.join("pivxd.tar.gz");
        let mut file = File::create(&file_path)?;
        while let Some(chunk) = request.chunk().await? {
            file.write_all(&chunk)?;
        }

        let digest =
            sha256::try_digest(&file_path).map_err(|e| PIVXErrors::WrongSha256Sum(Some(e)))?;
        println!("{:?}", file_path.to_str());
        println!("{}", digest);
        if digest != Self::get_pivxd_sha256sum() {
            Err(PIVXErrors::WrongSha256Sum(None))
        } else {
            Ok(())
        }
    }

    fn decompress_archive(dir: &PathBuf) -> Result<(), PIVXErrors> {
        let mut tarball = Archive::new(GzDecoder::new(File::open(dir.join("pivxd.tar.gz"))?));
        tarball.unpack(dir)?;

        Ok(())
    }

    #[cfg(not(test))]
    fn get_data_dir() -> Result<PathBuf, PIVXErrors> {
        Ok(dirs::data_dir()
            .ok_or(PIVXErrors::NoDataDir)?
            .join("pivx-rust"))
    }

    #[cfg(test)]
    fn get_data_dir() -> Result<PathBuf, PIVXErrors> {
        use tempdir::TempDir;
        Ok(TempDir::new("pivx-rust")?.into_path())
    }

    fn get_pivxd_url() -> &'static str {
        #[cfg(target_os = "linux")]
	return "https://github.com/PIVX-Project/PIVX/releases/download/v5.6.1/pivx-5.6.1-x86_64-linux-gnu.tar.gz";

        #[allow(unreachable_code)]
        {
            panic!("Unsupported OS")
        }
    }

    #[cfg(test)]
    fn get_pivxd_sha256sum() -> &'static str {
        "398e8a1a206f898139947a2003bf738c0f39b63f5d9a3116a68d6f483421b0b5"
    }

    #[cfg(not(test))]
    fn get_pivxd_sha256sum() -> &'static str {
        #[cfg(target_os = "linux")]
        return "6704625c63ff73da8c57f0fbb1dab6f1e4bd8f62c17467e05f52a64012a0ee2f";
        #[allow(unreachable_code)]
        {
            panic!("Unsupported OS")
        }
    }

    fn new_by_path(path: &str) -> Result<Self, PIVXErrors> {
        let data_dir = Self::get_data_dir()?.join(".pivx");
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir)?;
        }
        let mut handle = Command::new(path)
            .arg(format!(
                "-datadir={}",
                data_dir.to_str().ok_or(PIVXErrors::PivxdNotFound)?
            ))
            .spawn()
            .map_err(|_| PIVXErrors::PivxdNotFound)?;
        Ok(PIVX { handle })
    }

    pub fn new() -> Result<Self, PIVXErrors> {
        Self::new_by_path("pivxd")
    }

    pub async fn new_by_fetching() -> Result<Self, PIVXErrors> {
        let data_dir = Self::get_data_dir()?;
        let pivxd_path = data_dir.join("pivx-5.6.1").join("bin").join("pivxd");
        if !pivxd_path.exists() {
            Self::fetch(Self::get_pivxd_url(), &data_dir).await?;
            Self::decompress_archive(&data_dir)?;
        }
        Self::new_by_path(&*pivxd_path.to_string_lossy())
    }
}
