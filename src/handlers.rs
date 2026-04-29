use crate::models::*;
use crate::SharedState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

// ============================================================
// Auth helper
// ============================================================
async fn require_auth(
    headers: &HeaderMap,
    state: &SharedState,
) -> Result<String, StatusCode> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header.strip_prefix("Bearer ").ok_or(StatusCode::UNAUTHORIZED)?;
    let s = state.read().await;
    s.db.verify_session(token)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)
}

// ============================================================
// Health
// ============================================================
pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok", "version": "0.5.0-rust" }))
}

// ============================================================
// Auth Routes
// ============================================================
pub fn auth_routes() -> axum::Router<SharedState> {
    axum::Router::new()
        .route("/api/login", axum::routing::post(login))
        .route("/api/logout", axum::routing::post(logout))
        .route("/api/change-password", axum::routing::post(change_password))
        .route("/api/me", axum::routing::get(me))
}

async fn login(
    State(state): State<SharedState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let s = state.read().await;
    match s.db.login(&req.username, &req.password) {
        Ok(Some((_user_id, username))) => {
            let token = crate::auth::generate_token();
            let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();
            if let Err(e) = s.db.store_session(&token, &username, &expires_at) {
                tracing::error!("Failed to store session: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "Internal error" }))).into_response();
            }
            Json(serde_json::json!({
                "token": token,
                "expires_at": expires_at,
                "username": username
            }))
            .into_response()
        }
        _ => (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Invalid credentials" }))).into_response(),
    }
}

async fn logout(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Some(auth_header) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            let s = state.read().await;
            let _ = s.db.delete_session(token);
        }
    }
    Json(serde_json::json!({ "success": true })).into_response()
}

async fn change_password(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    let username = match require_auth(&headers, &state).await {
        Ok(u) => u,
        Err(_) => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Unauthorized" }))).into_response(),
    };
    let s = state.read().await;
    match s.db.change_password(&username, &req.old_password, &req.new_password) {
        Ok(true) => Json(serde_json::json!({ "success": true })).into_response(),
        _ => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "Invalid old password" }))).into_response(),
    }
}

