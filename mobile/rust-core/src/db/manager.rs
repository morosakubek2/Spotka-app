// mobile/rust-core/src/db/manager.rs
// Database Manager: Encrypted Storage (SQLCipher) + ORM (Drift).
// Features: Argon2 Key Derivation, Secure Memory Wiping, Migration Handling, Free/Pro Logic.
// Year: 2026 | Rust Edition: 2024

use crate::db::schema::{AppDatabase, LocalConfig};
use argon2::{Argon2, password_hash::SaltString, PasswordHasher};
use drift::prelude::*;
use log::{info, error, warn};
use rand::rngs::OsRng;
use zeroize::{Zeroize, Zeroizing};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use hex; // Do konwersji binarnych na hex

/// Custom Error Type for Database Operations.
#[derive(Debug)]
pub enum DbError {
    InitFailed,
    KeyDerivationFailed,
    MigrationFailed,
    QueryFailed,
    EncryptionFailed,
    PathInvalid,
    SaltMissing, // Nowy błąd dla braku stałej soli
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
            DbError::SaltMissing => write!(f, "ERR_DB_SALT_MISSING"),
        }
    }
}

/// Main Database Manager structure.
pub struct DbManager {
    conn: Arc<RwLock<SqliteConnection>>,
}

impl DbManager {
    /// Initializes the database connection with SQLCipher encryption.
    /// 
    /// # Arguments
    /// * `db_path` - Path to the SQLite file.
    /// * `auth_secret` - Secret derived from biometrics/PIN.
    /// * `device_salt` - Persistent salt stored in OS Keystore (CRITICAL for decryption consistency).
    pub async fn new(db_path: &str, auth_secret: &str, device_salt: Option<&[u8]>) -> Result<Self, DbError> {
        if !Path::new(db_path).parent().map_or(false, |p| p.exists()) {
            return Err(DbError::PathInvalid);
        }

        info!("MSG_DB_INIT_START: {}", db_path);

        // 1. Secure Key Derivation (Argon2id)
        // Używamy stałej soli z urządzenia (device_salt), aby klucz był odtwarzalny.
        let salt_str = if let Some(salt) = device_salt {
            SaltString::from_b64(&hex::encode(salt))
                .map_err(|_| DbError::KeyDerivationFailed)?
        } else {
            // W ostateczności generujemy nową (tylko przy pierwszej instalacji, potem musi być zapisana w Keystore)
            // W produkcji: Jeśli brak soli w Keystore, wygeneruj i ZAPISZ w Keystore przed użyciem.
            SaltString::generate(&mut OsRng)
        };

        let argon2 = Argon2::default();
        
        let mut hashed_key = Zeroizing::new(
            argon2.hash_password(auth_secret.as_bytes(), &salt_str)
                .map_err(|_| DbError::KeyDerivationFailed)?
                .to_string()
        );

        // Derive 32-byte key for SQLCipher using BLAKE3
        let hash_output = blake3::hash(hashed_key.as_bytes());
        let cipher_key = hex::encode(hash_output.as_bytes()[..32].iter());
        
        hashed_key.zeroize(); 

        let mut conn_builder = SqliteConnection::new(db_path);

        // Configure SQLCipher
        conn_builder.execute("PRAGMA key = '{}'", &[&cipher_key])
            .map_err(|_| DbError::EncryptionFailed)?;
        
        conn_builder.execute("PRAGMA kdf_iter = 256000", &[]).ok();
        conn_builder.execute("PRAGMA cipher_page_size = 4096", &[]).ok();
        conn_builder.execute("PRAGMA journal_mode = WAL", &[]).ok();

        let conn = Arc::new(RwLock::new(conn_builder));

        // Run Migrations
        Self::run_migrations(&conn).await?;

        info!("MSG_DB_INIT_SUCCESS");
        Ok(DbManager { conn })
    }

    /// Runs database migrations.
    async fn run_migrations(conn: &Arc<RwLock<SqliteConnection>>) -> Result<(), DbError> {
        let mut db_conn = conn.write().await;
        
        // Pełne definicje tabel zgodne ze schema.rs
        let queries = vec![
            // Users Table
            "CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY, 
                display_name TEXT, 
                reputation_score INTEGER, 
                trust_level INTEGER, 
                public_key_blob BLOB, 
                verifier_count INTEGER,
                is_ghost INTEGER DEFAULT 0,
                last_seen INTEGER,
                reputation_index INTEGER
            )",
            "CREATE INDEX IF NOT EXISTS idx_users_rep ON users(reputation_index)",

            // Meetings Table (Updated for Free/Pro logic & Schema)
            "CREATE TABLE IF NOT EXISTS meetings (
                id TEXT PRIMARY KEY, 
                organizer_phone_hash TEXT, 
                location_lat REAL,
                location_lon REAL,
                location_accuracy_meters REAL,
                start_time INTEGER,
                min_duration_mins INTEGER,
                tags_cts_raw TEXT,
                tags_cts_compressed BLOB,
                status INTEGER, -- u8: 1=Active, 3=Cancelled (Free); 0,2 added in Pro
                guest_count INTEGER,
                invited_users_count INTEGER,
                created_at INTEGER,
                updated_at INTEGER,
                geo_time_index_lat REAL,
                geo_time_index_lon REAL,
                status_index INTEGER
            )",
            "CREATE INDEX IF NOT EXISTS idx_meetings_geo ON meetings(geo_time_index_lat, geo_time_index_lon)",
            "CREATE INDEX IF NOT EXISTS idx_meetings_status ON meetings(status_index)",

