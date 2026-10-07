use crate::models::{AccountPool, ProviderAccount, RoutePolicy, SelectionStrategy};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteDecision {
    pub account_id: String,
    pub attempt: u32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouteError {
    #[error("no provider account is available")]
    NoAvailableAccount,
    #[error("account not found: {0}")]
    AccountNotFound(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetryError<E> {
    #[error("no provider account is available after {attempts} attempt(s)")]
    NoAvailableAccount { attempts: u32 },
    #[error("provider attempts exhausted after {attempts} attempt(s)")]
    Exhausted { last_error: E, attempts: u32 },
}

/// Routes jobs while keeping session bindings in memory.  The binding is
/// deliberately runtime state: a persisted account configuration should not
/// persist a user's session-to-provider affinity indefinitely.
#[derive(Clone, Debug, Default)]
pub struct Router {
    pub pool: AccountPool,
    sessions: HashMap<String, String>,
    round_robin_cursor: usize,
}

impl Router {
    pub fn new(pool: AccountPool) -> Self {
        Self {
            pool,
            sessions: HashMap::new(),
            round_robin_cursor: 0,
        }
    }

    pub fn select(
        &mut self,
        session_id: Option<&str>,
        policy: &RoutePolicy,
        now_ms: u64,
    ) -> Result<RouteDecision, RouteError> {
        self.select_excluding(session_id, policy, now_ms, &HashSet::new(), 1)
    }

    fn select_excluding(
        &mut self,
        session_id: Option<&str>,
        policy: &RoutePolicy,
        now_ms: u64,
        excluded: &HashSet<String>,
        attempt: u32,
    ) -> Result<RouteDecision, RouteError> {
        let policy = policy.normalized();

        let existing_binding = session_id.and_then(|session| self.sessions.get(session).cloned());
        let preserve_binding = existing_binding.as_ref().map_or(false, |bound_id| {
            excluded.contains(bound_id)
                || self
                    .pool
                    .get(bound_id)
                    .and_then(|account| account.cooldown_until_ms)
                    .map(|until| until > now_ms)
                    .unwrap_or(false)
        });

        // A sticky account is preferred whenever it is healthy.  If it is in
        // cooldown, a different account is temporarily selected while the
        // binding is kept for the next healthy request.
        if policy.session_sticky {
            if let Some(session) = session_id {
                if let Some(bound_id) = self.sessions.get(session) {
                    if !excluded.contains(bound_id) && self.matches(bound_id, &policy, now_ms) {
                        return Ok(RouteDecision {
                            account_id: bound_id.clone(),
                            attempt,
                        });
                    }
                }
            }
        }

        let mut candidates: Vec<ProviderAccount> = self
            .pool
            .iter()
            .filter(|account| {
                !excluded.contains(&account.id) && self.matches_account(account, &policy, now_ms)
            })
            .cloned()
            .collect();
        if candidates.is_empty() {
            return Err(RouteError::NoAvailableAccount);
        }

        let chosen = match policy.strategy {
            SelectionStrategy::Single
            | SelectionStrategy::Priority
            | SelectionStrategy::Fallback => {
                candidates.sort_by_key(|account| {
                    (
                        account.priority,
                        account.last_used_at_ms.unwrap_or(0),
                        account.id.clone(),
                    )
                });
                candidates.remove(0)
            }
            SelectionStrategy::QuotaAware => {
                candidates.sort_by(|a, b| {
                    b.quota_remaining
                        .unwrap_or(u64::MAX)
                        .cmp(&a.quota_remaining.unwrap_or(u64::MAX))
                        .then_with(|| a.priority.cmp(&b.priority))
                        .then_with(|| a.id.cmp(&b.id))
                });
                candidates.remove(0)
            }
            SelectionStrategy::Weighted => {
                candidates.sort_by(|a, b| a.id.cmp(&b.id));
                let total_weight: usize = candidates.iter().map(|a| a.weight.max(1) as usize).sum();
                let slot = self.round_robin_cursor % total_weight;
                self.round_robin_cursor = self.round_robin_cursor.wrapping_add(1);
                let mut offset = 0;
                candidates
                    .into_iter()
                    .find(|account| {
                        offset += account.weight.max(1) as usize;
                        slot < offset
                    })
                    .expect("weighted candidates are non-empty")
            }
            SelectionStrategy::RoundRobin => {
                candidates.sort_by(|a, b| a.id.cmp(&b.id));
                let index = self.round_robin_cursor % candidates.len();
                self.round_robin_cursor = self.round_robin_cursor.wrapping_add(1);
                candidates.remove(index)
            }
            SelectionStrategy::Random => {
                // The runtime can randomize insertion order when true random
                // routing is desired; the cursor keeps this core deterministic.
                candidates.sort_by(|a, b| a.id.cmp(&b.id));
                let index = self.round_robin_cursor % candidates.len();
                self.round_robin_cursor = self.round_robin_cursor.wrapping_add(1);
                candidates.remove(index)
            }
        };

        if let Some(session) = session_id.filter(|_| policy.session_sticky) {
            if !preserve_binding {
                self.sessions.insert(session.to_owned(), chosen.id.clone());
            }
        }
        if let Some(account) = self.pool.get_mut(&chosen.id) {
            account.last_used_at_ms = Some(now_ms);
        }
        Ok(RouteDecision {
            account_id: chosen.id,
            attempt,
        })
    }

    fn matches(&self, account_id: &str, policy: &RoutePolicy, now_ms: u64) -> bool {
        self.pool
            .get(account_id)
            .map(|account| self.matches_account(account, policy, now_ms))
            .unwrap_or(false)
    }

    fn matches_account(
        &self,
        account: &ProviderAccount,
        policy: &RoutePolicy,
        now_ms: u64,
    ) -> bool {
        account.is_available(now_ms)
            && (policy.allowed_providers.is_empty()
                || policy
                    .allowed_providers
                    .iter()
                    .any(|p| p == &account.provider))
            && policy
                .preferred_model
                .as_ref()
                .map(|model| model == &account.model)
                .unwrap_or(true)
    }

    pub fn record_success(
        &mut self,
        account_id: &str,
        session_id: Option<&str>,
        now_ms: u64,
    ) -> Result<(), RouteError> {
        let account = self
            .pool
            .get_mut(account_id)
            .ok_or_else(|| RouteError::AccountNotFound(account_id.into()))?;
        account.failure_count = 0;
        account.cooldown_until_ms = None;
        account.last_used_at_ms = Some(now_ms);
        if let Some(session) = session_id {
            self.sessions.insert(session.into(), account_id.into());
        }
        Ok(())
    }

    pub fn record_failure(
        &mut self,
        account_id: &str,
        policy: &RoutePolicy,
        now_ms: u64,
    ) -> Result<(), RouteError> {
        let account = self
            .pool
            .get_mut(account_id)
            .ok_or_else(|| RouteError::AccountNotFound(account_id.into()))?;
        account.failure_count = account.failure_count.saturating_add(1);
        account.cooldown_until_ms = Some(now_ms.saturating_add(policy.cooldown_ms));
        Ok(())
    }

    /// Runs a bounded series of attempts.  The closure owns transport details
    /// (HTTP, image generation, etc.); this method only handles account choice,
    /// cooldown state, session stickiness, and attempt limits.  It never sleeps
    /// so a Tauri command can apply `policy.backoff_for_attempt` asynchronously.
    pub fn run_with_retry<T, E, F>(
        &mut self,
        session_id: Option<&str>,
        policy: &RoutePolicy,
        now_ms: u64,
        mut invoke: F,
    ) -> Result<T, RetryError<E>>
    where
        F: FnMut(&ProviderAccount, u32) -> Result<T, E>,
    {
        let policy = policy.normalized();
        let mut excluded = HashSet::new();
        let mut attempts = 0;
        let mut last_error = None;
        while attempts < policy.max_attempts {
            let attempt = attempts + 1;
            let decision =
                match self.select_excluding(session_id, &policy, now_ms, &excluded, attempt) {
                    Ok(decision) => decision,
                    Err(RouteError::NoAvailableAccount) => {
                        return Err(RetryError::NoAvailableAccount { attempts });
                    }
                    Err(RouteError::AccountNotFound(_id)) => {
                        return Err(RetryError::NoAvailableAccount { attempts });
                    }
                };
            let account = self
                .pool
                .get(&decision.account_id)
                .expect("selection returned an account")
                .clone();
            attempts = attempt;
            match invoke(&account, attempt) {
                Ok(value) => {
                    let _ = self.record_success(&account.id, session_id, now_ms);
                    return Ok(value);
                }
                Err(error) => {
                    let _ = self.record_failure(&account.id, &policy, now_ms);
                    excluded.insert(account.id);
                    last_error = Some(error);
                }
            }
        }
        Err(RetryError::Exhausted {
            last_error: last_error.expect("at least one attempt ran"),
            attempts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AccountPool, ProviderAccount, RoutePolicy, SelectionStrategy};

    fn account(id: &str) -> ProviderAccount {
        ProviderAccount::new(id, "demo", "image-v1", "https://example.invalid")
    }

    #[test]
    fn sticky_sessions_reuse_account_until_cooldown() {
        let mut pool = AccountPool::new();
        pool.insert(account("a")).unwrap();
        pool.insert(account("b")).unwrap();
        let mut router = Router::new(pool);
        let policy = RoutePolicy::default();
        let first = router.select(Some("session"), &policy, 100).unwrap();
        let second = router.select(Some("session"), &policy, 200).unwrap();
        assert_eq!(first.account_id, second.account_id);
        router
            .record_failure(&first.account_id, &policy, 300)
            .unwrap();
        let fallback = router.select(Some("session"), &policy, 301).unwrap();
        assert_ne!(fallback.account_id, first.account_id);
        let recovered = router.select(Some("session"), &policy, 30_301).unwrap();
        assert_eq!(recovered.account_id, first.account_id);
    }

    #[test]
    fn weighted_strategy_uses_account_weights() {
        let mut pool = AccountPool::new();
        let mut a = account("a");
        a.weight = 2;
        pool.insert(a).unwrap();
        pool.insert(account("b")).unwrap();
        let mut router = Router::new(pool);
        let mut seen = Vec::new();
        let policy = RoutePolicy {
            strategy: SelectionStrategy::Weighted,
            session_sticky: false,
            ..Default::default()
        };
        for _ in 0..3 {
            seen.push(router.select(None, &policy, 1).unwrap().account_id);
        }
        assert_eq!(seen, vec!["a", "a", "b"]);
    }

    #[test]
    fn retries_are_finite_and_switch_accounts() {
        let mut pool = AccountPool::new();
        pool.insert(account("a")).unwrap();
        pool.insert(account("b")).unwrap();
        let mut router = Router::new(pool);
        let policy = RoutePolicy {
            max_attempts: 2,
            cooldown_ms: 100,
            session_sticky: false,
            ..Default::default()
        };
        let mut called = Vec::new();
        let result = router.run_with_retry(None, &policy, 1, |account, attempt| {
            called.push((account.id.clone(), attempt));
            Err::<(), _>("failed")
        });
        assert!(matches!(
            result,
            Err(RetryError::Exhausted { attempts: 2, .. })
        ));
        assert_eq!(called.len(), 2);
        assert_ne!(called[0].0, called[1].0);
    }
}
