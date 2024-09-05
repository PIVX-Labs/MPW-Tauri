#[cfg(test)]
mod test;

use flate2::read::GzDecoder;
use std::fs::File;
use std::io::Write;
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
        let mut file = File::create(dir.join("pivxd.tar.gz"))?;
        while let Some(chunk) = request.chunk().await? {
            file.write_all(&chunk)?;
        }
        Ok(())
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
        println!(
            "-datadir={}",
            data_dir.to_str().ok_or(PIVXErrors::PivxdNotFound)?
        );
        /*	handle.wait();
        let mut string = String::new();
        use std::io::Read;
        if let Some(ref mut stdout) = handle.stdout {
            stdout.read_to_string(&mut string);
        }
        println!("{}", string);*/
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
