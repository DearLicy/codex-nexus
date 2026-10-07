use crate::models::ProviderAccount;
use rusqlite::{params, Connection};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Local metadata store. Secret material is never placed in this database;
/// `credential_ref` points to the platform secret backend.
pub struct WorkspaceStore {
    connection: Connection,
}

impl WorkspaceStore {
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, StoreError> {
        let connection = Connection::open(path)?;
        let store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self, StoreError> {
        let connection = Connection::open_in_memory()?;
        let store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), StoreError> {
        self.connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS providers (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, protocol TEXT NOT NULL,
                base_url TEXT NOT NULL, capabilities_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY, provider_id TEXT NOT NULL, model TEXT NOT NULL,
                endpoint TEXT NOT NULL, credential_ref TEXT, enabled INTEGER NOT NULL,
                priority INTEGER NOT NULL, weight INTEGER NOT NULL,
                cooldown_until INTEGER, last_used_at INTEGER, quota_remaining INTEGER,
                plan TEXT, image_enabled INTEGER NOT NULL DEFAULT 0,
                max_concurrent INTEGER NOT NULL DEFAULT 1, in_flight INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY(provider_id) REFERENCES providers(id)
             );
             CREATE TABLE IF NOT EXISTS artifacts (
                id TEXT PRIMARY KEY, job_id TEXT NOT NULL, source_path TEXT NOT NULL,
                size_bytes INTEGER NOT NULL, status TEXT NOT NULL, sha256 TEXT,
                created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS cleanup_log (
                id TEXT PRIMARY KEY, action TEXT NOT NULL, artifact_ids TEXT NOT NULL,
                created_at INTEGER NOT NULL
             );",
        )?;
        Ok(())
    }

    pub fn upsert_account(&self, account: &ProviderAccount) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT INTO accounts (id, provider_id, model, endpoint, credential_ref, enabled, priority, weight, cooldown_until, last_used_at, quota_remaining, plan, image_enabled, max_concurrent, in_flight)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(id) DO UPDATE SET provider_id=excluded.provider_id, model=excluded.model,
               endpoint=excluded.endpoint, credential_ref=excluded.credential_ref, enabled=excluded.enabled,
               priority=excluded.priority, weight=excluded.weight, cooldown_until=excluded.cooldown_until,
               last_used_at=excluded.last_used_at, quota_remaining=excluded.quota_remaining,
               plan=excluded.plan, image_enabled=excluded.image_enabled,
               max_concurrent=excluded.max_concurrent, in_flight=excluded.in_flight",
            params![
                account.id, account.provider, account.model, account.endpoint,
                account.credential_ref, account.enabled, account.priority, account.weight,
                account.cooldown_until_ms, account.last_used_at_ms, account.quota_remaining,
                account.plan, account.image_enabled, account.max_concurrent, account.in_flight
            ],
        )?;
        Ok(())
    }

    pub fn list_accounts(&self) -> Result<Vec<ProviderAccount>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, provider_id, model, endpoint, credential_ref, enabled, priority, weight, cooldown_until, last_used_at, quota_remaining, plan, image_enabled, max_concurrent, in_flight FROM accounts ORDER BY id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ProviderAccount {
                id: row.get(0)?,
                provider: row.get(1)?,
                model: row.get(2)?,
                endpoint: row.get(3)?,
                credential_ref: row.get(4)?,
                enabled: row.get(5)?,
                priority: row.get(6)?,
                weight: row.get(7)?,
                failure_count: 0,
                cooldown_until_ms: row.get(8)?,
                last_used_at_ms: row.get(9)?,
                quota_remaining: row.get(10)?,
                plan: row.get(11)?,
                image_enabled: row.get(12)?,
                max_concurrent: row.get(13)?,
                in_flight: row.get(14)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}