async fn me(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    match require_auth(&headers, &state).await {
        Ok(username) => Json(serde_json::json!({ "username": username })).into_response(),
        Err(_) => (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Unauthorized" }))).into_response(),
    }
}

// ============================================================
// Channel Routes
// ============================================================
pub fn channel_routes() -> axum::Router<SharedState> {
    axum::Router::new()
        .route("/api/channels/:id/toggle", axum::routing::post(toggle_channel))
        .route("/api/channels/:id", axum::routing::get(get_channel).put(update_channel).delete(delete_channel))
        .route("/api/channels", axum::routing::get(list_channels).post(create_channel))
}

async fn list_channels(State(state): State<SharedState>, headers: HeaderMap) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.list_channels() {
        Ok(channels) => Json(channels).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn create_channel(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CreateChannel>,
) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.create_channel(&req) {
        Ok(ch) => (StatusCode::CREATED, Json(ch)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn get_channel(State(state): State<SharedState>, headers: HeaderMap, Path(id): Path<String>) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.get_channel(&id) {
        Ok(Some(ch)) => Json(ch).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Not found" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn update_channel(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateChannel>,
) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.update_channel(&id, &req) {
        Ok(Some(ch)) => Json(ch).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Not found" }))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn delete_channel(State(state): State<SharedState>, headers: HeaderMap, Path(id): Path<String>) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.delete_channel(&id) {
        Ok(true) => Json(serde_json::json!({ "success": true })).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Not found" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn toggle_channel(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let enabled = body.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    let s = state.read().await;
    match s.db.toggle_channel(&id, enabled) {
        Ok(true) => Json(serde_json::json!({ "success": true })).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Not found" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

// ============================================================
// API Key Routes
// ============================================================
pub fn key_routes() -> axum::Router<SharedState> {
    axum::Router::new()
        .route("/api/keys/:id/toggle", axum::routing::post(toggle_key))
        .route("/api/keys/:id", axum::routing::delete(delete_key))
        .route("/api/keys", axum::routing::get(list_keys).post(create_key))
}

async fn list_keys(State(state): State<SharedState>, headers: HeaderMap) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.list_api_keys() {
        Ok(keys) => Json(keys).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn create_key(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CreateApiKey>,
) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.create_api_key(&req) {
        Ok(key) => (StatusCode::CREATED, Json(key)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn delete_key(State(state): State<SharedState>, headers: HeaderMap, Path(id): Path<String>) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.delete_api_key(&id) {
        Ok(true) => Json(serde_json::json!({ "success": true })).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Not found" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn toggle_key(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let enabled = body.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    let s = state.read().await;
    match s.db.toggle_api_key(&id, enabled) {
        Ok(true) => Json(serde_json::json!({ "success": true })).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Not found" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

// ============================================================
// Log Routes
// ============================================================
#[derive(Debug, Deserialize)]
pub struct LogQuery {
    pub limit: Option<i64>,
    pub page: Option<i64>,
}

pub fn log_routes() -> axum::Router<SharedState> {
    axum::Router::new()
        .route("/api/logs", axum::routing::get(list_logs))
        .route("/api/logs/stats", axum::routing::get(log_stats))
        .route("/api/logs/clear", axum::routing::post(clear_logs))
}

async fn list_logs(State(state): State<SharedState>, headers: HeaderMap, Query(q): Query<LogQuery>) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let limit = q.limit.unwrap_or(50).min(500);
    let page = q.page.unwrap_or(0);
    let offset = page * limit;
    let s = state.read().await;
    match s.db.list_logs(limit, offset, None, None, None) {
        Ok((logs, total)) => Json(serde_json::json!({ "logs": logs, "total": total, "page": page, "limit": limit })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn log_stats(State(state): State<SharedState>, headers: HeaderMap) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.get_log_stats() {
        Ok(stats) => Json(stats).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn clear_logs(State(state): State<SharedState>, headers: HeaderMap) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.clear_logs() {
        Ok(()) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

// ============================================================
// Entry (Pool) Routes
// ============================================================
pub fn entry_routes() -> axum::Router<SharedState> {
    axum::Router::new()
        .route("/api/entries/reorder", axum::routing::post(reorder_entries))
        .route("/api/entries/:id/toggle", axum::routing::post(toggle_entry))
        .route("/api/entries/:id", axum::routing::delete(delete_entry))
        .route("/api/entries", axum::routing::get(list_entries).post(create_entry))
}

async fn list_entries(State(state): State<SharedState>, headers: HeaderMap) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.list_entries() {
        Ok(entries) => Json(entries).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn create_entry(State(state): State<SharedState>, headers: HeaderMap, Json(req): Json<CreateEntry>) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.create_entry(&req) {
        Ok(entry) => (StatusCode::CREATED, Json(entry)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn toggle_entry(State(state): State<SharedState>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let enabled = body.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    let s = state.read().await;
    match s.db.toggle_entry(&id, enabled) {
        Ok(true) => Json(serde_json::json!({ "success": true })).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Not found" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn delete_entry(State(state): State<SharedState>, headers: HeaderMap, Path(id): Path<String>) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.delete_entry(&id) {
        Ok(true) => Json(serde_json::json!({ "success": true })).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Not found" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn reorder_entries(State(state): State<SharedState>, headers: HeaderMap, Json(body): Json<ReorderEntries>) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.reorder_entries(&body.ordered_ids) {
        Ok(()) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

// ============================================================
// Dashboard Routes
// ============================================================
pub fn dashboard_routes() -> axum::Router<SharedState> {
    axum::Router::new()
        .route("/api/dashboard/stats", axum::routing::get(dashboard_stats))
        .route("/api/dashboard/models", axum::routing::get(model_ranking))
        .route("/api/dashboard/chart", axum::routing::get(chart_data))
}

async fn dashboard_stats(State(state): State<SharedState>, headers: HeaderMap) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.get_dashboard_stats() {
        Ok(stats) => Json(stats).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn model_ranking(State(state): State<SharedState>, headers: HeaderMap) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.get_model_ranking(10) {
        Ok(ranking) => Json(ranking).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct ChartQuery {
    pub granularity: Option<String>,
}

async fn chart_data(State(state): State<SharedState>, headers: HeaderMap, Query(q): Query<ChartQuery>) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let granularity = q.granularity.as_deref().unwrap_or("day");
    let s = state.read().await;
    match s.db.get_chart_data(granularity) {
        Ok(data) => Json(data).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

// ============================================================
// Channel Test & Model Discovery
// ============================================================
pub fn channel_action_routes() -> axum::Router<SharedState> {
    axum::Router::new()
        .route("/api/channels/:id/test", axum::routing::post(test_channel))
        .route("/api/channels/:id/discover", axum::routing::post(discover_models))
}

async fn test_channel(State(state): State<SharedState>, headers: HeaderMap, Path(id): Path<String>) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    let channel = match s.db.get_channel(&id) {
        Ok(Some(ch)) => ch,
        _ => return (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Channel not found" }))).into_response(),
    };
    let client = reqwest::Client::new();
    let start = std::time::Instant::now();
    let result = client
        .post(&format!("{}/v1/chat/completions", channel.base_url.trim_end_matches('/')))
        .header("Authorization", format!("Bearer {}", channel.api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 1
        }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await;
    match result {
        Ok(resp) => {
            let latency = start.elapsed().as_millis();
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            Json(serde_json::json!({
                "success": status >= 200 && status < 300,
                "status": status,
                "latency_ms": latency,
                "response": body.chars().take(200).collect::<String>()
            })).into_response()
        }
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        })).into_response(),
    }
}

async fn discover_models(State(state): State<SharedState>, headers: HeaderMap, Path(id): Path<String>) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    let channel = match s.db.get_channel(&id) {
        Ok(Some(ch)) => ch,
        _ => return (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Channel not found" }))).into_response(),
    };
    let client = reqwest::Client::new();
    let result = client
        .get(&format!("{}/v1/models", channel.base_url.trim_end_matches('/')))
        .header("Authorization", format!("Bearer {}", channel.api_key))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await;
    match result {
        Ok(resp) => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let models: Vec<String> = body.get("data")
                .and_then(|d| d.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            // Update channel models
            if !models.is_empty() {
                let _ = s.db.update_channel_models(&id, &models);
            }
            Json(serde_json::json!({ "models": models, "count": models.len() })).into_response()
        }
        Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

// ============================================================
// Chat Test Route
// ============================================================
pub fn chat_test_routes() -> axum::Router<SharedState> {
    axum::Router::new()
        .route("/api/chat/test", axum::routing::post(chat_test))
}

#[derive(Debug, Deserialize)]
pub struct ChatTestRequest {
    pub model: String,
    pub message: String,
}

async fn chat_test(State(state): State<SharedState>, headers: HeaderMap, Json(req): Json<ChatTestRequest>) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    let channels = s.db.list_channels().unwrap_or_default();
    let channel = channels.iter()
        .filter(|c| c.enabled)
        .find(|c| {
            if let Ok(models) = serde_json::from_str::<Vec<String>>(&c.models) {
                models.contains(&req.model) || models.is_empty()
            } else { true }
        });
    match channel {
        Some(ch) => {
            let client = reqwest::Client::new();
            let result = client
                .post(&format!("{}/v1/chat/completions", ch.base_url.trim_end_matches('/')))
                .header("Authorization", format!("Bearer {}", ch.api_key))
                .header("Content-Type", "application/json")
                .json(&serde_json::json!({
                    "model": req.model,
                    "messages": [{"role": "user", "content": req.message}],
                    "max_tokens": 100
                }))
                .timeout(std::time::Duration::from_secs(30))
                .send()
                .await;
            match result {
                Ok(resp) => {
                    let body = resp.text().await.unwrap_or_default();
                    body.into_response()
                }
                Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
            }
        }
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "No available channel for this model" }))).into_response(),
    }
}

// ============================================================
// Settings Routes
// ============================================================
pub fn settings_routes() -> axum::Router<SharedState> {
    axum::Router::new()
        .route("/api/settings", axum::routing::get(get_settings).put(update_settings))
}

async fn get_settings(State(state): State<SharedState>, headers: HeaderMap) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.get_settings() {
        Ok(settings) => Json(settings).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn update_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(settings): Json<Settings>,
) -> impl IntoResponse {
    let _ = require_auth(&headers, &state).await;
    let s = state.read().await;
    match s.db.update_settings(&settings) {
        Ok(()) => Json(settings).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

// ============================================================
// Proxy Routes
// ============================================================
pub fn proxy_routes() -> axum::Router<SharedState> {
    axum::Router::new()
        .route("/v1/chat/completions", axum::routing::post(chat_completions))
        .route("/v1/models", axum::routing::get(list_models))
}

async fn list_models(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.read().await;
    let channels = match s.db.list_channels() {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    };

    let mut model_list = Vec::new();
    for ch in channels.iter().filter(|c| c.enabled) {
        if let Ok(models) = serde_json::from_str::<Vec<String>>(&ch.models) {
            for m in models {
                model_list.push(serde_json::json!({
                    "id": m,
                    "object": "model",
                    "owned_by": ch.name
                }));
            }
        }
    }

    Json(serde_json::json!({ "object": "list", "data": model_list })).into_response()
}

async fn chat_completions(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ChatCompletionRequest>,
) -> impl IntoResponse {
    let s = state.read().await;

    // API Key auth
    let keys = s.db.list_api_keys().unwrap_or_default();
    if !keys.is_empty() {
        let auth_header = headers.get("authorization").and_then(|v| v.to_str().ok());
        let key_str = auth_header.and_then(|h| h.strip_prefix("Bearer "));
        match key_str {
            Some(k) => {
                match s.db.validate_api_key(k) {
                    Ok(Some(api_key)) => {
                        let _ = s.db.increment_key_usage(&api_key.id);
                    }
                    _ => {
                        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
                            "error": { "message": "Invalid API key", "type": "auth_error" }
                        })))
                        .into_response();
                    }
                }
            }
            None => {
                return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
                    "error": { "message": "Missing API key", "type": "auth_error" }
                })))
                .into_response();
            }
        }
    }

    let settings = s.db.get_settings().unwrap_or_default();

    // Get all entries (API Pool) - this is the key improvement from original API-Switch
    let entries = s.db.list_entries().unwrap_or_default();
    let channels = s.db.list_channels().unwrap_or_default();
    
    // Resolve which entries to use based on model
    let resolved_entries = if req.model.eq_ignore_ascii_case("auto") {
        // AUTO: use enabled + available entries sorted by sort_index
        entries.iter()
            .filter(|e| e.enabled)
            .filter(|e| futures::executor::block_on(s.breaker.is_available(&e.channel_id)))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        // Exact match: try matched entries first, then fallback to auto pool
        let matched: Vec<_> = entries.iter()
            .filter(|e| e.enabled && e.model == req.model)
            .filter(|e| futures::executor::block_on(s.breaker.is_available(&e.channel_id)))
            .cloned()
            .collect();
        
        if matched.is_empty() {
            // Fallback to auto pool
            entries.iter()
                .filter(|e| e.enabled)
                .filter(|e| futures::executor::block_on(s.breaker.is_available(&e.channel_id)))
                .cloned()
                .collect()
        } else {
            // Append auto pool entries as fallback
            let mut result = matched;
            for e in entries.iter().filter(|e| e.enabled) {
                if !result.iter().any(|r| r.id == e.id) {
                    result.push(e.clone());
                }
            }
            result
        }
    };

    if resolved_entries.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": { "message": format!("No available entry for model: {}", req.model), "type": "invalid_request_error" }
        })))
        .into_response();
    }

    let client = reqwest::Client::new();
    let max_retries = settings.retry_times.max(1) as usize;

    // Build the request body
    let mut body_map = req.extra.clone();
    body_map.insert("model".to_string(), serde_json::Value::String(req.model.clone()));
    body_map.insert("messages".to_string(), serde_json::Value::Array(req.messages.clone()));
    if req.stream {
        body_map.insert("stream".to_string(), serde_json::Value::Bool(true));
    }

    for (attempt, entry) in resolved_entries.iter().cycle().enumerate().take(max_retries) {
        // Get the channel for this entry
        let channel = match channels.iter().find(|c| c.id == entry.channel_id) {
            Some(ch) => ch,
            None => continue,
        };
        
        let start = std::time::Instant::now();
        let upstream_url = format!("{}/v1/chat/completions", channel.base_url.trim_end_matches('/'));

        let result = client
            .post(&upstream_url)
            .header("Authorization", format!("Bearer {}", channel.api_key))
            .header("Content-Type", "application/json")
            .json(&body_map)
            .timeout(std::time::Duration::from_millis(settings.timeout as u64))
            .send()
            .await;

        match result {
            Ok(resp) => {
                let status = resp.status().as_u16() as i32;
                let latency = start.elapsed().as_millis() as i64;

                if resp.status().is_success() {
                    let breaker_id = channel.id.clone();
                    let log_channel_id = channel.id.clone();
                    let log_channel_name = channel.name.clone();
                    let log_model = req.model.clone();

                    if req.stream {
                        let body = resp.bytes().await.unwrap_or_default();
                        let (prompt_tokens, completion_tokens) = parse_stream_tokens(&String::from_utf8_lossy(&body));
                        let state2 = state.clone();
                        tokio::spawn(async move {
                            let s2 = state2.read().await;
                            s2.breaker.record_success(&breaker_id).await;
                            let log = RequestLog {
                                id: uuid::Uuid::new_v4().to_string(),
                                channel_id: Some(log_channel_id),
                                channel_name: Some(log_channel_name),
                                model: Some(log_model),
                                api_key_id: None,
                                request_type: "chat".into(),
                                status_code: status,
                                latency_ms: latency,
                                                        prompt_tokens,
                        completion_tokens,
                                error: None,
                                created_at: chrono::Utc::now().to_rfc3339(),
                            };
                            let _ = s2.db.create_log(&log);
                        });
                        return (
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                            body,
                        )
                            .into_response();
                    } else {
                        let body = resp.text().await.unwrap_or_default();
                        let (prompt_tokens, completion_tokens) = parse_response_tokens(&body);
                        let state2 = state.clone();
                        tokio::spawn(async move {
                            let s2 = state2.read().await;
                            s2.breaker.record_success(&breaker_id).await;
                            let log = RequestLog {
                                id: uuid::Uuid::new_v4().to_string(),
                                channel_id: Some(log_channel_id),
                                channel_name: Some(log_channel_name),
                                model: Some(log_model),
                                api_key_id: None,
                                request_type: "chat".into(),
                                status_code: status,
                                latency_ms: latency,
                                                        prompt_tokens,
                        completion_tokens,
                                error: None,
                                created_at: chrono::Utc::now().to_rfc3339(),
                            };
                            let _ = s2.db.create_log(&log);
                        });
                        return (StatusCode::OK, body).into_response();
                    }
                } else {
                    let error_body = resp.text().await.unwrap_or_default();
                    let error_body_clone = error_body.clone();
                    let breaker_id = channel.id.clone();
                    let state2 = state.clone();
                    tokio::spawn(async move {
                        let s2 = state2.read().await;
                        s2.breaker.record_failure(&breaker_id).await;
                    });

                    if attempt >= max_retries - 1 {
                        let state2 = state.clone();
                        let log_channel_id = channel.id.clone();
                        let log_channel_name = channel.name.clone();
                        let log_model = req.model.clone();
                        tokio::spawn(async move {
                            let s2 = state2.read().await;
                            let log = RequestLog {
                                id: uuid::Uuid::new_v4().to_string(),
                                channel_id: Some(log_channel_id),
                                channel_name: Some(log_channel_name),
                                model: Some(log_model),
                                api_key_id: None,
                                request_type: "chat".into(),
                                status_code: status,
                                latency_ms: latency,
                                prompt_tokens: 0,
                                completion_tokens: 0,
                                error: Some(error_body_clone),
                                created_at: chrono::Utc::now().to_rfc3339(),
                            };
                            let _ = s2.db.create_log(&log);
                        });
                        return (
                            StatusCode::from_u16(status as u16).unwrap_or(StatusCode::BAD_GATEWAY),
                            error_body,
                        )
                            .into_response();
                    }
                }
            }
            Err(e) => {
                let breaker_id = channel.id.clone();
                let state2 = state.clone();
                tokio::spawn(async move {
                    let s2 = state2.read().await;
                    s2.breaker.record_failure(&breaker_id).await;
                });

                if attempt >= max_retries - 1 {
                    let state2 = state.clone();
                    let log_channel_id = channel.id.clone();
                    let log_channel_name = channel.name.clone();
                    let log_model = req.model.clone();
                    let err_msg = e.to_string();
                    tokio::spawn(async move {
                        let s2 = state2.read().await;
                        let log = RequestLog {
                            id: uuid::Uuid::new_v4().to_string(),
                            channel_id: Some(log_channel_id),
                            channel_name: Some(log_channel_name),
                            model: Some(log_model),
                            api_key_id: None,
                            request_type: "chat".into(),
                            status_code: 0,
                            latency_ms: start.elapsed().as_millis() as i64,
                            prompt_tokens: 0,
                            completion_tokens: 0,
                            error: Some(err_msg),
                            created_at: chrono::Utc::now().to_rfc3339(),
                        };
                        let _ = s2.db.create_log(&log);
                    });
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(serde_json::json!({
                            "error": { "message": e.to_string(), "type": "upstream_error" }
                        })),
                    )
                        .into_response();
                }
            }
        }
    }

    (
        StatusCode::BAD_GATEWAY,
        Json(serde_json::json!({
            "error": { "message": "All channels failed", "type": "upstream_error" }
        })),
    )
        .into_response()
}

// ============================================================
// Token extraction helpers
// ============================================================
fn parse_response_tokens(body: &str) -> (i64, i64) {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(usage) = v.get("usage") {
            let prompt = usage.get("prompt_tokens").and_then(|t| t.as_i64()).unwrap_or(0);
            let completion = usage.get("completion_tokens").and_then(|t| t.as_i64()).unwrap_or(0);
            return (prompt, completion);
        }
    }
    (0, 0)
}

fn parse_stream_tokens(body: &str) -> (i64, i64) {
    let mut prompt_tokens = 0i64;
    let mut completion_tokens = 0i64;
    for line in body.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" { continue; }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                // OpenAI format
                if let Some(usage) = v.get("usage") {
                    if let Some(p) = usage.get("prompt_tokens").and_then(|t| t.as_i64()) {
                        prompt_tokens = p;
                    }
                    if let Some(c) = usage.get("completion_tokens").and_then(|t| t.as_i64()) {
                        completion_tokens = c;
                    }
                }
                // DeepSeek format with reasoning_content
                if let Some(usage) = v.get("usage") {
                    if let Some(p) = usage.get("prompt_tokens").and_then(|t| t.as_i64()) {
                        prompt_tokens = p;
                    }
                    if let Some(c) = usage.get("completion_tokens").and_then(|t| t.as_i64()) {
                        completion_tokens = c;
                    }
                }
            }
        }
    }
    (prompt_tokens, completion_tokens)
}
