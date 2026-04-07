use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockHeader {
    pub height: u64,
    pub prev_hash: String,
    pub merkle_root: String,
    pub timestamp: u64,
    pub validator_id: String, // Hash numeru telefonu walidatora
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub signature: Vec<u8>,
}

impl Block {
    pub fn calculate_hash(&self) -> String {
        let mut hasher = Sha256::new();
        let data = bincode::serialize(&self.header).unwrap();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    pub fn new(height: u64, prev_hash: String, txs: Vec<Transaction>, validator: String) -> Self {
        let merkle_root = Self::calculate_merkle_root(&txs);
        let header = BlockHeader {
            height,
            prev_hash,
            merkle_root,
            timestamp: chrono::Utc::now().timestamp() as u64,
            validator_id: validator,
        };
        Block {
            header,
            transactions: txs,
            signature: vec![], // Podpis dodawany później
        }
    }

    fn calculate_merkle_root(txs: &[Transaction]) -> String {
        if txs.is_empty() {
            return Sha256::digest("empty").to_string();
        }
        // Uproszczona implementacja Merkle Root
        let mut hashes: Vec<String> = txs.iter().map(|t| {
            let h = Sha256::digest(bincode::serialize(t).unwrap());
            hex::encode(h)
        }).collect();
        
        while hashes.len() > 1 {
            let mut new_hashes = Vec::new();
            for chunk in hashes.chunks(2) {
                let combined = format!("{}{}", chunk[0], chunk.get(1).unwrap_or(&chunk[0]));
                let h = Sha256::digest(combined.as_bytes());
                new_hashes.push(hex::encode(h));
            }
            hashes = new_hashes;
        }
        hashes[0].clone()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TxType {
    TrustIssue,
    TrustRevoke,
    RepUpdate,
    MeetupMeta,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub tx_type: TxType,
    pub payload_hash: String,
    pub raw_data: Vec<u8>,
    pub signature: Vec<u8>,
}
