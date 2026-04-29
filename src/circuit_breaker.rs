use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

struct BreakerState {
    failures: u32,
    last_failure: Option<Instant>,
    open: bool,
}

pub struct CircuitBreakerManager {
    states: Arc<RwLock<HashMap<String, BreakerState>>>,
    default_threshold: u32,
    default_reset_duration: Duration,
}

impl CircuitBreakerManager {
    pub fn new() -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
            default_threshold: 5,
            default_reset_duration: Duration::from_secs(300),
        }
    }

    pub async fn is_available(&self, channel_id: &str) -> bool {
        let states = self.states.read().await;
        if let Some(state) = states.get(channel_id) {
            if state.open {
                // Check if reset duration has elapsed
                if let Some(last) = state.last_failure {
                    if last.elapsed() > self.default_reset_duration {
                        return true; // Half-open: allow one request
                    }
                }
                return false;
            }
        }
        true
    }

    pub async fn record_success(&self, channel_id: &str) {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(channel_id) {
            state.failures = 0;
            state.open = false;
            state.last_failure = None;
        }
    }

    pub async fn record_failure(&self, channel_id: &str) {
        let mut states = self.states.write().await;
        let state = states.entry(channel_id.to_string()).or_insert(BreakerState {
            failures: 0,
            last_failure: None,
            open: false,
        });
        state.failures += 1;
        state.last_failure = Some(Instant::now());
        if state.failures >= self.default_threshold {
            state.open = true;
            tracing::warn!("🔴 Circuit breaker OPEN for channel {}", channel_id);
        }
    }
}