            // Meeting Participants Table (Updated with user_status_index)
            "CREATE TABLE IF NOT EXISTS meeting_participants (
                meeting_id TEXT, 
                user_id TEXT, 
                status INTEGER, -- u8: 0=Invited, 1=Interested, 2=Confirmed, 3=Present, 4=NoShow
                verification_signature BLOB,
                user_status_index INTEGER,
                PRIMARY KEY (meeting_id, user_id)
            )",
            "CREATE INDEX IF NOT EXISTS idx_participants_status ON meeting_participants(user_status_index)",

            // Chain Blocks Table
            "CREATE TABLE IF NOT EXISTS chain_blocks (
                height INTEGER PRIMARY KEY, 
                prev_hash TEXT, 
                merkle_root TEXT, 
                timestamp INTEGER, 
                validator_id TEXT, 
                signature BLOB,
                transactions_blob BLOB,
                extended_retention INTEGER DEFAULT 0,
                timestamp_index INTEGER
            )",
            "CREATE INDEX IF NOT EXISTS idx_blocks_time ON chain_blocks(timestamp_index)",

            // Local Config Table
            "CREATE TABLE IF NOT EXISTS local_config (
                key TEXT PRIMARY KEY, 
                value_blob BLOB,
                updated_at INTEGER
            )",

            // Dictionary Cache Table (Full Schema)
            "CREATE TABLE IF NOT EXISTS dictionary_cache (
                word TEXT PRIMARY KEY, 
                category TEXT, 
                frequency_rank INTEGER,
                static_index INTEGER,
                dynamic_index INTEGER,
                language_code TEXT,
                lang_freq_index TEXT
            )",
            "CREATE INDEX IF NOT EXISTS idx_dict_lang ON dictionary_cache(lang_freq_index)",

            // Push Tokens Table (Full Schema)
            "CREATE TABLE IF NOT EXISTS push_tokens (
                provider TEXT PRIMARY KEY, 
                token_blob BLOB, 
                registered_at INTEGER,
                last_used INTEGER,
                is_active INTEGER DEFAULT 1
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
            dictionary: DictionaryCachesTable::new(),
            push_tokens: PushTokensTable::new(),
        }
    }

    /// Helper: Save configuration value as blob.
    pub async fn save_config(&self, key: &str, value: &[u8]) -> Result<(), DbError> {
        let conn = self.conn.read().await;
        let now = chrono::Utc::now().timestamp();
        let query = "INSERT OR REPLACE INTO local_config (key, value_blob, updated_at) VALUES (?, ?, ?)";
        conn.execute(query, &[key, value, &now.to_string()])
            .map_err(|_| DbError::QueryFailed)?;
        Ok(())
    }

    /// Helper: Get configuration value.
    pub async fn get_config(&self, key: &str) -> Result<Option<Vec<u8>>, DbError> {
        let conn = self.conn.read().await;
        // Drift would handle this elegantly, here raw SQL for clarity
        let query = "SELECT value_blob FROM local_config WHERE key = ?";
        // Implementacja pobierania blobu zależy od bindowania w danym wrapperze SQLite
        // Poniżej pseudokod ilustrujący ideę
        let mut stmt = conn.prepare(query).map_err(|_| DbError::QueryFailed)?;
        let mut rows = stmt.query(&[key]).map_err(|_| DbError::QueryFailed)?;
        
        if let Some(row) = rows.next().map_err(|_| DbError::QueryFailed)? {
            // Pobranie blobu z kolumny 0
            let blob: Vec<u8> = row.get(0).map_err(|_| DbError::QueryFailed)?;
            Ok(Some(blob))
        } else {
            Ok(None)
        }
    }

    /// Helper: Prune old meetings (Free Mode Logic).
    /// Removes meetings that are finished (start_time + duration < now) or cancelled.
    pub async fn prune_old_meetings(&self, storage_radius_km: f64, user_lat: f64, user_lon: f64) -> Result<usize, DbError> {
        let conn = self.conn.write().await;
        let now = chrono::Utc::now().timestamp();
        
        // Usuwaj spotkania:
        // 1. Anulowane (status = 3)
        // 2. Zakończone (start_time + min_duration < now) - symulacja Finished dla Free
        // 3. Poza promieniem (opcjonalne, jeśli chcemy czyścić też geograficznie)
        
        let query = "DELETE FROM meetings 
                     WHERE status = 3 
                        OR (start_time + (min_duration_mins * 60)) < ?";
        
        let rows_affected = conn.execute(query, &[&now.to_string()])
            .map_err(|_| DbError::QueryFailed)?;
            
        info!("MSG_DB_PRUNED_MEETINGS: {}", rows_affected);
        Ok(rows_affected)
    }

    /// Helper: Execute a transaction.
    pub async fn with_transaction<F, T>(&self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&SqliteConnection) -> Result<T, DbError>,
    {
        let conn = self.conn.write().await;
        conn.execute("BEGIN IMMEDIATE", &[]).map_err(|_| DbError::QueryFailed)?;
        
        match f(&conn) {
            Ok(res) => {
                conn.execute("COMMIT", &[]).map_err(|_| DbError::QueryFailed)?;
                Ok(res)
            },
            Err(e) => {
                conn.execute("ROLLBACK", &[]).ok();
                Err(e)
            }
        }
    }
}
