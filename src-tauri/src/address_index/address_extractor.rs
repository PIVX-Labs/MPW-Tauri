use super::block_source::BlockSource;
use super::types::{Block, Tx};
use crate::error::PIVXErrors;
use futures::stream::Stream;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::prelude::*;
use std::io::Cursor;
use std::path::PathBuf;
use std::pin::Pin;

pub struct AddressExtractor;

impl AddressExtractor {
    pub fn read_varint<T>(byte_source: &mut T) -> crate::error::Result<u64>
    where
        T: Read,
    {
        let mut first_byte = [0u8; 1];
        byte_source.read_exact(&mut first_byte)?;

        let value = match first_byte[0] {
            0x00..=0xFC => first_byte[0] as u64,
            0xFD => {
                let mut buf = [0u8; 2];
                byte_source.read_exact(&mut buf)?;
                u16::from_le_bytes(buf) as u64
            }
            0xFE => {
                let mut buf = [0u8; 4];
                byte_source.read_exact(&mut buf)?;
                u32::from_le_bytes(buf) as u64
            }
            0xFF => {
                let mut buf = [0u8; 8];
                byte_source.read_exact(&mut buf)?;
                u64::from_le_bytes(buf) as u64
            }
            _ => return Err(crate::error::PIVXErrors::InvalidVarInt),
        };

        Ok(value)
    }

    fn double_sha256(data: &[u8]) -> Vec<u8> {
        let first_hash = Sha256::digest(data);
        let second_hash = Sha256::digest(&first_hash);
        second_hash.to_vec()
    }

    fn get_address_from_pubkey_hash(pubkey_hash: &[u8]) -> Option<String> {
        if pubkey_hash.len() != 20 {
            return None;
        }
        let mut address = vec![30];
        address.extend_from_slice(pubkey_hash);
        let checksum = Self::double_sha256(&address);
        address.extend_from_slice(&checksum[0..4]);
        Some(bs58::encode(&address).into_string())
    }

    pub fn get_address_from_p2pkh<T>(byte_source: &mut T) -> Option<String>
    where
        T: Read + Seek,
    {
        let mut script_bytes = [0u8; 25];
        byte_source.read_exact(&mut script_bytes).ok()?;
        if script_bytes[0] == 0x76  // OP_DUP
        && script_bytes[1] == 0xa9  // OP_HASH160
        && script_bytes[2] == 0x14  // Push 20 bytes
        && script_bytes[23] == 0x88 // OP_EQUALVERIFY
        && script_bytes[24] == 0xac
        // OP_CHECKSIG
        {
            Self::get_address_from_pubkey_hash(&script_bytes[3..23])
        } else {
            None
        }
    }

    pub fn get_address_from_p2cs<T>(byte_source: &mut T) -> Option<String>
    where
        T: Read + Seek,
    {
        let mut script_bytes = [0u8; 51];
        byte_source.read_exact(&mut script_bytes).ok()?;
        if script_bytes[0] == 0x76  // OP_DUP
            && script_bytes[1] == 0xa9  // OP_HASH160
            && script_bytes[2] == 0x7b  // OP_ROT
            && script_bytes[3] == 0x63 // OP_IF
            && (script_bytes[4] == 0xd1 || script_bytes[4] == 0xd2) // OP_CHECKCOLDSTAKEVERIFY_LOF
            && script_bytes[5] == 20 // first address length
            && script_bytes[26] == 0x67 // OP_ELSE
	    && script_bytes[27] == 20 // second address length
	    && script_bytes[48] == 0x68 // OP_ENDIF
	    && script_bytes[49] == 0x88 // OP_EQUALVERIFY
	    && script_bytes[50] == 0xac
        // OP_CHECKSIG
        {
            Self::get_address_from_pubkey_hash(&script_bytes[28..48])
        } else {
            None
        }
    }

