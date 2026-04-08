// mobile/rust-core/src/db/manager.rs
// Database Manager: Encrypted Storage (SQLCipher) + ORM (Drift).
// Features: Argon2 Key Derivation, Secure Memory Wiping, Migration Handling.
// Year: 2026 | Rust Edition: 2024

use crate::db::schema::{AppDatabase, User, Meeting, ChainBlock, LocalConfig, DictionaryCache, PushToken};
use argon2::{Argon2, password_hash::SaltString, PasswordHasher};
use drift::prelude::*;
use log::{info, error, warn};
use rand::rngs::OsRng;
use zeroize::{Zeroize, Zeroizing};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Custom Error Type for Database Operations.
/// Returns keys for translation, never hardcoded messages.
#[derive(Debug)]
pub enum DbError {
    InitFailed,
    KeyDerivationFailed,
    MigrationFailed,
    QueryFailed,
    EncryptionFailed,
    PathInvalid,
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DbError::InitFailed => write!(f, "ERR_DB_INIT_FAILED"),
            DbError::KeyDerivationFailed => write!(f, "ERR_DB_KEY_DERIVATION_FAILED"),
            DbError::MigrationFailed => write!(f, "ERR_DB_MIGRATION_FAILED"),
            DbError::QueryFailed => write!(f, "ERR_DB_QUERY_FAILED"),
            DbError::EncryptionFailed => write!(f, "ERR_DB_ENCRYPTION_FAILED"),
            DbError::PathInvalid => write!(f, "ERR_DB_PATH_INVALID"),
        }
    }
}

/// Main Database Manager structure.
/// Holds the connection pool and provides high-level access methods.
pub struct DbManager {
    conn: Arc<RwLock<SqliteConnection>>,
}

impl DbManager {
    /// Initializes the database connection with SQLCipher encryption.
    /// 
    /// # Arguments
    /// * `db_path` - Path to the SQLite file.
    /// * `auth_secret` - Secret derived from biometrics/PIN (used to derive encryption key).
    pub async fn new(db_path: &str, auth_secret: &str) -> Result<Self, DbError> {
        // 1. Validate Path
        if !Path::new(db_path).parent().map_or(false, |p| p.exists()) {
            // In a real app, we might create directories, but here we fail safely
            return Err(DbError::PathInvalid);
        }

        info!("MSG_DB_INIT_START: {}", db_path);

        // 2. Secure Key Derivation (Argon2id)
        // Use a fixed salt for the device-specific key derivation (stored in OS KeyStore ideally)
        // For this example, we generate a random salt per session (NOT recommended for production without persistent salt)
        // Production: Fetch persistent salt from Android Keystore / iOS Secure Enclave
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        
        // Zeroizing ensures the key is wiped from memory when dropped
        let mut hashed_key = Zeroizing::new(
            argon2.hash_password(auth_secret.as_bytes(), &salt)
                .map_err(|_| DbError::KeyDerivationFailed)?
                .to_string()
        );

        // Derive a 32-byte hex key for SQLCipher from the Argon2 hash
        // Simplified: taking first 32 chars of hex representation (in prod, use HKDF)
        let cipher_key = format!("{:x}", blake3::hash(hashed_key.as_bytes()).as_bytes()[..16].iter().map(|b| format!("{:02x}", b)).collect::<String>());
        
        // Wipe sensitive data immediately after use
        hashed_key.zeroize(); 

        // 3. Initialize Connection
        let mut conn_builder = SqliteConnection::new(db_path);

        // 4. Configure SQLCipher (Security Hardening)
        // Set key before opening
        conn_builder.execute("PRAGMA key = '{}'", &[&cipher_key])
            .map_err(|_| DbError::EncryptionFailed)?;
        
        // Harden settings
        conn_builder.execute("PRAGMA kdf_iter = 256000", &[]).ok(); // High iterations
        conn_builder.execute("PRAGMA cipher_page_size = 4096", &[]).ok();
        conn_builder.execute("PRAGMA journal_mode = WAL", &[]).ok(); // Better performance

        let conn = Arc::new(RwLock::new(conn_builder));

        // 5. Run Migrations (Create Tables if not exist)
        Self::run_migrations(&conn).await?;

        info!("MSG_DB_INIT_SUCCESS");
        Ok(DbManager { conn })
    }

