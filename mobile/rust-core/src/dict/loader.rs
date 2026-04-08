// mobile/rust-core/src/dict/loader.rs
// Dictionary Loader & Merger for Multi-language Support.
// Architecture: Official Dictionaries take precedence over Custom ones.
// Indices are managed to avoid collisions between Official and Custom sets.
// Year: 2026 | Rust Edition: 2024

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use log::{info, warn};

/// Manifest for remote dictionary management (versioning, hash verification).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DictManifest {
    pub language: String,
    pub version: u32,
    pub entry_count: usize,
    pub content_hash: String, // SHA-256 of the JSON content to verify integrity
}

/// Structure of a single entry in the JSON dictionary file.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DictEntryJson {
    pub word: String,
    pub category: String,
    #[serde(default)]
    pub freq: u32,
}

/// Structure of the entire JSON dictionary file.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DictFile {
    pub language: String,
    pub version: u32,
    pub entries: Vec<DictEntryJson>,
}

/// Source of the dictionary entry (Official vs Custom).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DictSource {
    Official,
    Custom,
}

/// Internal representation of a dictionary entry with assigned index.
#[derive(Debug, Clone)]
pub struct DictEntry {
    pub word: String,
    pub category: String,
    pub freq: u32,
    pub index: u8,
    pub source: DictSource,
}

/// Error keys for translation (Language Agnostic).
#[derive(Debug, PartialEq)]
pub enum DictError {
    InvalidJson,
    VersionMismatch,
    EmptyDictionary,
    IndexOverflow,
    MergeFailed,
}

impl std::fmt::Display for DictError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DictError::InvalidJson => write!(f, "ERR_DICT_INVALID_JSON"),
            DictError::VersionMismatch => write!(f, "ERR_DICT_VERSION_MISMATCH"),
            DictError::EmptyDictionary => write!(f, "ERR_DICT_EMPTY"),
            DictError::IndexOverflow => write!(f, "ERR_DICT_INDEX_OVERFLOW"),
            DictError::MergeFailed => write!(f, "ERR_DICT_MERGE_FAILED"),
        }
    }
}

/// Main Dictionary Manager.
/// Holds the merged map of words to entries.
pub struct DictionaryLoader { // Renamed from DictionaryManager to match mod.rs usage better
    entries: HashMap<String, DictEntry>,
    index_map: HashMap<u8, String>,
    language: String,
    max_official_index: u8,
}

impl DictionaryLoader {
    pub fn new() -> Self {
        DictionaryLoader {
            entries: HashMap::new(),
            index_map: HashMap::new(),
            language: String::new(),
            max_official_index: 0,
        }
    }

    /// Loads an OFFICIAL dictionary.
    /// Overwrites existing entries. Indices start from 1.
    pub fn load_from_json_official(&mut self, json_content: &str, expected_version: u32) -> Result<(), DictError> {
        let file: DictFile = serde_json::from_str(json_content)
            .map_err(|_| DictError::InvalidJson)?;

        if file.version != expected_version {
            return Err(DictError::VersionMismatch);
        }

        if file.entries.is_empty() {
            return Err(DictError::EmptyDictionary);
        }

        self.language = file.language.clone();
        self.max_official_index = 0; // Reset for official load
        self.entries.clear();
        self.index_map.clear();

        let mut current_index: u8 = 1;
        let mut sorted_entries = file.entries;
        sorted_entries.sort_by(|a, b| b.freq.cmp(&a.freq));

        for entry_json in sorted_entries {
            if current_index == 255 {
                return Err(DictError::IndexOverflow);
            }

            let word_key = entry_json.word.to_lowercase();
            let entry = DictEntry {
                word: entry_json.word,
                category: entry_json.category,
                freq: entry_json.freq,
                index: current_index,
                source: DictSource::Official,
            };

            self.entries.insert(word_key.clone(), entry);
            self.index_map.insert(current_index, word_key);
            current_index += 1;
        }

        self.max_official_index = current_index - 1;
        info!("MSG_DICT_OFFICIAL_LOADED: {} entries", self.entries.len());
        Ok(())
    }

