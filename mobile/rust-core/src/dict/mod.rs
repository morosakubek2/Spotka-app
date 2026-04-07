// mobile/rust-core/src/dict/mod.rs
// Dictionary Module: Loading, Parsing (CTS), and Adaptive Compression.
// Architecture: Multi-language support, Official vs Custom priority, Session-based compression.
// Year: 2026 | Rust Edition: 2024

pub mod cts_parser;
pub mod loader;
pub mod compressor;

// Re-export key types and functions for easier access in other modules
pub use cts_parser::{parse_cts, serialize_cts, CtsTag, TagStatus, CtsError};
pub use loader::{DictionaryLoader, DictEntry, DictSource};
pub use compressor::{SessionDictionary, CompressedTag, estimate_savings};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{info, warn};

/// Global configuration constants for the Dictionary system.
pub mod consts {
    /// Default threshold for promoting a word to a dynamic index in a session.
    pub const DEFAULT_PROMOTION_THRESHOLD: u32 = 3;
    
    /// Maximum number of custom dictionaries a user can load simultaneously.
    pub const MAX_CUSTOM_DICTS: usize = 5;
}

/// Manages the global state of dictionaries (Official + Custom).
/// Thread-safe wrapper around multiple DictionaryLoaders.
pub struct GlobalDictManager {
    // Map: Language Code (e.g., "en", "pl") -> Loader
    loaders: RwLock<HashMap<String, DictionaryLoader>>,
    // Active custom dictionary paths (for persistence)
    active_custom_paths: RwLock<Vec<String>>,
}

impl GlobalDictManager {
    pub fn new() -> Self {
        GlobalDictManager {
            loaders: RwLock::new(HashMap::new()),
            active_custom_paths: RwLock::new(Vec::new()),
        }
    }

    /// Registers a new dictionary (Official or Custom).
    /// Official dictionaries overwrite existing ones for that language.
    /// Custom dictionaries are merged or appended based on policy.
    pub async fn register_dict(&self, lang_code: &str, json_content: &str, is_custom: bool) -> Result<(), &'static str> {
        let loader = DictionaryLoader::load_from_json(json_content)?;
        
        let mut loaders = self.loaders.write().await;
        
        if is_custom {
            // Policy: Custom dicts do not overwrite official ones. 
            // They might be stored in a separate list or merged carefully.
            // For simplicity here, we store them under a modified key or merge entries.
            // Let's assume we merge entries into the existing loader if present, or create new.
            if let Some(existing) = loaders.get_mut(lang_code) {
                existing.merge_custom(&loader); // Hypothetical method in Loader
                info!("MSG_DICT_CUSTOM_MERGED: {}", lang_code);
            } else {
                loaders.insert(lang_code.to_string(), loader);
                info!("MSG_DICT_CUSTOM_LOADED: {}", lang_code);
            }
            
            let mut paths = self.active_custom_paths.write().await;
            paths.push(format!("custom_{}", lang_code)); // Placeholder for real path
        } else {
            // Official: Overwrite
            loaders.insert(lang_code.to_string(), loader);
            info!("MSG_DICT_OFFICIAL_LOADED: {}", lang_code);
        }

        Ok(())
    }

    /// Retrieves an entry for a given word in the specified language.
    /// Handles fallback logic (Custom -> Official -> Default).
    pub async fn get_entry(&self, lang_code: &str, word: &str) -> Option<DictEntry> {
        let loaders = self.loaders.read().await;
        loaders.get(lang_code).and_then(|l| l.get_entry(word))
    }

    /// Creates a new SessionDictionary for a P2P connection, pre-loaded with frequent words.
    pub async fn create_session_dict(&self, lang_code: &str) -> SessionDictionary {
        let mut session_dict = SessionDictionary::new();
        
        // Pre-populate with top N frequent words from the global loader for this language
        if let Some(loader) = self.loaders.read().await.get(lang_code) {
            let top_words = loader.get_top_frequent(20); // Get top 20 words
            for (idx, word) in top_words.into_iter().enumerate() {
                session_dict.word_to_index.insert(word.clone(), (idx + 10) as u8); // Offset for dynamic
                session_dict.index_to_word.insert((idx + 10) as u8, word);
            }
            info!("MSG_DICT_SESSION_PRELOADED: {} words", top_words.len());
        }
        
        session_dict
    }
}

// Default implementation for convenience
impl Default for GlobalDictManager {
    fn default() -> Self {
        Self::new()
    }
}
