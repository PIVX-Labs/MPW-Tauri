use std::path::PathBuf;

use super::database::Database;
use super::types::Tx;
use rusqlite::{params, Connection};

pub struct SqlLite {
    connection: Connection,
}

impl SqlLite {
    pub async fn new(path: PathBuf) -> crate::error::Result<Self> {
        tauri::async_runtime::spawn_blocking(move || {
	    let connection = Connection::open(path)?;
	    connection.execute_batch("
BEGIN;
CREATE TABLE IF NOT EXISTS transactions(txid TEXT NOT NULL, address TEXT NOT NULL, PRIMARY KEY (txid, address));
CREATE INDEX IF NOT EXISTS idx_address ON transactions (address);
COMMIT;
")?;
	    Ok(Self{connection})
	}).await?
    }
}

impl Database for SqlLite {
    async fn get_address_txids(&self, address: &str) -> crate::error::Result<Vec<String>> {
        let mut stmt = self
            .connection
            .prepare("SELECT txid FROM transactions WHERE address=?1")?;
        let mut rows = stmt.query([address])?;
        let mut txids = vec![];
        while let Some(row) = rows.next()? {
            let txid: String = row.get(0)?;
            txids.push(txid);
        }
        Ok(txids)
    }
    async fn store_tx(&mut self, tx: &Tx) -> crate::error::Result<()> {
        let txid = &tx.txid;
        let mut stmt = self
            .connection
            .prepare("INSERT OR IGNORE INTO transactions (txid, address) VALUES (?1, ?2);")?;
        for address in &tx.addresses {
            stmt.execute(params![txid, &address])?;
        }
        Ok(())
    }

    async fn store_txs<I>(&mut self, txs: I) -> crate::error::Result<()>
    where
        I: Iterator<Item = Tx>,
    {
        let connection = self.connection.transaction()?;
        for tx in txs {
            let txid = &tx.txid;
            for address in &tx.addresses {
                connection.execute(
                    "INSERT OR IGNORE INTO transactions (txid, address) VALUES (?1, ?2);",
                    params![txid, &address],
                )?;
            }
        }
        connection.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::super::types::test::get_test_blocks;
    use super::*;
    use tempdir::TempDir;

    async fn test_address_retrival(sql_lite: &SqlLite) -> crate::error::Result<()> {
        assert_eq!(
            sql_lite.get_address_txids("address1").await?,
            vec!["txid1", "txid2", "txid3"]
        );
        assert_eq!(sql_lite.get_address_txids("address2").await?, vec!["txid1"]);
        assert_eq!(sql_lite.get_address_txids("address4").await?, vec!["txid2"]);
        assert_eq!(sql_lite.get_address_txids("address5").await?, vec!["txid3"]);
        assert_eq!(
            sql_lite.get_address_txids("address6").await?,
            Vec::<String>::new()
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_sqlite() -> crate::error::Result<()> {
        let temp_dir = TempDir::new("sqlite-test")?;
        let mut sql_lite = SqlLite::new(temp_dir.path().join("test.sqlite")).await?;
        let test_blocks = get_test_blocks();
        for block in test_blocks {
            for tx in block.txs {
                sql_lite.store_tx(&tx).await?;
            }
        }
        test_address_retrival(&sql_lite).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_sqlite_batch() -> crate::error::Result<()> {
        let temp_dir = TempDir::new("sqlite-test-batch")?;
        let mut sql_lite = SqlLite::new(temp_dir.path().join("test.sqlite")).await?;
        let test_blocks = get_test_blocks();
        sql_lite
            .store_txs(
                test_blocks
                    .into_iter()
                    .flat_map(|block| block.txs.into_iter()),
            )
            .await?;

        test_address_retrival(&sql_lite).await?;
        Ok(())
    }
}
