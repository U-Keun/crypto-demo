use chrono::Utc;
use hmac::{Hmac, Mac};
use rusqlite::{params, Connection, Result as SqliteResult};
use sha2::Sha256;
use std::sync::{Arc, Mutex};
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

/// Database connection wrapper
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    hmac_key: [u8; 32],
}

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("User not found")]
    NotFound,
}

/// User data stored in database
#[derive(Debug)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub phone_enc: Vec<u8>,
    pub phone_nonce: Vec<u8>,
    pub phone_tag: Vec<u8>,
    pub phone_keyver: i32,
    pub phone_token: Vec<u8>,
    pub created_at: String,
}

/// User data for insertion
pub struct NewUser {
    pub name: String,
    pub phone_enc: Vec<u8>,
    pub phone_nonce: Vec<u8>,
    pub phone_tag: Vec<u8>,
    pub phone_keyver: i32,
    pub phone_plaintext: Vec<u8>, // Used only for token generation
}

impl Database {
    /// Create a new database connection and initialize schema
    pub fn new(db_path: &str, hmac_key: &[u8; 32]) -> Result<Self, DbError> {
        let conn = Connection::open(db_path)?;

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", [])?;

        // Create tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                phone_enc BLOB NOT NULL,
                phone_nonce BLOB NOT NULL,
                phone_tag BLOB NOT NULL,
                phone_keyver INTEGER NOT NULL,
                phone_token BLOB NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        // Create index on token for search
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_users_phone_token ON users(phone_token)",
            [],
        )?;

        Ok(Database {
            conn: Arc::new(Mutex::new(conn)),
            hmac_key: *hmac_key,
        })
    }

    /// Generate HMAC token for search (exact match)
    fn generate_token(&self, plaintext: &[u8]) -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(&self.hmac_key)
            .expect("HMAC can take key of any size");
        mac.update(plaintext);
        mac.finalize().into_bytes().to_vec()
    }

    /// Insert a new user
    pub fn insert_user(&self, user: NewUser) -> Result<i64, DbError> {
        let token = self.generate_token(&user.phone_plaintext);
        let created_at = Utc::now().to_rfc3339();

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO users (name, phone_enc, phone_nonce, phone_tag, phone_keyver, phone_token, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                user.name,
                user.phone_enc,
                user.phone_nonce,
                user.phone_tag,
                user.phone_keyver,
                token,
                created_at,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get user by ID
    pub fn get_user(&self, id: i64) -> Result<User, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, phone_enc, phone_nonce, phone_tag, phone_keyver, phone_token, created_at
             FROM users WHERE id = ?1",
        )?;

        let user = stmt
            .query_row(params![id], |row| {
                Ok(User {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    phone_enc: row.get(2)?,
                    phone_nonce: row.get(3)?,
                    phone_tag: row.get(4)?,
                    phone_keyver: row.get(5)?,
                    phone_token: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound,
                _ => DbError::Sqlite(e),
            })?;

        Ok(user)
    }

    /// Search user by phone (exact match using token)
    pub fn find_user_by_phone(&self, phone_plaintext: &[u8]) -> Result<Option<User>, DbError> {
        let token = self.generate_token(phone_plaintext);
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, name, phone_enc, phone_nonce, phone_tag, phone_keyver, phone_token, created_at
             FROM users WHERE phone_token = ?1 LIMIT 1",
        )?;

        let mut rows = stmt.query(params![token])?;

        if let Some(row) = rows.next()? {
            Ok(Some(User {
                id: row.get(0)?,
                name: row.get(1)?,
                phone_enc: row.get(2)?,
                phone_nonce: row.get(3)?,
                phone_tag: row.get(4)?,
                phone_keyver: row.get(5)?,
                phone_token: row.get(6)?,
                created_at: row.get(7)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// List all users (for testing/debugging)
    pub fn list_users(&self) -> Result<Vec<User>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, phone_enc, phone_nonce, phone_tag, phone_keyver, phone_token, created_at
             FROM users ORDER BY id",
        )?;

        let users = stmt
            .query_map([], |row| {
                Ok(User {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    phone_enc: row.get(2)?,
                    phone_nonce: row.get(3)?,
                    phone_tag: row.get(4)?,
                    phone_keyver: row.get(5)?,
                    phone_token: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(users)
    }
}
