pub mod block;
pub mod transaction; // Placeholder
pub mod consensus;   // Placeholder
pub mod merkle;      // Placeholder

use crate::chain::block::{Block, Transaction};
use std::collections::HashMap;

pub struct AppChain {
    blocks: HashMap<u64, Block>,
    height: u64,
}

impl AppChain {
    pub fn new() -> Self {
        AppChain {
            blocks: HashMap::new(),
            height: 0,
        }
    }

    pub fn add_block(&mut self, block: Block) -> Result<(), &'static str> {
        // Prosta walidacja wysokości
        if block.header.height != self.height + 1 {
            return Err("ERR_INVALID_BLOCK_HEIGHT");
        }
        // Tu powinna być pełna walidacja konsensusu
        self.blocks.insert(block.header.height, block.clone());
        self.height = block.header.height;
        Ok(())
    }

    pub fn get_latest_hash(&self) -> Option<String> {
        self.blocks.get(&self.height).map(|b| b.calculate_hash())
    }
}