    /// Runs database migrations to ensure schema is up-to-date.
    /// Handles creation of all tables defined in schema.rs including new ones.
    async fn run_migrations(conn: &Arc<RwLock<SqliteConnection>>) -> Result<(), DbError> {
        let mut db_conn = conn.write().await;
        
        // Note: In Drift, migrations are often auto-generated or handled by macros.
        // Here we simulate manual execution for clarity or use Drift's migrate function if available.
        // Since we added DictionaryCache and PushTokens, we ensure they exist.
        
        // Pseudo-code for Drift migration execution:
        // drift::migrate::run(&mut *db_conn, MIGRATIONS).await.map_err(|_| DbError::MigrationFailed)?;
        
        // Manual fallback simulation (Drift usually handles this via #[drift::migration])
        // Creating tables explicitly if Drift macro isn't used for auto-migration in this snippet
        let queries = vec![
            // Users Table
            "CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY, 
                display_name TEXT, 
                reputation_score INTEGER, 
                trust_level INTEGER, 
                last_seen INTEGER, 
                public_key_blob BLOB, 
                verifier_count INTEGER,
                is_ghost INTEGER DEFAULT 0
            )",
            // Meetings Table
            "CREATE TABLE IF NOT EXISTS meetings (
                id TEXT PRIMARY KEY, 
                organizer_id TEXT, 
                location_lat REAL, 
                location_lon REAL, 
                start_time INTEGER, 
                min_duration_mins INTEGER, 
                tags_cts TEXT, 
                status INTEGER, 
                guest_count INTEGER, 
                invited_users_count INTEGER, 
                created_at INTEGER
            )",
            // Participants Table
            "CREATE TABLE IF NOT EXISTS meeting_participants (
                meeting_id TEXT, 
                user_id TEXT, 
                status INTEGER, 
                verification_signature BLOB, 
                PRIMARY KEY (meeting_id, user_id)
            )",
            // Chain Blocks Table
            "CREATE TABLE IF NOT EXISTS chain_blocks (
                height INTEGER PRIMARY KEY, 
                prev_hash TEXT, 
                merkle_root TEXT, 
                timestamp INTEGER, 
                validator_id TEXT, 
                signature BLOB
            )",
            // Local Config Table
            "CREATE TABLE IF NOT EXISTS local_config (
                key TEXT PRIMARY KEY, 
                value_blob BLOB
            )",
            // NEW: Dictionary Cache Table
            "CREATE TABLE IF NOT EXISTS dictionary_cache (
                word TEXT PRIMARY KEY, 
                category TEXT, 
                freq INTEGER, 
                lang_code TEXT
            )",
            // NEW: Push Tokens Table
            "CREATE TABLE IF NOT EXISTS push_tokens (
                provider TEXT PRIMARY KEY, 
                token_blob BLOB, 
                updated_at INTEGER
            )"
        ];

        for query in queries {
            db_conn.execute(query, &[]).map_err(|_| DbError::MigrationFailed)?;
        }

        info!("MSG_DB_MIGRATIONS_COMPLETE");
        Ok(())
    }

    /// Returns an instance of the typed Database API generated by Drift.
    pub fn database(&self) -> AppDatabase {
        AppDatabase {
            users: UsersTable::new(),
            meetings: MeetingsTable::new(),
            participants: MeetingParticipantsTable::new(),
            blocks: ChainBlocksTable::new(),
            config: LocalConfigsTable::new(),
            dict_cache: DictionaryCachesTable::new(),
            push_tokens: PushTokensTable::new(),
        }
    }

    /// Helper: Save a Push Token securely.
    pub async fn save_push_token(&self, provider: &str, token: &[u8]) -> Result<(), DbError> {
        let db = self.database();
        let conn = self.conn.read().await;
        
        // Using Drift syntax (pseudo-code adapted for clarity)
        // db.push_tokens.insert(PushToken {
        //     provider: provider.to_string(),
        //     token_blob: token.to_vec(),
        //     updated_at: chrono::Utc::now().timestamp(),
        // }).execute(&*conn).await.map_err(|_| DbError::QueryFailed)?;
        
        // Raw SQL fallback for demonstration
        let query = "INSERT OR REPLACE INTO push_tokens (provider, token_blob, updated_at) VALUES (?, ?, ?)";
        conn.execute(query, &[provider, token, &chrono::Utc::now().timestamp().to_string()])
            .map_err(|_| DbError::QueryFailed)?;
            
        Ok(())
    }

    /// Helper: Save/Update Dictionary Entry.
    pub async fn save_dict_entry(&self, word: &str, category: &str, freq: u32, lang: &str) -> Result<(), DbError> {
        let conn = self.conn.read().await;
        let query = "INSERT OR REPLACE INTO dictionary_cache (word, category, freq, lang_code) VALUES (?, ?, ?, ?)";
        conn.execute(query, &[word, category, &freq.to_string(), lang])
            .map_err(|_| DbError::QueryFailed)?;
        Ok(())
    }
    
    /// Helper: Execute a transaction.
    pub async fn with_transaction<F, T>(&self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&SqliteConnection) -> Result<T, DbError>,
    {
        let conn = self.conn.write().await;
        // Start transaction
        conn.execute("BEGIN IMMEDIATE", &[]).map_err(|_| DbError::QueryFailed)?;
        
        match f(&conn) {
            Ok(res) => {
                conn.execute("COMMIT", &[]).map_err(|_| DbError::QueryFailed)?;
                Ok(res)
            },
            Err(e) => {
                conn.execute("ROLLBACK", &[]).ok(); // Ignore rollback error
                Err(e)
            }
        }
    }
}
