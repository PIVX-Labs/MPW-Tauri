#[cfg(test)]
mod test;

use crate::error::PIVXErrors;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub trait BinaryDefinition {
    fn get_url(&self) -> &str;
    fn get_sha256sum(&self) -> &str;
    fn get_archive_name(&self) -> &str;
    fn decompress_archive(&self, dir: &PathBuf) -> Result<(), PIVXErrors>;
    fn get_binary_path(&self, base_dir: &PathBuf) -> PathBuf;
    fn get_binary_args(&self, base_dir: &PathBuf) -> Result<Vec<String>, PIVXErrors>;
}

pub struct Binary {
    handle: Child,
}

impl Drop for Binary {
    fn drop(&mut self) {
        // This sends SIGKILL so this should be refactored to send SIGTERM
        self.handle.kill().expect("Failed to kill pivxd");
        self.handle.wait().expect("Failed to wait");
    }
}

impl Binary {
    /**
     * Fetches a binary and copies it into $XDG_DATA_HOME/pivx-rust or equivalent based on OS
     */
    async fn fetch<T: BinaryDefinition + Send>(
        dir: &PathBuf,
        binary_definition: &T,
    ) -> Result<(), PIVXErrors> {
        let mut request = reqwest::get(binary_definition.get_url()).await?;
        if !request.status().is_success() {
            return Err(PIVXErrors::ServerError);
        }
        std::fs::create_dir_all(&dir)?;
        let file_path = dir.join(binary_definition.get_archive_name());
        let mut file = File::create(&file_path)?;
        while let Some(chunk) = request.chunk().await? {
            file.write_all(&chunk)?;
        }

        let digest =
            sha256::try_digest(&file_path).map_err(|e| PIVXErrors::WrongSha256Sum(Some(e)))?;
        if digest != binary_definition.get_sha256sum() {
            Err(PIVXErrors::WrongSha256Sum(None))
        } else {
            Ok(())
        }
    }

    #[cfg(not(test))]
    pub fn get_data_dir() -> Result<PathBuf, PIVXErrors> {
        Ok(dirs::data_dir()
            .ok_or(PIVXErrors::NoDataDir)?
            .join("pivx-rust"))
    }

    #[cfg(test)]
    pub fn get_data_dir() -> Result<PathBuf, PIVXErrors> {
        use tempdir::TempDir;
        Ok(TempDir::new("pivx-rust")?.into_path())
    }

    fn new_by_path<T: BinaryDefinition + Send>(
        path: &str,
        binary_definition: &T,
    ) -> Result<Self, PIVXErrors> {
        let data_dir = Self::get_data_dir()?.join(".pivx");
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir)?;
        }
        let handle = Command::new(path)
            .args(binary_definition.get_binary_args(&data_dir)?)
            .stdout(Stdio::null())
            .spawn()
            .map_err(|_| PIVXErrors::PivxdNotFound)?;
        Ok(Binary { handle })
    }

    pub async fn new_by_fetching<T: BinaryDefinition + Send>(
        binary_definition: &T,
    ) -> Result<Self, PIVXErrors> {
        let data_dir = Self::get_data_dir()?;
        let binary_path = binary_definition.get_binary_path(&data_dir);
        if !binary_path.exists() {
            Self::fetch(&data_dir, binary_definition).await?;
            binary_definition.decompress_archive(&data_dir)?;
        }
        Self::new_by_path(&*binary_path.to_string_lossy(), binary_definition)
    }
}
