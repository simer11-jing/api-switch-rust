use serde::{Deserialize, Serialize};

// ============================================================
// API Entry (Pool)
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEntry {
    pub id: String,
    pub channel_id: String,
    pub channel_name: Option<String>,
    pub model: String,
    pub display_name: Option<String>,
    pub enabled: bool,
    pub priority: i32,
    pub sort_index: i32,
    pub weight: i32,
    pub response_ms: Option<String>,
    pub cooldown_until: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateEntry {
    pub channel_id: String,
    pub model: String,
    pub display_name: Option<String>,
    pub priority: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ReorderEntries {
    pub ordered_ids: Vec<String>,
}

// ============================================================
// Dashboard Stats
// ============================================================
#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub total_requests: i64,
    pub today_requests: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub today_prompt_tokens: i64,
    pub today_completion_tokens: i64,
}

#[derive(Debug, Serialize)]
pub struct ChartDataPoint {
    pub time: String,
    pub model: String,
    pub value: i64,
}

#[derive(Debug, Serialize)]
pub struct ModelRanking {
    pub model: String,
    pub count: i64,
    pub tokens: i64,
}

// ============================================================
// Channel
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub api_type: String,
    pub base_url: String,
    pub api_key: String,
    pub models: String,
    pub enabled: bool,
    pub priority: i32,
    pub weight: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateChannel {
    pub name: String,
    pub api_type: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub models: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default = "default_weight")]
    pub weight: i32,
}

fn default_weight() -> i32 { 1 }

#[derive(Debug, Deserialize)]
pub struct UpdateChannel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<i32>,
}

// ============================================================
// API Key
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub key: String,
    pub usage_count: i64,
    pub usage_limit: i64,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKey {
    #[serde(default = "default_key_name")]
    pub name: String,
    #[serde(default)]
    pub usage_limit: i64,
}

fn default_key_name() -> String { "default".into() }

// ============================================================
// Request Log
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLog {
    pub id: String,
    pub channel_id: Option<String>,
    pub channel_name: Option<String>,
    pub model: Option<String>,
    pub api_key_id: Option<String>,
    pub request_type: String,
    pub status_code: i32,
    pub latency_ms: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct LogStats {
    pub total: i64,
    pub success: i64,
    pub errors: i64,
    pub today: i64,
}

// ============================================================
// Settings
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_threshold")]
    pub circuit_breaker_threshold: i32,
    #[serde(default = "default_reset_time")]
    pub circuit_breaker_reset_time: i32,
    #[serde(default = "default_retry")]
    pub retry_times: i32,
    #[serde(default = "default_timeout")]
    pub timeout: i32,
    #[serde(default)]
    pub auto_select_new_models: bool,
    #[serde(default)]
    pub max_tokens_per_month: i64,
    #[serde(default)]
    pub default_model: String,
}

fn default_threshold() -> i32 { 5 }
fn default_reset_time() -> i32 { 300 }
fn default_retry() -> i32 { 3 }
fn default_timeout() -> i32 { 60000 }

impl Default for Settings {
    fn default() -> Self {
        Self {
            circuit_breaker_threshold: default_threshold(),
            circuit_breaker_reset_time: default_reset_time(),
            retry_times: default_retry(),
            timeout: default_timeout(),
            auto_select_new_models: true,
            max_tokens_per_month: 0,
            default_model: String::new(),
        }
    }
}

// ============================================================
// Auth
// ============================================================
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

// ============================================================
// Chat Completion (proxy)
// ============================================================
#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<serde_json::Value>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
