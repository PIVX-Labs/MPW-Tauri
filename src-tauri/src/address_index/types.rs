use serde::{Deserialize, Deserializer};

#[derive(Deserialize, Debug)]
pub struct Block {
    #[serde(rename = "tx")]
    pub txs: Vec<Tx>,
}
#[derive(Deserialize, Debug)]
pub struct Tx {
    pub txid: String,

    #[serde(deserialize_with = "concat_addresses")]
    #[serde(rename = "vout")]
    pub addresses: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct Vout {
    #[serde(rename = "scriptPubKey")]
    pub script_pub_key: Option<ScriptPubKey>,
}
#[derive(Deserialize, Debug)]
struct ScriptPubKey {
    pub addresses: Option<Vec<String>>,
}

fn concat_addresses<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let vouts: Vec<Vout> = Vec::deserialize(deserializer)?;
    let mut addresses: Vec<String> = vec![];
    for vout in vouts {
        if let Some(vout_addresses) = vout.script_pub_key.and_then(|s| s.addresses) {
            addresses.extend(vout_addresses);
        }
    }
    Ok(addresses)
}

#[cfg(test)]
pub mod test {
    use super::*;

    pub fn get_test_blocks() -> Vec<Block> {
        vec![
            Block {
                txs: vec![Tx {
                    txid: "txid1".to_owned(),
                    addresses: vec!["address1".to_owned(), "address2".to_owned()],
                }],
            },
            Block {
                txs: vec![Tx {
                    txid: "txid2".to_owned(),
                    addresses: vec!["address1".to_owned(), "address4".to_owned()],
                }],
            },
            Block {
                txs: vec![Tx {
                    txid: "txid3".to_owned(),
                    addresses: vec!["address1".to_owned(), "address5".to_owned()],
                }],
            },
        ]
    }

    #[test]
    fn test_deserialization() -> Result<(), Box<dyn std::error::Error>> {
        let block: Block = serde_json::from_str(
            r#"
{
    "tx": [
        {
            "txid": "123",
            "vout": [
                {

                    "scriptPubKey": {
                        "addresses": ["Address1"]
                    }
                },
                {
                    "scriptPubKey": {
                        "addresses": ["Address2"]
                    }
                }
            ]
        },
        {
            "txid": "456",
            "vout": [
                {
                },
                {
                    "scriptPubKey": {
                        "addresses": ["Address3"]
                    }
                },
                {
                    "scriptPubKey": {
                        "addresses": ["Address4", "Address5"]
                    }
                }
            ]
        }
    ]
}
"#,
        )?;
        assert_eq!(block.txs.len(), 2);
        assert_eq!(block.txs[0].txid, "123");
        assert_eq!(block.txs[1].txid, "456");
        assert_eq!(block.txs[0].addresses, vec!["Address1", "Address2"]);
        assert_eq!(
            block.txs[1].addresses,
            vec!["Address3", "Address4", "Address5"]
        );
        Ok(())
    }
}
