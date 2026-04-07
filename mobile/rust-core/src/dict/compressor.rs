// mobile/rust-core/src/dict/compressor.rs
// Adaptive Dictionary Compression for P2P Sessions.
// Features: Session Negotiation, Frequency Tracking, Fallback to Text.
// Year: 2026 | Rust Edition: 2024

use crate::dict::cts_parser::{CtsTag, TagStatus};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// Threshold for promoting a text tag to a dynamic index.
// If a word appears more than this times in a session, it gets an index.
const FREQUENCY_PROMOTION_THRESHOLD: u32 = 3;

/// Represents a compressed or uncompressed tag payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressedTag {
    /// Full text word (used for rare words or initial handshake)
    Text(String),
    /// Dynamic index (1 byte, 0-255) negotiated for this session
    Index(u8),
}

/// Manages the dynamic dictionary for a specific P2P session.
/// Each peer pair might have a slightly different dictionary based on their usage patterns.
pub struct SessionDictionary {
    // Map: Word -> Index
    word_to_index: HashMap<String, u8>,
    // Map: Index -> Word
    index_to_word: HashMap<u8, String>,
    // Map: Word -> Usage Count (for adaptive promotion)
    usage_stats: HashMap<String, u32>,
    // Next available dynamic index (reserved: 0-9 for static, 10-255 for dynamic)
    next_dynamic_index: u8,
}

impl SessionDictionary {
    pub fn new() -> Self {
        SessionDictionary {
            word_to_index: HashMap::new(),
            index_to_word: HashMap::new(),
            usage_stats: HashMap::new(),
            next_dynamic_index: 10, // Start dynamic indices after static range
        }
    }

    /// Records the usage of a word. If it becomes frequent enough, assigns a dynamic index.
    /// Returns the assigned index if promoted, or None if still text-based.
    pub fn record_usage(&mut self, word: &str) -> Option<u8> {
        // If already indexed, just return it
        if let Some(&idx) = self.word_to_index.get(word) {
            *self.usage_stats.entry(word.to_string()).or_insert(0) += 1;
            return Some(idx);
        }

        // Update stats
        let count = self.usage_stats.entry(word.to_string()).or_insert(0);
        *count += 1;

        // Promote if threshold met and we have space
        if *count >= FREQUENCY_PROMOTION_THRESHOLD && self.next_dynamic_index <= 255 {
            let new_index = self.next_dynamic_index;
            self.next_dynamic_index += 1;

            self.word_to_index.insert(word.to_string(), new_index);
            self.index_to_word.insert(new_index, word.to_string());
            
            // Log key for debugging (translated by UI later if needed)
            log::info!("MSG_DICT_PROMOTED_TO_INDEX: {} -> {}", word, new_index);
            return Some(new_index);
        }

        None
    }

    /// Attempts to compress a list of CtsTags into a compact byte stream representation.
    /// Returns a vector of CompressedTag enums.
    pub fn compress_tags(&mut self, tags: &[CtsTag]) -> Vec<CompressedTag> {
        tags.iter()
            .map(|tag| {
                if let Some(idx) = self.record_usage(&tag.word) {
                    CompressedTag::Index(idx)
                } else {
                    CompressedTag::Text(tag.word.clone())
                }
            })
            .collect()
    }

    /// Decompresses a list of CompressedTag back to CtsTag structs.
    /// Handles cases where an index might be unknown (fallback logic).
    pub fn decompress_tags(&self, compressed: &[CompressedTag]) -> Result<Vec<CtsTag>, &'static str> {
        let mut result = Vec::with_capacity(compressed.len());

        for item in compressed {
            match item {
                CompressedTag::Index(idx) => {
                    if let Some(word) = self.index_to_word.get(idx) {
                        result.push(CtsTag {
                            status: TagStatus::Positive, // Status must be preserved in transmission, simplified here
                            word: word.clone(),
                            index: Some(*idx),
                        });
                    } else {
                        // Critical Error: Unknown index received.
                        // In a real protocol, this would trigger a "Resync Dictionary" request.
                        log::warn!("ERR_UNKNOWN_COMPRESSION_INDEX: {}", idx);
                        return Err("ERR_UNKNOWN_COMPRESSION_INDEX");
                    }
                },
                CompressedTag::Text(word) => {
                    result.push(CtsTag {
                        status: TagStatus::Positive, // Simplified
                        word: word.clone(),
                        index: None,
                    });
                }
            }
        }
        Ok(result)
    }

    /// Exports the current dictionary state to send to a peer during handshake.
    /// Format: List of (Index, Word) pairs.
    pub fn export_for_sync(&self) -> Vec<(u8, String)> {
        self.index_to_word.iter().map(|(&k, v)| (k, v.clone())).collect()
    }

    /// Imports dictionary entries received from a peer.
    pub fn import_sync(&mut self, entries: Vec<(u8, String)>) {
        for (idx, word) in entries {
            // Only insert if we don't have a conflict, or overwrite if policy allows
            if !self.index_to_word.contains_key(&idx) {
                self.index_to_word.insert(idx, word.clone());
                self.word_to_index.insert(word, idx);
            }
        }
        log::info!("MSG_DICT_SYNC_COMPLETE: Imported {} entries", entries.len());
    }
}

/// Helper to estimate size savings (for metrics/debugging).
pub fn estimate_savings(original: &[CtsTag], compressed: &[CompressedTag]) -> f32 {
    let original_size: usize = original.iter().map(|t| t.word.len() + 2).sum(); // +2 for status/overhead
    let compressed_size: usize = compressed.iter().map(|c| match c {
        CompressedTag::Index(_) => 1, // 1 byte
        CompressedTag::Text(s) => s.len() + 2,
    }).sum();

    if original_size == 0 { return 0.0; }
    
    ((original_size - compressed_size) as f32 / original_size as f32) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_promotion() {
        let mut dict = SessionDictionary::new();
        let word = "coffee";
        
        // First 2 times: should be text
        assert!(dict.record_usage(word).is_none());
        assert!(dict.record_usage(word).is_none());
        
        // 3rd time: should promote to index
        let idx = dict.record_usage(word);
        assert!(idx.is_some());
        assert_eq!(idx.unwrap(), 10); // First dynamic index
        
        // Subsequent times: should return same index
        assert_eq!(dict.record_usage(word), Some(10));
    }

    #[test]
    fn test_roundtrip_compression() {
        let mut dict = SessionDictionary::new();
        // Force promotion manually for test stability
        dict.word_to_index.insert("beer".to_string(), 15);
        dict.index_to_word.insert(15, "beer".to_string());

        let tags = vec![
            CtsTag { status: TagStatus::Positive, word: "beer".to_string(), index: None },
            CtsTag { status: TagStatus::Negative, word: "vodka".to_string(), index: None }, // Rare word
        ];

        let compressed = dict.compress_tags(&tags);
        
        assert!(matches!(compressed[0], CompressedTag::Index(15)));
        assert!(matches!(compressed[1], CompressedTag::Text(_)));

        let decompressed = dict.decompress_tags(&compressed).unwrap();
        assert_eq!(decompressed[0].word, "beer");
        assert_eq!(decompressed[1].word, "vodka");
    }
}
