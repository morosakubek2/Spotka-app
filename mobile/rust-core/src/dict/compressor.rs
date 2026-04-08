// mobile/rust-core/src/dict/compressor.rs
// Adaptive Dictionary Compression for P2P Sessions.
// Features: Session Negotiation, Frequency Tracking, Status Preservation, Static Dict Pre-loading.
// Security: Memory Safe, Error Keys only.
// Year: 2026 | Rust Edition: 2024

use crate::dict::cts_parser::{CtsTag, TagStatus};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use zeroize::Zeroize; // For secure memory clearing

/// Threshold for promoting a text tag to a dynamic index.
const FREQUENCY_PROMOTION_THRESHOLD: u32 = 3;

/// Represents a compressed or uncompressed tag payload.
/// Includes status to ensure full reconstruction is possible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedTagPayload {
    pub status: TagStatus,
    pub data: TagData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TagData {
    Text(String),
    Index(u8),
}

/// Manages the dynamic dictionary for a specific P2P session.
pub struct SessionDictionary {
    word_to_index: HashMap<String, u8>,
    index_to_word: HashMap<u8, String>,
    usage_stats: HashMap<String, u32>,
    next_dynamic_index: u8,
}

impl SessionDictionary {
    pub fn new() -> Self {
        SessionDictionary {
            word_to_index: HashMap::new(),
            index_to_word: HashMap::new(),
            usage_stats: HashMap::new(),
            next_dynamic_index: 10, 
        }
    }

    /// Pre-loads static indices from a global dictionary (Official/Custom).
    /// This boosts compression efficiency immediately upon session start.
    pub fn preload_static_dict(&mut self, static_entries: Vec<(String, u8)>) {
        for (word, idx) in static_entries {
            // Only insert if not already present (dynamic takes precedence if collision, though unlikely)
            if !self.word_to_index.contains_key(&word) && idx < 10 {
                self.word_to_index.insert(word.clone(), idx);
                self.index_to_word.insert(idx, word);
            }
        }
        log::info!("MSG_DICT_STATIC_PRELOADED: {} entries", static_entries.len());
    }

    /// Records usage and promotes to index if threshold met.
    pub fn record_usage(&mut self, word: &str) -> Option<u8> {
        if let Some(&idx) = self.word_to_index.get(word) {
            *self.usage_stats.entry(word.to_string()).or_insert(0) += 1;
            return Some(idx);
        }

        let count = self.usage_stats.entry(word.to_string()).or_insert(0);
        *count += 1;

        if *count >= FREQUENCY_PROMOTION_THRESHOLD && self.next_dynamic_index <= 255 {
            let new_index = self.next_dynamic_index;
            self.next_dynamic_index += 1;
            self.word_to_index.insert(word.to_string(), new_index);
            self.index_to_word.insert(new_index, word.to_string());
            return Some(new_index);
        }
        None
    }

    /// Compresses tags preserving their STATUS.
    pub fn compress_tags(&mut self, tags: &[CtsTag]) -> Vec<CompressedTagPayload> {
        tags.iter()
            .map(|tag| {
                let data = if let Some(idx) = self.record_usage(&tag.word) {
                    TagData::Index(idx)
                } else {
                    TagData::Text(tag.word.clone())
                };
                CompressedTagPayload {
                    status: tag.status.clone(),
                    data,
                }
            })
            .collect()
    }

    /// Decompresses tags restoring their original STATUS.
    pub fn decompress_tags(&self, compressed: &[CompressedTagPayload]) -> Result<Vec<CtsTag>, &'static str> {
        let mut result = Vec::with_capacity(compressed.len());

        for item in compressed {
            let word = match &item.data {
                TagData::Index(idx) => {
                    self.index_to_word.get(idx)
                        .cloned()
                        .ok_or("ERR_UNKNOWN_COMPRESSION_INDEX")?
                },
                TagData::Text(word) => word.clone(),
            };

            result.push(CtsTag {
                status: item.status.clone(),
                word,
                index: match &item.data {
                    TagData::Index(idx) => Some(*idx),
                    TagData::Text(_) => None,
                },
            });
        }
        Ok(result)
    }

    pub fn export_for_sync(&self) -> Vec<(u8, String)> {
        self.index_to_word.iter().map(|(&k, v)| (k, v.clone())).collect()
    }

    pub fn import_sync(&mut self, entries: Vec<(u8, String)>) {
        for (idx, word) in entries {
            if !self.index_to_word.contains_key(&idx) {
                self.index_to_word.insert(idx, word.clone());
                self.word_to_index.insert(word, idx);
            }
        }
    }
}

impl Drop for SessionDictionary {
    fn drop(&mut self) {
        // Clear maps to zeroize memory (though strings are not secrets, it's good practice)
        self.word_to_index.clear();
        self.index_to_word.clear();
        self.usage_stats.clear();
    }
}

pub fn estimate_savings(original: &[CtsTag], compressed: &[CompressedTagPayload]) -> f32 {
    let original_size: usize = original.iter().map(|t| t.word.len() + 2).sum();
    let compressed_size: usize = compressed.iter().map(|c| match &c.data {
        TagData::Index(_) => 2, // 1 byte index + 1 byte status
        TagData::Text(s) => s.len() + 2,
    }).sum();

    if original_size == 0 { return 0.0; }
    ((original_size - compressed_size) as f32 / original_size as f32) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_preservation() {
        let mut dict = SessionDictionary::new();
        let tags = vec![
            CtsTag { status: TagStatus::Positive, word: "kino".to_string(), index: None },
            CtsTag { status: TagStatus::Negative, word: "deszcz".to_string(), index: None },
            CtsTag { status: TagStatus::Mediating, word: "przyjaciele".to_string(), index: None },
        ];

        let compressed = dict.compress_tags(&tags);
        
        assert_eq!(compressed[0].status, TagStatus::Positive);
        assert_eq!(compressed[1].status, TagStatus::Negative);
        assert_eq!(compressed[2].status, TagStatus::Mediating);

        let decompressed = dict.decompress_tags(&compressed).unwrap();
        assert_eq!(decompressed[1].status, TagStatus::Negative);
        assert_eq!(decompressed[1].word, "deszcz");
    }
}