    /// Merges a CUSTOM dictionary into the current one.
    /// Does NOT overwrite official entries. Indices continue after max_official_index.
    pub fn merge_custom(&mut self, other: &mut DictionaryLoader) -> Result<(), DictError> {
        let mut current_index: u8 = self.max_official_index + 1;
        let mut merged_count = 0;

        // Iterate over other's entries (assuming 'other' is loaded as custom or temporary)
        // We need to access other.entries directly. 
        // Note: In a real scenario, 'other' might be a temporary loader just for parsing.
        for (_, entry_json_like) in other.entries.drain() {
            let word_key = entry_json_like.word.to_lowercase();

            // Priority Rule: Skip if official exists
            if self.entries.contains_key(&word_key) {
                continue;
            }

            if current_index == 255 {
                warn!("MSG_DICT_CUSTOM_INDEX_LIMIT_REACHED");
                break;
            }

            let new_entry = DictEntry {
                word: entry_json_like.word,
                category: entry_json_like.category,
                freq: entry_json_like.freq,
                index: current_index,
                source: DictSource::Custom,
            };

            self.entries.insert(word_key.clone(), new_entry);
            self.index_map.insert(current_index, word_key);
            current_index += 1;
            merged_count += 1;
        }

        info!("MSG_DICT_CUSTOM_MERGED: {} new entries", merged_count);
        Ok(())
    }

    /// Helper to load from JSON specifically for merging (parses into self temporarily)
    pub fn load_from_json(&mut self, json_content: &str) -> Result<(), DictError> {
        // This is a simplified parser used by GlobalDictManager before deciding merge strategy
        // For the purpose of 'merge_custom', we assume the caller parses into a temp DictionaryLoader
        // and then calls merge_custom. 
        // However, to support the flow in mod.rs where we call load_from_json then merge:
        // Let's assume this method populates 'self' as a temporary container if called on a new instance.
        let file: DictFile = serde_json::from_str(json_content)
            .map_err(|_| DictError::InvalidJson)?;
        
        self.language = file.language;
        for entry_json in file.entries {
             let word_key = entry_json.word.to_lowercase();
             let entry = DictEntry {
                word: entry_json.word,
                category: entry_json.category,
                freq: entry_json.freq,
                index: 0, // Temporary, will be reassigned during merge
                source: DictSource::Custom,
            };
            self.entries.insert(word_key, entry);
        }
        Ok(())
    }

    /// Retrieves the static index for a word. Returns None if not found or is custom (depending on policy).
    /// Here we return index regardless of source, but caller can check source via get_entry.
    pub fn get_static_index(&self, word: &str) -> Option<u8> {
        self.entries.get(&word.to_lowercase()).map(|e| e.index)
    }

    /// Retrieves full entry metadata.
    pub fn get_entry(&self, word: &str) -> Option<&DictEntry> {
        self.entries.get(&word.to_lowercase())
    }

    /// Retrieves word by index (for decompression).
    pub fn get_word_by_index(&self, index: u8) -> Option<&String> {
        self.index_map.get(&index)
    }

    /// Gets top N frequent words for session pre-loading.
    pub fn get_top_frequent(&self, n: usize) -> Vec<String> {
        let mut all_entries: Vec<&DictEntry> = self.entries.values().collect();
        all_entries.sort_by(|a, b| b.freq.cmp(&a.freq));
        
        all_entries.into_iter()
            .take(n)
            .map(|e| e.word.clone())
            .collect()
    }

    pub fn language(&self) -> &str {
        &self.language
    }
    
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl Default for DictionaryLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_official_priority_and_merge() {
        let mut official_loader = DictionaryLoader::new();
        let official_json = r#"{"language": "en", "version": 1, "entries": [
            {"word": "Kino", "category": "fun", "freq": 100}
        ]}"#;
        official_loader.load_from_json_official(official_json, 1).unwrap();

        let mut custom_loader = DictionaryLoader::new();
        let custom_json = r#"{"language": "en", "version": 1, "entries": [
            {"word": "Kino", "category": "custom", "freq": 10}, 
            {"word": "Theater", "category": "fun", "freq": 50}
        ]}"#;
        custom_loader.load_from_json(custom_json).unwrap();

        // Merge custom into official
        official_loader.merge_custom(&mut custom_loader).unwrap();

        // "Kino" must remain official (index 1)
        let kino = official_loader.get_entry("kino").unwrap();
        assert_eq!(kino.index, 1);
        assert_eq!(kino.source, DictSource::Official);

        // "Theater" must be custom (index 2)
        let theater = official_loader.get_entry("theater").unwrap();
        assert_eq!(theater.index, 2);
        assert_eq!(theater.source, DictSource::Custom);
    }
}