    pub fn get_addresses_from_tx<T>(byte_source: &mut T) -> crate::error::Result<(Tx, bool)>
    where
        T: Read + Seek,
    {
        let start = byte_source.stream_position()?;
        let mut buff4 = [0u8; 4];
        // version (4)
        byte_source.read_exact(&mut buff4)?;
        let version = u32::from_le_bytes(buff4);
        let has_sapling_data = version >= 3;

        // Vin length (varint)
        let vin_length = Self::read_varint(byte_source)?;
        for _ in 0..vin_length {
            // txid (32) + n (4)
            byte_source.read_exact(&mut [0u8; 36])?;
            // script length (varint)
            let script_length = Self::read_varint(byte_source)?;
            // script + sequence (4)
            byte_source.seek_relative((script_length as i64) + 4)?;
        }

        let vout_length = Self::read_varint(byte_source)?;
        let mut addresses = vec![];
        let mut first_vout_empty = false;
        for i in 0..vout_length {
            // value (8)
            byte_source.read_exact(&mut [0u8; 8])?;
            let script_length = Self::read_varint(byte_source)?;
            if i == 0 {
                first_vout_empty = script_length == 0;
            }
            let mut script = vec![0u8; script_length as usize];
            byte_source.read_exact(&mut script)?;
            if let Some(address) = Self::get_address_from_p2pkh(&mut Cursor::new(&script)) {
                addresses.push(address);
            } else if let Some(address) = Self::get_address_from_p2cs(&mut Cursor::new(&script)) {
                addresses.push(address);
            }
        }
        // locktime
        byte_source.read_exact(&mut [0u8; 4])?;
        if has_sapling_data {
            let mut has_sapling_data = [0u8; 1];
	    byte_source.read_exact(&mut has_sapling_data)?;
            if has_sapling_data[0] >= 1 {
                // value balance
                byte_source.read_exact(&mut [0u8; 8])?;
                // shield spend len
                let spend_len = Self::read_varint(byte_source)?;
                for i in 0..spend_len {
                    // cv (32) + anchor (32) + nullifier (32) + rk (32) + proof(192) + spendAuthSig (64)
                    byte_source.read_exact(&mut [0u8; 384])?;
                }

                let output_len = Self::read_varint(byte_source)?;
                for _ in 0..output_len {
                    // cv (32) + cmu (32) + ephemeralKey (32) + encCiphertext (580) + outCiphertext (80) + proof (192)
                    byte_source.read_exact(&mut [0u8; 948])?;
                }
		// Binding sig (64)
		byte_source.read_exact(&mut [0u8; 64])?;
            }
        }
        let end = byte_source.stream_position()?;
        byte_source.seek(std::io::SeekFrom::Start(start))?;
        let mut tx_bytes = vec![0u8; (end - start) as usize];
        byte_source.read_exact(&mut tx_bytes)?;
        let mut txid_bytes = Self::double_sha256(&tx_bytes);
        txid_bytes.reverse();

        let txid = hex::encode(txid_bytes);
        Ok((Tx { txid, addresses }, first_vout_empty))
    }

