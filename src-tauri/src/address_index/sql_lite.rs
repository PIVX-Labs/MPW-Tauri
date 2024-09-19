use std::path::PathBuf;

use super::database::Database;
use super::types::Tx;
use rusqlite::{params, Connection};

pub struct SqlLite {
    connection: Connection,
}

impl SqlLite {
    pub async fn new(path: &PathBuf) -> crate::error::Result<Self> {
        let path = path.clone();
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
            .prepare("SELECT txid FROM transaction WHERE address=?1")?;
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
