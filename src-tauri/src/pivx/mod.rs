#[cfg(test)]
mod test;

use crate::error::PIVXErrors;
use flate2::read::GzDecoder;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Archive;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::binary::BinaryDefinition;

pub struct PIVXDefinition;

impl PIVXDefinition {
    fn inner_decompress_archive(&self, dir: &Path) -> Result<(), PIVXErrors> {
        let mut tarball = Archive::new(GzDecoder::new(File::open(dir.join("pivxd.tar.gz"))?));
        tarball.unpack(dir)?;
        Ok(())
    }

    fn inner_install_params(&self, dir: &Path) -> Result<(), PIVXErrors> {
        let pivx_dir = dir.join("pivx-5.6.1");
        let script_path = pivx_dir.join("install-params.sh");
        let mut handle = Command::new(script_path)
            .current_dir(pivx_dir)
            .spawn()
            .map_err(|_| PIVXErrors::FetchParamsFailed)?;
        let status = handle.wait().map_err(|_| PIVXErrors::FetchParamsFailed)?;
        match status.success() {
            true => Ok(()),
            false => Err(PIVXErrors::FetchParamsFailed),
        }
    }
}

impl BinaryDefinition for PIVXDefinition {
    fn decompress_archive(&self, dir: &Path) -> Result<(), PIVXErrors> {
        self.inner_decompress_archive(dir)?;
        self.inner_install_params(dir)
    }

    fn get_url(&self) -> &str {
        #[cfg(target_os = "linux")]
	return "https://github.com/PIVX-Project/PIVX/releases/download/v5.6.1/pivx-5.6.1-x86_64-linux-gnu.tar.gz";

        #[allow(unreachable_code)]
        {
            panic!("Unsupported OS")
        }
    }

    fn get_sha256sum(&self) -> &str {
        #[cfg(target_os = "linux")]
        return "6704625c63ff73da8c57f0fbb1dab6f1e4bd8f62c17467e05f52a64012a0ee2f";
        #[allow(unreachable_code)]
        {
            panic!("Unsupported OS")
        }
    }

    fn get_archive_name(&self) -> &str {
        #[cfg(target_os = "linux")]
        return "pivxd.tar.gz";

        #[allow(unreachable_code)]
        {
            panic!("Unsupported OS")
        }
    }

    fn get_binary_path(&self, base_dir: &Path) -> PathBuf {
        base_dir.join("pivx-5.6.1").join("bin").join("pivxd")
    }

    fn get_binary_args(&self, base_dir: &Path) -> Result<Vec<String>, PIVXErrors> {
        let args = format!(
            "-datadir={} -rpcport={} -rpcuser={} -rpcpassword={} -txindex=1",
            base_dir.to_str().ok_or(PIVXErrors::PivxdNotFound)?,
            crate::RPC_PORT,
            crate::RPC_USERNAME,
            crate::RPC_PASSWORD,
        );
        Ok(args.split(" ").map(|s| s.to_string()).collect::<Vec<_>>())
    }

    async fn wait_for_load(&self, handle: &mut tokio::process::Child) -> crate::error::Result<()> {
        let stdout = handle.stdout.take();
        match stdout {
            Some(stdout) => {
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();
                loop {
                    let read_bytes = reader.read_line(&mut line).await?;
                    if read_bytes == 0 {
                        return Err(PIVXErrors::PivxdStopped);
                    }
                    if line.contains("asking peer for sporks") {
                        break;
                    }
                }
            }
            None => eprintln!(
                "Warning: couldn't wait for load because no stdout is attached to the handle."
            ),
        }
        Ok(())
    }
}
