use rusqlite::{params, Connection};
use crate::models::*;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn: Mutex::new(conn) };
        db.init_tables()?;
        db.seed_defaults()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sessions (
                token TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                expires_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS channels (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                api_type TEXT NOT NULL DEFAULT 'openai',
                base_url TEXT NOT NULL,
                api_key TEXT NOT NULL DEFAULT '',
                models TEXT NOT NULL DEFAULT '[]',
                enabled INTEGER NOT NULL DEFAULT 1,
                priority INTEGER NOT NULL DEFAULT 0,
                weight INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS entries (
                id TEXT PRIMARY KEY,
                channel_id TEXT NOT NULL,
                model TEXT NOT NULL,
                display_name TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                priority INTEGER NOT NULL DEFAULT 0,
                sort_index INTEGER NOT NULL DEFAULT 0,
                weight INTEGER NOT NULL DEFAULT 1,
                response_ms TEXT,
                cooldown_until INTEGER,
                created_at TEXT NOT NULL,
                FOREIGN KEY (channel_id) REFERENCES channels(id)
            );
            CREATE TABLE IF NOT EXISTS api_keys (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                key TEXT UNIQUE NOT NULL,
                usage_count INTEGER NOT NULL DEFAULT 0,
                usage_limit INTEGER NOT NULL DEFAULT 0,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS logs (
                id TEXT PRIMARY KEY,
                channel_id TEXT,
                channel_name TEXT,
                model TEXT,
                api_key_id TEXT,
                request_type TEXT NOT NULL DEFAULT 'chat',
                status_code INTEGER NOT NULL,
                latency_ms INTEGER NOT NULL DEFAULT 0,
                prompt_tokens INTEGER NOT NULL DEFAULT 0,
                completion_tokens INTEGER NOT NULL DEFAULT 0,
                error TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                circuit_breaker_threshold INTEGER NOT NULL DEFAULT 5,
                circuit_breaker_reset_time INTEGER NOT NULL DEFAULT 300,
                retry_times INTEGER NOT NULL DEFAULT 3,
                timeout INTEGER NOT NULL DEFAULT 60000,
                auto_select_new_models INTEGER NOT NULL DEFAULT 1,
                max_tokens_per_month INTEGER NOT NULL DEFAULT 0,
                default_model TEXT NOT NULL DEFAULT ''
            );
        ")?;
        Ok(())
    }

    fn seed_defaults(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        // Default user
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
        if count == 0 {
            let hash = crate::auth::hash_password("admin"); // 默认密码
            conn.execute(
                "INSERT INTO users (id, username, password_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![uuid::Uuid::new_v4().to_string(), "root", hash, chrono::Utc::now().to_rfc3339()],
            )?;
        }
        // Default settings
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0))?;
        if count == 0 {
            conn.execute(
                "INSERT INTO settings (id, circuit_breaker_threshold, circuit_breaker_reset_time, retry_times, timeout, auto_select_new_models, max_tokens_per_month, default_model) VALUES (1, 5, 300, 3, 60000, 1, 0, '')",
                [],
            )?;
        }
        Ok(())
    }

    // ============================================================
    // Auth
    // ============================================================
    pub fn login(&self, username: &str, password: &str) -> Result<Option<(String, String)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let hash = crate::auth::hash_password(password);
        let result = conn.query_row(
            "SELECT id, username FROM users WHERE username = ?1 AND password_hash = ?2",
            params![username, hash],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        );
        match result {
            Ok(pair) => Ok(Some(pair)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn change_password(&self, username: &str, old_password: &str, new_password: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let old_hash = crate::auth::hash_password(old_password);
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE username = ?1 AND password_hash = ?2",
            params![username, old_hash],
            |r| r.get(0),
        )?;
        if count == 0 { return Ok(false); }
        let new_hash = crate::auth::hash_password(new_password);
        conn.execute(
            "UPDATE users SET password_hash = ?1 WHERE username = ?2",
            params![new_hash, username],
        )?;
        Ok(true)
    }

    // ============================================================
    // Sessions (token store)
    // ============================================================
    pub fn store_session(&self, token: &str, username: &str, expires_at: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO sessions (token, username, expires_at) VALUES (?1, ?2, ?3)",
            params![token, username, expires_at],
        )?;
        Ok(())
    }

    pub fn verify_session(&self, token: &str) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let result = conn.query_row(
            "SELECT username FROM sessions WHERE token = ?1 AND expires_at > ?2",
            params![token, now],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(u) => Ok(Some(u)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn delete_session(&self, token: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sessions WHERE token = ?1", params![token])?;
        Ok(())
    }

    // ============================================================
    // API Entries (Pool)
    // ============================================================
    pub fn list_entries(&self) -> Result<Vec<ApiEntry>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT e.id, e.channel_id, c.name, e.model, e.display_name, e.enabled, e.priority, e.sort_index, e.weight, e.response_ms, e.cooldown_until, e.created_at FROM entries e LEFT JOIN channels c ON e.channel_id = c.id ORDER BY e.sort_index ASC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ApiEntry {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                channel_name: row.get(2)?,
                model: row.get(3)?,
                display_name: row.get(4)?,
                enabled: row.get::<_, i32>(5)? != 0,
                priority: row.get(6)?,
                sort_index: row.get(7)?,
                weight: row.get(8)?,
                response_ms: row.get(9)?,
                cooldown_until: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    }

    pub fn create_entry(&self, req: &CreateEntry) -> Result<ApiEntry, rusqlite::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        // Get max sort_index
        let max_idx: i32 = conn.query_row("SELECT COALESCE(MAX(sort_index), -1) FROM entries", [], |r| r.get(0))?;
        conn.execute(
            "INSERT INTO entries (id, channel_id, model, display_name, enabled, priority, sort_index, weight, response_ms, cooldown_until, created_at) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, 1, NULL, NULL, ?7)",
            params![id, req.channel_id, req.model, req.display_name, req.priority.unwrap_or(0), max_idx + 1, now],
        )?;
        drop(conn);
        self.list_entries()?.into_iter().find(|e| e.id == id).ok_or(rusqlite::Error::InvalidParameterName("entry not found".into()))
    }

    pub fn toggle_entry(&self, id: &str, enabled: bool) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("UPDATE entries SET enabled = ?1 WHERE id = ?2", params![enabled as i32, id])?;
        Ok(count > 0)
    }

    pub fn reorder_entries(&self, ordered_ids: &[String]) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        for (idx, id) in ordered_ids.iter().enumerate() {
            conn.execute("UPDATE entries SET sort_index = ?1 WHERE id = ?2", params![idx as i32, id])?;
        }
        Ok(())
    }

    pub fn delete_entry(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM entries WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    pub fn update_entry_response_ms(&self, id: &str, response_ms: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE entries SET response_ms = ?1 WHERE id = ?2", params![response_ms, id])?;
        Ok(())
    }

    // ============================================================
    // Dashboard Stats
    // ============================================================
    pub fn get_dashboard_stats(&self) -> Result<DashboardStats, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let total_requests: i64 = conn.query_row("SELECT COUNT(*) FROM logs", [], |r| r.get(0))?;
        let today_start = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let today_requests: i64 = conn.query_row(
            "SELECT COUNT(*) FROM logs WHERE created_at >= ?1",
            params![today_start],
            |r| r.get(0),
        )?;
        let total_prompt_tokens: i64 = conn.query_row("SELECT COALESCE(SUM(prompt_tokens), 0) FROM logs", [], |r| r.get(0))?;
        let total_completion_tokens: i64 = conn.query_row("SELECT COALESCE(SUM(completion_tokens), 0) FROM logs", [], |r| r.get(0))?;
        let today_prompt_tokens: i64 = conn.query_row(
            "SELECT COALESCE(SUM(prompt_tokens), 0) FROM logs WHERE created_at >= ?1",
            params![today_start],
            |r| r.get(0),
        )?;
        let today_completion_tokens: i64 = conn.query_row(
            "SELECT COALESCE(SUM(completion_tokens), 0) FROM logs WHERE created_at >= ?1",
            params![today_start],
            |r| r.get(0),
        )?;
        Ok(DashboardStats {
            total_requests,
            today_requests,
            total_prompt_tokens,
            total_completion_tokens,
            today_prompt_tokens,
            today_completion_tokens,
        })
    }

    pub fn get_model_ranking(&self, limit: i64) -> Result<Vec<ModelRanking>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT model, COUNT(*) as count, SUM(prompt_tokens + completion_tokens) as tokens FROM logs WHERE model IS NOT NULL GROUP BY model ORDER BY count DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(ModelRanking {
                model: row.get(0)?,
                count: row.get(1)?,
                tokens: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    }

    pub fn get_chart_data(&self, granularity: &str) -> Result<Vec<ChartDataPoint>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let fmt = match granularity {
            "day" => "%Y-%m-%d",
            _ => "%Y-%m-%d %H:00",
        };
        let sql = format!(
            "SELECT strftime('{}', created_at) as time, model, COUNT(*) as value FROM logs WHERE model IS NOT NULL GROUP BY time, model ORDER BY time ASC",
            fmt
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(ChartDataPoint {
                time: row.get(0)?,
                model: row.get(1)?,
                value: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    }

    // ============================================================
    // Channels
    // ============================================================
    pub fn list_channels(&self) -> Result<Vec<Channel>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, api_type, base_url, api_key, models, enabled, priority, weight, created_at, updated_at FROM channels ORDER BY priority DESC, weight DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Channel {
                id: row.get(0)?,
                name: row.get(1)?,
                api_type: row.get(2)?,
                base_url: row.get(3)?,
                api_key: row.get(4)?,
                models: row.get(5)?,
                enabled: row.get::<_, i32>(6)? != 0,
                priority: row.get(7)?,
                weight: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    }

    pub fn get_channel(&self, id: &str) -> Result<Option<Channel>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT id, name, api_type, base_url, api_key, models, enabled, priority, weight, created_at, updated_at FROM channels WHERE id = ?1",
            params![id],
            |row| {
                Ok(Channel {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    api_type: row.get(2)?,
                    base_url: row.get(3)?,
                    api_key: row.get(4)?,
                    models: row.get(5)?,
                    enabled: row.get::<_, i32>(6)? != 0,
                    priority: row.get(7)?,
                    weight: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        );
        match result {
            Ok(ch) => Ok(Some(ch)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn create_channel(&self, req: &CreateChannel) -> Result<Channel, rusqlite::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let enabled = 1i32;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO channels (id, name, api_type, base_url, api_key, models, enabled, priority, weight, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![id, req.name, req.api_type, req.base_url, req.api_key, req.models, enabled, req.priority, req.weight, now, now],
        )?;
        drop(conn);
        self.get_channel(&id).map(|opt| opt.unwrap())
    }

    pub fn update_channel(&self, id: &str, req: &UpdateChannel) -> Result<Option<Channel>, rusqlite::Error> {
        if self.get_channel(id)?.is_none() { return Ok(None); }
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let mut sets = Vec::new();
        let mut vals: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref v) = req.name { sets.push("name = ?"); vals.push(Box::new(v.clone())); }
        if let Some(ref v) = req.api_type { sets.push("api_type = ?"); vals.push(Box::new(v.clone())); }
        if let Some(ref v) = req.base_url { sets.push("base_url = ?"); vals.push(Box::new(v.clone())); }
        if let Some(ref v) = req.api_key { sets.push("api_key = ?"); vals.push(Box::new(v.clone())); }
        if let Some(ref v) = req.models { sets.push("models = ?"); vals.push(Box::new(v.clone())); }
        if let Some(v) = req.enabled { sets.push("enabled = ?"); vals.push(Box::new(v as i32)); }
        if let Some(v) = req.priority { sets.push("priority = ?"); vals.push(Box::new(v)); }
        if let Some(v) = req.weight { sets.push("weight = ?"); vals.push(Box::new(v)); }

        sets.push("updated_at = ?");
        vals.push(Box::new(now));

        sets.push("id = id WHERE id = ?");
        vals.push(Box::new(id.to_string()));

        let sql = format!("UPDATE channels SET {} = ?", sets.join(" = ?, "));
        // Simpler approach
        let sql = format!(
            "UPDATE channels SET {} WHERE id = ?",
            sets.iter().map(|s| s.replace(" = ?", "").replace(" WHERE id = ?", "")).collect::<Vec<_>>().join(" = ?, ")
        );

        drop(conn);
        // Use simpler update approach
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE channels SET name = COALESCE(?1, name), api_type = COALESCE(?2, api_type), base_url = COALESCE(?3, base_url), api_key = COALESCE(?4, api_key), models = COALESCE(?5, models), enabled = COALESCE(?6, enabled), priority = COALESCE(?7, priority), weight = COALESCE(?8, weight), updated_at = ?9 WHERE id = ?10",
            params![
                req.name,
                req.api_type,
                req.base_url,
                req.api_key,
                req.models,
                req.enabled.map(|v| v as i32),
                req.priority,
                req.weight,
                chrono::Utc::now().to_rfc3339(),
                id,
            ],
        )?;
        drop(conn);
        self.get_channel(id)
    }

    pub fn toggle_channel(&self, id: &str, enabled: bool) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "UPDATE channels SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
            params![enabled as i32, chrono::Utc::now().to_rfc3339(), id],
        )?;
        Ok(count > 0)
    }

    pub fn delete_channel(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM channels WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    pub fn update_channel_models(&self, id: &str, models: &[String]) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let models_json = serde_json::to_string(models).unwrap_or_default();
        conn.execute(
            "UPDATE channels SET models = ?1, updated_at = ?2 WHERE id = ?3",
            params![models_json, chrono::Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    // ============================================================
    // API Keys
    // ============================================================
    pub fn list_api_keys(&self) -> Result<Vec<ApiKey>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, key, usage_count, usage_limit, enabled, created_at FROM api_keys ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ApiKey {
                id: row.get(0)?,
                name: row.get(1)?,
                key: row.get(2)?,
                usage_count: row.get(3)?,
                usage_limit: row.get(4)?,
                enabled: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    }

    pub fn create_api_key(&self, req: &CreateApiKey) -> Result<ApiKey, rusqlite::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let key = format!("sk-{}", hex::encode(&rand::random::<[u8; 24]>()));
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO api_keys (id, name, key, usage_count, usage_limit, enabled, created_at) VALUES (?1, ?2, ?3, 0, ?4, 1, ?5)",
            params![id, req.name, key, req.usage_limit, now],
        )?;
        drop(conn);
        Ok(ApiKey {
            id,
            name: req.name.clone(),
            key,
            usage_count: 0,
            usage_limit: req.usage_limit,
            enabled: true,
            created_at: now,
        })
    }

    pub fn validate_api_key(&self, key: &str) -> Result<Option<ApiKey>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT id, name, key, usage_count, usage_limit, enabled, created_at FROM api_keys WHERE key = ?1 AND enabled = 1",
            params![key],
            |row| {
                Ok(ApiKey {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    key: row.get(2)?,
                    usage_count: row.get(3)?,
                    usage_limit: row.get(4)?,
                    enabled: row.get::<_, i32>(5)? != 0,
                    created_at: row.get(6)?,
                })
            },
        );
        match result {
            Ok(k) => {
                if k.usage_limit > 0 && k.usage_count >= k.usage_limit {
                    Ok(None)
                } else {
                    Ok(Some(k))
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn increment_key_usage(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE api_keys SET usage_count = usage_count + 1 WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn toggle_api_key(&self, id: &str, enabled: bool) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "UPDATE api_keys SET enabled = ?1 WHERE id = ?2",
            params![enabled as i32, id],
        )?;
        Ok(count > 0)
    }

    pub fn delete_api_key(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM api_keys WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    // ============================================================
    // Logs
    // ============================================================
    pub fn create_log(&self, log: &RequestLog) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO logs (id, channel_id, channel_name, model, api_key_id, request_type, status_code, latency_ms, prompt_tokens, completion_tokens, error, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![log.id, log.channel_id, log.channel_name, log.model, log.api_key_id, log.request_type, log.status_code, log.latency_ms, log.prompt_tokens, log.completion_tokens, log.error, log.created_at],
        )?;
        Ok(())
    }

    pub fn list_logs(&self, limit: i64, offset: i64, _channel_id: Option<&str>, _model: Option<&str>, _status: Option<i32>) -> Result<(Vec<RequestLog>, i64), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM logs", [], |r| r.get(0))?;
        let mut stmt = conn.prepare(
            "SELECT id, channel_id, channel_name, model, api_key_id, request_type, status_code, latency_ms, prompt_tokens, completion_tokens, error, created_at FROM logs ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
        )?;
        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok(RequestLog {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                channel_name: row.get(2)?,
                model: row.get(3)?,
                api_key_id: row.get(4)?,
                request_type: row.get(5)?,
                status_code: row.get(6)?,
                latency_ms: row.get(7)?,
                prompt_tokens: row.get(8)?,
                completion_tokens: row.get(9)?,
                error: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?;
        let logs: Vec<RequestLog> = rows.collect::<Result<Vec<_>, _>>()?;
        Ok((logs, total))
    }

    pub fn get_log_stats(&self) -> Result<LogStats, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM logs", [], |r| r.get(0))?;
        let success: i64 = conn.query_row("SELECT COUNT(*) FROM logs WHERE status_code >= 200 AND status_code < 300", [], |r| r.get(0))?;
        let errors: i64 = conn.query_row("SELECT COUNT(*) FROM logs WHERE status_code >= 400", [], |r| r.get(0))?;
        let today: i64 = conn.query_row(
            "SELECT COUNT(*) FROM logs WHERE created_at >= ?1",
            params![chrono::Utc::now().format("%Y-%m-%d").to_string()],
            |r| r.get(0),
        )?;
        Ok(LogStats { total, success, errors, today })
    }

    pub fn clear_logs(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM logs", [])?;
        Ok(())
    }

    // ============================================================
    // Settings
    // ============================================================
    pub fn get_settings(&self) -> Result<Settings, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT circuit_breaker_threshold, circuit_breaker_reset_time, retry_times, timeout, auto_select_new_models, max_tokens_per_month, default_model FROM settings WHERE id = 1",
            [],
            |row| {
                Ok(Settings {
                    circuit_breaker_threshold: row.get(0)?,
                    circuit_breaker_reset_time: row.get(1)?,
                    retry_times: row.get(2)?,
                    timeout: row.get(3)?,
                    auto_select_new_models: row.get::<_, i32>(4)? != 0,
                    max_tokens_per_month: row.get(5)?,
                    default_model: row.get(6)?,
                })
            },
        );
        match result {
            Ok(s) => Ok(s),
            Err(_) => Ok(Settings::default()),
        }
    }

    pub fn update_settings(&self, s: &Settings) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE settings SET circuit_breaker_threshold = ?1, circuit_breaker_reset_time = ?2, retry_times = ?3, timeout = ?4, auto_select_new_models = ?5, max_tokens_per_month = ?6, default_model = ?7 WHERE id = 1",
            params![s.circuit_breaker_threshold, s.circuit_breaker_reset_time, s.retry_times, s.timeout, s.auto_select_new_models as i32, s.max_tokens_per_month, s.default_model],
        )?;
        Ok(())
    }
}
