// mobile/rust-core/src/dict/mod.rs
// Dictionary Module: Loading, Parsing (CTS), and Adaptive Compression.
// Architecture: Multi-language support, Official vs Custom priority, Session-based compression.
// Security: No hardcoded strings, all errors are keys.
// Year: 2026 | Rust Edition: 2024

pub mod cts_parser;
pub mod loader;
pub mod compressor;

// Re-export key types and functions for easier access in other modules
pub use cts_parser::{parse_cts, serialize_cts, CtsTag, TagStatus, CtsError};
pub use loader::{DictionaryLoader, DictEntry, DictSource, DictManifest};
pub use compressor::{SessionDictionary, CompressedTag, estimate_savings};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{info, warn, error};

/// Configuration for the Dictionary system.
pub struct DictConfig {
    /// Default threshold for promoting a word to a dynamic index in a session.
    pub promotion_threshold: u32,
    /// Maximum number of custom dictionaries a user can load simultaneously.
    pub max_custom_dicts: usize,
    /// Fallback language code if requested language is missing (e.g., "en").
    pub fallback_language: String,
}

impl Default for DictConfig {
    fn default() -> Self {
        DictConfig {
            promotion_threshold: 3,
            max_custom_dicts: 5,
            fallback_language: "en".to_string(),
        }
    }
}

/// Manages the global state of dictionaries (Official + Custom).
/// Thread-safe wrapper around multiple DictionaryLoaders.
/// Ensures Official dicts have priority over Custom ones for static indexing.
pub struct GlobalDictManager {
    // Map: Language Code (e.g., "en", "pl") -> Loader
    loaders: RwLock<HashMap<String, DictionaryLoader>>,
    // List of active custom dictionary IDs/names (for UI display and management)
    active_custom_ids: RwLock<Vec<String>>,
    // Configuration
    config: DictConfig,
}

impl GlobalDictManager {
    pub fn new(config: DictConfig) -> Self {
        GlobalDictManager {
            loaders: RwLock::new(HashMap::new()),
            active_custom_ids: RwLock::new(Vec::new()),
            config,
        }
    }

    /// Registers a new dictionary.
    /// - `is_official`: If true, overwrites existing loader for this language.
    /// - `is_official`: If false, merges entries into existing loader or creates new if missing.
    pub async fn register_dict(
        &self, 
        lang_code: &str, 
        json_content: &str, 
        is_official: bool
    ) -> Result<(), &'static str> {
        let mut loader = DictionaryLoader::load_from_json(json_content)
            .map_err(|_| "ERR_DICT_PARSE_FAILED")?;
        
        let mut loaders = self.loaders.write().await;
        
        if is_official {
            // Official: Overwrite completely (higher priority)
            loaders.insert(lang_code.to_string(), loader);
            info!("MSG_DICT_OFFICIAL_LOADED: {}", lang_code);
        } else {
            // Custom: Merge or Append
            if let Some(existing_loader) = loaders.get_mut(lang_code) {
                // Merge custom entries into the existing loader (usually into a separate 'custom' map inside Loader)
                existing_loader.merge_custom(&mut loader);
                info!("MSG_DICT_CUSTOM_MERGED: {}", lang_code);
            } else {
                // If no official dict exists yet, just insert the custom one
                loaders.insert(lang_code.to_string(), loader);
                info!("MSG_DICT_CUSTOM_LOADED_NO_OFFICIAL: {}", lang_code);
            }
            
            // Track active custom dict
            let mut ids = self.active_custom_ids.write().await;
            if ids.len() >= self.config.max_custom_dicts {
                warn!("ERR_DICT_MAX_CUSTOM_REACHED");
                // Policy: Reject new or replace oldest? Here we reject.
                return Err("ERR_DICT_MAX_CUSTOM_REACHED");
            }
            ids.push(format!("custom_{}", lang_code)); 
        }

        Ok(())
    }

    /// Retrieves a static index for a word if it exists in the official dictionary.
    /// Returns None if word is not in static dict or only in custom dict.
    pub async fn get_static_index(&self, lang_code: &str, word: &str) -> Option<u8> {
        let loaders = self.loaders.read().await;
        loaders.get(lang_code).and_then(|l| l.get_static_index(word))
    }

    /// Retrieves an entry for a given word (metadata, frequency, etc.).
    pub async fn get_entry(&self, lang_code: &str, word: &str) -> Option<DictEntry> {
        let loaders = self.loaders.read().await;
        // Try requested language
        if let Some(entry) = loaders.get(lang_code).and_then(|l| l.get_entry(word)) {
            return Some(entry);
        }
        
        // Fallback to default language if configured
        if lang_code != self.config.fallback_language {
            if let Some(entry) = loaders.get(&self.config.fallback_language).and_then(|l| l.get_entry(word)) {
                return Some(entry);
            }
        }
        
        None
    }

    /// Creates a new SessionDictionary for a P2P connection.
    /// Pre-loads it with high-frequency words from the global loader to speed up compression.
    pub async fn create_session_dict(&self, lang_code: &str) -> SessionDictionary {
        let mut session_dict = SessionDictionary::new();
        
        // Pre-populate with top N frequent words from the global loader
        if let Some(loader) = self.loaders.read().await.get(lang_code) {
            let top_words = loader.get_top_frequent(20); // Get top 20 words
            for (idx, word) in top_words.into_iter().enumerate() {
                // Offset by 10 to leave room for potential protocol reserved indices (0-9)
                let dynamic_idx = (idx + 10) as u8; 
                session_dict.word_to_index.insert(word.clone(), dynamic_idx);
                session_dict.index_to_word.insert(dynamic_idx, word);
            }
            info!("MSG_DICT_SESSION_PRELOADED: {} words for {}", top_words.len(), lang_code);
        } else {
            warn!("MSG_DICT_LANG_NOT_FOUND_FOR_SESSION: {}", lang_code);
        }
        
        session_dict
    }

    /// Unloads a custom dictionary by ID.
    pub async fn unload_custom_dict(&self, custom_id: &str) -> Result<(), &'static str> {
        // Logic to remove specific custom entries would go here
        // For now, simplified:
        let mut ids = self.active_custom_ids.write().await;
        if let Some(pos) = ids.iter().position(|x| x == custom_id) {
            ids.remove(pos);
            info!("MSG_DICT_CUSTOM_UNLOADED: {}", custom_id);
            Ok(())
        } else {
            Err("ERR_DICT_CUSTOM_NOT_FOUND")
        }
    }
}

impl Default for GlobalDictManager {
    fn default() -> Self {
        Self::new(DictConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_merge() {
        let manager = GlobalDictManager::default();
        
        // Register Official
        let official_json = r#"{"meta": {"lang": "en"}, "static_dict": {"kino": 1}}"#;
        assert!(manager.register_dict("en", official_json, true).await.is_ok());
        
        // Register Custom (should merge)
        let custom_json = r#"{"meta": {"lang": "en"}, "static_dict": {"gry": 2}}"#;
        assert!(manager.register_dict("en", custom_json, false).await.is_ok());
        
        // Check if both exist (logic depends on Loader implementation of merge)
        // This assumes Loader::merge_custom works correctly
    }
}