    pub fn get_addresses_from_block<T>(byte_source: &mut T) -> crate::error::Result<Block>
    where
        T: Read + Seek,
    {
        let start = byte_source.stream_position()?;
        let mut buff4 = [0u8; 4];
        // magic
        byte_source.read_exact(&mut buff4)?;
        let magic = u32::from_le_bytes(buff4);
        if magic != 0xe9fdc490 {
            println!("WARNING: magic is wrong!!! {:x}", magic);
            //panic!("what the fuck?");
	    if magic == 0 {
		return Err(PIVXErrors::InvalidVarInt)
	    }
            return Err(PIVXErrors::InvalidBlock);
        }
        // size
        byte_source.read_exact(&mut buff4)?;
        let size = u32::from_le_bytes(buff4);
        // version
        byte_source.read_exact(&mut buff4)?;
        let version = u32::from_le_bytes(buff4);

        // hash block (32) + hash merkle root (32) + time (4) + diff (4) + nonce (4)
        byte_source.read_exact(&mut [0u8; 76])?;
        // sapling/zercoin hash (32), only if version > 3 and != 7
        if version > 3 && version != 7 {
            byte_source.read_exact(&mut [0u8; 32])?;
        }
        // tx length (varint)
        let mut txs = Self::read_varint(byte_source)?;
        let mut block = Block { txs: vec![] };

        let mut is_proof_of_stake = false;
        for i in 0..txs {
            let (tx, first_vout_empty) = Self::get_addresses_from_tx(byte_source)?;
            if i == 1 && first_vout_empty {
                is_proof_of_stake = true;
            }
            if tx.addresses.len() > 0 {
                block.txs.push(tx);
            }
        }

        if is_proof_of_stake {
            let block_sig_size = Self::read_varint(byte_source)?;
            byte_source.seek_relative(block_sig_size as i64)?;
        }
        //byte_source.seek(std::io::SeekFrom::Start((start + size as u64)))?;

        Ok(block)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_gets_addresses_from_blocks() -> crate::error::Result<()> {
        // Block 4569426. This needs to be updated when cold stake is implemented
        let res = AddressExtractor::get_addresses_from_block(&mut Cursor::new(hex::decode("90c4fde90bc490e9fd00000078fc7b650cf71367dd5cb67c3ad81b7190606a30fe4c1a39f23f1be75915bcedf4b0b32a13cf38433c9ba6ff2141578163c524ff63f3245cff025201936e1dbf304cef6629b0051b00000000c718660c09f599491465a6c2f88134411622456d0ca3b32bd757295a32dc864f0401000000010000000000000000000000000000000000000000000000000000000000000000ffffffff050352b94500ffffffff010000000000000000000000000001000000019b3792f50d76bdc2c7385f9b19b3e2ce362690b7a32fb69bda598ce1e068b76d010000006b483045022100f9c88b20e9dddac557bab892bb22be0ed0f9c0b82c3595d49a0071e62735036502201861001a0730a4c07ed5679cc064bb8b1a9256851ef160f06f6d858e87edf91501210215912e6a40c2457b95ddc37ee592d3f4bc2ffc1e494861fbb3374e2edc414e1affffffff03000000000000000000fd80f269110000001976a9145b589b431bc3b563de290426ce74a70b6cc8c26b88ac0046c323000000001976a914361caa73d876ec846f8f0d828b6e708f779d40ce88ac0000000001000000036e54251ab6f582d15c9d25829595982e9d9553a640a29c55e2577ebc567c5867010000006b47304402204bc21a490bd44a3cab39370822ff7cd2909935d41379e32aa17eb0307ed88b9a02203c34cd909c05e48fc01aa70a55b6fbc1d32218117e65b8f37b5436f98462be500100210311b85ed73eafceae37768a64e949d45dfd6bbf9c76d02399b4bbddcdbcd9d12affffffff46fbb49a6ba1bccae262764844fa448c9230dff048aa5b43b91b8f8ed5130ea9010000006c48304502210099d4359fb1ef7cc4da4177bcefb5d04dcdf5c8f8b1441c66fc613b176b40df5802200f7e9e4b9039970e9ef6344d84129e580f48cd6b4cb210fd72e59f8401122f1601002102610b26882bd065acf25cd5933b8ce3cf8c498e9998f61a2ca556d5e04f70940dffffffff593064def9102f2485e375f63e2b40d6c64bf913fd6c754bcbdae2261d8d9948010000006c483045022100fce38cae4ab9391dd30bfa16a8e5e48dbad51258bcc9365cfe214eac716e112a02203a00cf191d4dd0e33449969ba1aac7eb78aa3c8fb087f62870e148f8c94b7ad9010021027dadc650c0c4adfc5775c6cdc428e0ac6d12b038acf3b7ac429fe209b2f14b22ffffffff02008b585a170000001976a914611f84583fd9ccd8cf31d28448f46a95775c9d9088acdfaa0f840d0000003376a97b63d114b3be8567d0190c67ca4675a0019089c55fe695f96714611f84583fd9ccd8cf31d28448f46a95775c9d906888ac0000000001000000013f655c5b6d8c72c7c662c93a2144102f1b007acf34f40e70455ad443fc50f775000000006a473044022064059991cb1438516a8096ac158541416ca3db1d5fe07ef17173bd23791333e8022064f2d857d1a16dc8b7f11e2afd284bcbe124a08f73e1d9d1469171c9e918c19d01210288b6c831518cdc28194d92e8ffebca838b24b2a6b301ccda68860cbd4b7f449cffffffff017683585a170000001976a91474697912927e514e3d37d514adb62c94f22fbd2e88ac00000000463044022063e58d0d91876b2c3d0f329bc67fc27b40bac3648a8fa6d66cd5c162f780a480022073a7202b19f175511687ff2c19801de7f1d863ee01086ce946233467746fc331").unwrap()))?;
        assert_eq!(res.txs.len(), 3);
        assert_eq!(
            res.txs[0].txid,
            "a6ce3a9ae6fc25a800c07e1eeff2d7b0af3bf29c4cbfd644628428a320a3edfb"
        );
        assert_eq!(
            res.txs[0].addresses,
            vec![
                "DDU6BCfxp2eGdQ5AuoyL4QQo6D4abms5qg",
                "DA5DNMVNnj9ZKRBkknCoCeEjP1AWoQDHqg",
            ]
        );
        assert_eq!(
            res.txs[1].txid, "75f750fc43d45a45700ef434cf7a001b2f1044213ac962c6c7728c6d5b5c653f",
            "DDzdqhm3pEkPXkNwgHZLQzC9VZMXpcRykz"
        );
        assert_eq!(
            res.txs[1].addresses,
            vec![
                "DDzdqhm3pEkPXkNwgHZLQzC9VZMXpcRykz",
                "DDzdqhm3pEkPXkNwgHZLQzC9VZMXpcRykz",
            ]
        );
        assert_eq!(
            res.txs[2].txid,
            "012ade19a1aa317fa045dbc68011e5ad454fd21b0b7802d3325b3f7e62ba18c0"
        );
        assert_eq!(
            res.txs[2].addresses,
            vec!["DFkdGu6nq6rU7gwhw3DP4X1W9YKf5nZji9",]
        );

        Ok(())
    }

    #[test]
    fn it_gets_addresses_from_tx() -> crate::error::Result<()> {
        let bytes = hex::decode("0100000001f3614aede6f8d2366f52d9244999d9b26ebe0d2a63c7b7b4a06a8a6ab3bae5ba010000006b483045022100e3ebd6ca51e3abbb24bace92831facac82429ef9e0568ddf908bf601b4829e7b02202aa7081c4d0fd0112af0961f6c7923c97515b874464ce22b3c52f07f258bb64001210298279c6bd14d9fa47ffc4b8c40213e62ed0579f765d845bfa0ce44ba8cf8d385ffffffff0300000000000000000000c9d7930c0000001976a91444536354065eb3393f0ab11938e09725c467841e88ac0046c323000000001976a9144735f642faf6d1ab83478bd2fdda86f4188368ba88ac00000000").unwrap();
        let (Tx { txid, addresses }, _) =
            AddressExtractor::get_addresses_from_tx(&mut Cursor::new(&bytes))?;
        assert_eq!(
            &txid,
            "d09c64e78a0bf8943dc503fbbb3f19cead23685b7e725b5dd5fb35b09bd119f6"
        );
        assert_eq!(
            addresses,
            vec![
                "DBNNPCiQu8JjESEoxuHCTXgNEw7Mk72wuW",
                "DBddADmxi5g4tKdTA5yxFdDhN6gd85v5hx"
            ]
        );
        Ok(())
    }

    #[test]
    fn it_gets_address_from_p2pkh() -> crate::error::Result<()> {
        let bytes = hex::decode("76a9145b589b431bc3b563de290426ce74a70b6cc8c26b88ac").unwrap();
        let address = AddressExtractor::get_address_from_p2pkh(&mut Cursor::new(&bytes));
        assert_eq!(address, Some("DDU6BCfxp2eGdQ5AuoyL4QQo6D4abms5qg".into()));
        Ok(())
    }

    #[test]
    fn it_gets_address_from_p2cs() -> crate::error::Result<()> {
        let bytes = hex::decode("76a97b63d114ea6a55bd6e5eeab8453ae897bf5be28a62465fcc67146bc3ffd106a5b56efba5f0da6dba47bca83b2bd76888ac").unwrap();
        let address = AddressExtractor::get_address_from_p2cs(&mut Cursor::new(&bytes));
        assert_eq!(address, Some("DExue43LyQduJzkUwFq53LfSppAzdRGWU2".into()));
        Ok(())
    }

    // #[test]
    fn it_gets_address_from_sapling_tx() -> crate::error::Result<()> {
        todo!()
    }

    #[test]
    fn it_gets_address_from_sapling_block() -> crate::error::Result<()> {
	let bytes = hex::decode(include_str!("test/sapling_block.hex")).unwrap();
	let block = AddressExtractor::get_addresses_from_block(&mut Cursor::new(bytes))?;
	assert_eq!(block.txs.len(), 2);
	assert_eq!(block.txs[0].txid, "3f64c3328bac6d5bb8c002a46cd767e367ef6f9dd2298ba04ca51c2ef4f0cc2c");
	assert_eq!(block.txs[0].addresses, vec!["DEjsj3jQWoJuMnGXi9B4h9gaGyBr5TrXFN", "DM2TWw1NvJ7sPxNXPZ8Cmn4DNGxYfa6yfX"]);
	assert_eq!(block.txs[1].txid, "997938165e83478f25bd14b203e492b9ce39a3979384527868f72fd394e68a45");
	assert_eq!(block.txs[1].addresses, vec!["DExue43LyQduJzkUwFq53LfSppAzdRGWU2"]);
	Ok(())
    }
}
