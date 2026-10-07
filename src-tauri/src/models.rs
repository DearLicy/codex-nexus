use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A provider account references a secret by name.  Secret material is kept in
/// the platform keychain or another secret store and is never serialized here.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderAccount {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub endpoint: String,
    #[serde(default)]
    pub credential_ref: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Lower values are preferred by the `Priority` strategy.
    #[serde(default)]
    pub priority: u32,
    /// Weight used by the deterministic weighted strategy.
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default)]
    pub failure_count: u32,
    #[serde(default)]
    pub cooldown_until_ms: Option<u64>,
    #[serde(default)]
    pub last_used_at_ms: Option<u64>,
    #[serde(default)]
    pub quota_remaining: Option<u64>,
    #[serde(default)]
    pub plan: Option<String>,
    #[serde(default)]
    pub image_enabled: bool,
    #[serde(default = "default_concurrency")]
    pub max_concurrent: u16,
    #[serde(default)]
    pub in_flight: u16,
}

impl ProviderAccount {
    pub fn new(
        id: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
        endpoint: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider: provider.into(),
            model: model.into(),
            endpoint: endpoint.into(),
            credential_ref: None,
            enabled: true,
            priority: 0,
            weight: 1,
            failure_count: 0,
            cooldown_until_ms: None,
            last_used_at_ms: None,
            quota_remaining: None,
            plan: None,
            image_enabled: false,
            max_concurrent: default_concurrency(),
            in_flight: 0,
        }
    }

    pub fn is_available(&self, now_ms: u64) -> bool {
        self.enabled
            && self
                .cooldown_until_ms
                .map(|until| until <= now_ms)
                .unwrap_or(true)
    }
}

/// Runtime pool state.  It is serializable so the configured account list can
/// be persisted, but a caller should omit `credential_ref` when exporting it.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountPool {
    accounts: BTreeMap<String, ProviderAccount>,
}

impl AccountPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, account: ProviderAccount) -> Result<(), String> {
        if account.id.trim().is_empty() {
            return Err("account id cannot be empty".into());
        }
        if account.provider.trim().is_empty() {
            return Err("provider cannot be empty".into());
        }
        if account.endpoint.trim().is_empty() {
            return Err("endpoint cannot be empty".into());
        }
        if self.accounts.contains_key(&account.id) {
            return Err(format!("account already exists: {}", account.id));
        }
        self.accounts.insert(account.id.clone(), account);
        Ok(())
    }

    pub fn upsert(&mut self, account: ProviderAccount) -> Result<(), String> {
        if account.id.trim().is_empty() {
            return Err("account id cannot be empty".into());
        }
        if account.provider.trim().is_empty() {
            return Err("provider cannot be empty".into());
        }
        if account.endpoint.trim().is_empty() {
            return Err("endpoint cannot be empty".into());
        }
        self.accounts.insert(account.id.clone(), account);
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> Option<ProviderAccount> {
        self.accounts.remove(id)
    }

    pub fn get(&self, id: &str) -> Option<&ProviderAccount> {
        self.accounts.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ProviderAccount> {
        self.accounts.get_mut(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ProviderAccount> {
        self.accounts.values()
    }

    pub fn len(&self) -> usize {
        self.accounts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SelectionStrategy {
    Single,
    RoundRobin,
    Weighted,
    Priority,
    Random,
    QuotaAware,
    Fallback,
}

impl Default for SelectionStrategy {
    fn default() -> Self {
        Self::RoundRobin
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutePolicy {
    #[serde(default)]
    pub strategy: SelectionStrategy,
    #[serde(default = "default_true")]
    pub session_sticky: bool,
    /// The total number of provider attempts, including the initial attempt.
    #[serde(default = "default_attempts")]
    pub max_attempts: u32,
    /// A failed account is unavailable for this many milliseconds.
    #[serde(default = "default_cooldown")]
    pub cooldown_ms: u64,
    /// Exposed to a caller that wants to sleep between attempts.  The router
    /// itself never blocks the UI thread.
    #[serde(default)]
    pub retry_backoff_ms: u64,
    #[serde(default)]
    pub allowed_providers: Vec<String>,
    #[serde(default)]
    pub preferred_model: Option<String>,
}

impl Default for RoutePolicy {
    fn default() -> Self {
        Self {
            strategy: SelectionStrategy::QuotaAware,
            session_sticky: true,
            max_attempts: 3,
            cooldown_ms: 30_000,
            retry_backoff_ms: 250,
            allowed_providers: Vec::new(),
            preferred_model: None,
        }
    }
}

impl RoutePolicy {
    pub fn normalized(&self) -> Self {
        let mut value = self.clone();
        value.max_attempts = value.max_attempts.max(1);
        value
    }

    pub fn backoff_for_attempt(&self, attempt: u32) -> u64 {
        // Cap exponential backoff to one minute and avoid overflow.
        let shift = attempt.saturating_sub(1).min(8);
        self.retry_backoff_ms
            .saturating_mul(1_u64 << shift)
            .min(60_000)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl Default for JobStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageJob {
    pub id: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    pub prompt: String,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub input_artifacts: Vec<String>,
    #[serde(default = "default_output_format")]
    pub output_format: String,
    #[serde(default)]
    pub status: JobStatus,
    #[serde(default)]
    pub attempts: u32,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

impl ImageJob {
    pub fn new(id: impl Into<String>, prompt: impl Into<String>, now_ms: u64) -> Self {
        Self {
            id: id.into(),
            model: None,
            provider: None,
            session_id: None,
            prompt: prompt.into(),
            negative_prompt: None,
            width: None,
            height: None,
            input_artifacts: Vec::new(),
            output_format: default_output_format(),
            status: JobStatus::Pending,
            attempts: 0,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArtifactStatus {
    Accepted,
    /// Found during indexing but requires an explicit user decision before
    /// moving it to the recoverable quarantine.
    Review,
    Quarantined,
    Restored,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactRecord {
    pub id: String,
    pub job_id: String,
    /// Relative to the scanner root.  Keeping this relative prevents an
    /// imported record from silently referring to an arbitrary absolute path.
    pub source_path: String,
    #[serde(default)]
    pub quarantined_path: Option<String>,
    pub status: ArtifactStatus,
    #[serde(default)]
    pub mime_type: Option<String>,
    pub size_bytes: u64,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    pub created_at_ms: u64,
}

fn default_true() -> bool {
    true
}

fn default_weight() -> u32 {
    1
}

fn default_attempts() -> u32 {
    3
}

fn default_cooldown() -> u64 {
    30_000
}

fn default_output_format() -> String {
    "png".into()
}

fn default_concurrency() -> u16 {
    1
}
