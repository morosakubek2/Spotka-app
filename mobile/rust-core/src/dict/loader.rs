// mobile/rust-core/src/dict/loader.rs
// Dictionary Loader & Merger for Multi-language Support.
// Architecture: Official Dictionaries take precedence over Custom ones.
// Indices are managed to avoid collisions between Official and Custom sets.
// Year: 2026 | Rust Edition: 2024

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use log::{info, warn};

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
    pub language: String, // e.g., "en", "pl", "eo"
    pub version: u32,
    pub entries: Vec<DictEntryJson>,
}

/// Internal representation of a dictionary entry with assigned index.
#[derive(Debug, Clone)]
pub struct DictEntry {
    pub word: String,
    pub category: String,
    pub freq: u32,
    pub index: u8,        // The compressed index (0-254)
    pub is_custom: bool,  // Flag to distinguish source
}

/// Error keys for translation (Language Agnostic).
#[derive(Debug, PartialEq)]
pub enum DictError {
    InvalidJson,
    VersionMismatch,
    EmptyDictionary,
    IndexOverflow, // More than 255 unique tags
}

impl std::fmt::Display for DictError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DictError::InvalidJson => write!(f, "ERR_DICT_INVALID_JSON"),
            DictError::VersionMismatch => write!(f, "ERR_DICT_VERSION_MISMATCH"),
            DictError::EmptyDictionary => write!(f, "ERR_DICT_EMPTY"),
            DictError::IndexOverflow => write!(f, "ERR_DICT_INDEX_OVERFLOW"),
        }
    }
}

/// Main Dictionary Manager.
/// Holds the merged map of words to entries.
pub struct DictionaryManager {
    // Map: Word (lowercase) -> Entry
    entries: HashMap<String, DictEntry>,
    // Reverse Map: Index -> Word (for decompression/display)
    index_map: HashMap<u8, String>,
    language: String,
    max_official_index: u8,
}

impl DictionaryManager {
    pub fn new() -> Self {
        DictionaryManager {
            entries: HashMap::new(),
            index_map: HashMap::new(),
            language: String::new(),
            max_official_index: 0,
        }
    }

    /// Loads and merges an OFFICIAL dictionary.
    /// Official entries get indices starting from 1.
    /// Overwrites any existing entries (custom or official) with the same word.
    pub fn load_official(&mut self, json_content: &str, expected_version: u32) -> Result<(), DictError> {
        let file: DictFile = serde_json::from_str(json_content)
            .map_err(|_| DictError::InvalidJson)?;

        if file.version != expected_version {
            // In strict mode, we might reject. Here we just warn or accept depending on policy.
            // For Alpha, let's be strict.
            return Err(DictError::VersionMismatch);
        }

        if file.entries.is_empty() {
            return Err(DictError::EmptyDictionary);
        }

        self.language = file.language.clone();
        let mut current_index: u8 = 1; // Start official indices from 1

        // Sort by frequency to assign lower indices to more common words (optimization)
        let mut sorted_entries = file.entries;
        sorted_entries.sort_by(|a, b| b.freq.cmp(&a.freq));

        for entry_json in sorted_entries {
            let word_key = entry_json.word.to_lowercase();
            
            // Assign index
            if current_index == 255 { // Reserve 255 for special use/overflow
                return Err(DictError::IndexOverflow);
            }

            let entry = DictEntry {
                word: entry_json.word, // Keep original casing for display
                category: entry_json.category,
                freq: entry_json.freq,
                index: current_index,
                is_custom: false,
            };

            self.entries.insert(word_key.clone(), entry);
            self.index_map.insert(current_index, word_key);
            
            current_index += 1;
        }

        self.max_official_index = current_index - 1;
        info!("MSG_DICT_OFFICIAL_LOADED: {} entries", self.entries.len());
        Ok(())
    }

    /// Loads and merges a CUSTOM dictionary.
    /// Custom entries DO NOT overwrite official ones.
    /// Indices start after the last official index.
    pub fn load_custom(&mut self, json_content: &str) -> Result<(), DictError> {
        let file: DictFile = serde_json::from_str(json_content)
            .map_err(|_| DictError::InvalidJson)?;

        let mut current_index: u8 = self.max_official_index + 1;

        for entry_json in file.entries {
            let word_key = entry_json.word.to_lowercase();

            // Skip if word already exists in official dictionary (Priority Rule)
            if self.entries.contains_key(&word_key) {
                continue; 
            }

            if current_index == 255 {
                warn!("MSG_DICT_CUSTOM_INDEX_LIMIT_REACHED");
                break; 
            }

            let entry = DictEntry {
                word: entry_json.word,
                category: entry_json.category,
                freq: entry_json.freq,
                index: current_index,
                is_custom: true,
            };

            self.entries.insert(word_key.clone(), entry);
            self.index_map.insert(current_index, word_key);
            current_index += 1;
        }

        info!("MSG_DICT_CUSTOM_MERGED");
        Ok(())
    }

    /// Retrieves an entry by word (case-insensitive).
    pub fn get_entry(&self, word: &str) -> Option<&DictEntry> {
        self.entries.get(&word.to_lowercase())
    }

    /// Retrieves a word by its index (for decompression).
    pub fn get_word_by_index(&self, index: u8) -> Option<&String> {
        self.index_map.get(&index)
    }

    /// Checks if a word exists in the dictionary.
    pub fn contains(&self, word: &str) -> bool {
        self.entries.contains_key(&word.to_lowercase())
    }

    pub fn language(&self) -> &str {
        &self.language
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_official_and_priority() {
        let mut manager = DictionaryManager::new();
        
        let official_json = r#"{"language": "en", "version": 1, "entries": [
            {"word": "Kino", "category": "fun", "freq": 100},
            {"word": "Sport", "category": "act", "freq": 90}
        ]}"#;

        manager.load_official(official_json, 1).unwrap();

        // Verify case insensitivity
        assert!(manager.contains("kino"));
        assert!(manager.contains("KINO"));
        
        // Verify index assignment (sorted by freq: Kino=1, Sport=2)
        assert_eq!(manager.get_entry("kino").unwrap().index, 1);
        assert_eq!(manager.get_entry("sport").unwrap().index, 2);

        // Load custom with overlapping word
        let custom_json = r#"{"language": "en", "version": 1, "entries": [
            {"word": "Kino", "category": "custom", "freq": 10}, // Should be ignored
            {"word": "Theater", "category": "fun", "freq": 50}
        ]}"#;

        manager.load_custom(custom_json).unwrap();

        // "Kino" should still be official (not custom)
        assert_eq!(manager.get_entry("kino").unwrap().is_custom, false);
        
        // "Theater" should be added with index > max_official (which is 2)
        assert_eq!(manager.get_entry("theater").unwrap().index, 3);
        assert_eq!(manager.get_entry("theater").unwrap().is_custom, true);
    }
}
