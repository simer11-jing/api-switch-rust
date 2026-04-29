use axum::Router;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

mod db;
mod auth;
mod models;
mod handlers;
mod circuit_breaker;

pub type SharedState = Arc<RwLock<AppState>>;

pub struct AppState {
    pub db: db::Database,
    pub redis: Option<redis::Client>,
    pub breaker: circuit_breaker::CircuitBreakerManager,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "./data/api-switch.db".into());
    let redis_url = std::env::var("REDIS_URL").ok();
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "9091".into())
        .parse()
        .unwrap_or(9091);

    let database = db::Database::new(&db_path).expect("Failed to init database");

    let redis_client = if let Some(url) = redis_url {
        match redis::Client::open(url.as_str()) {
            Ok(client) => {
                match client.get_multiplexed_async_connection().await {
                    Ok(_) => {
                        tracing::info!("✅ Redis connected");
                        Some(client)
                    }
                    Err(e) => {
                        tracing::warn!("⚠️ Redis unavailable: {}", e);
                        None
                    }
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    let state: SharedState = Arc::new(RwLock::new(AppState {
        db: database,
        redis: redis_client,
        breaker: circuit_breaker::CircuitBreakerManager::new(),
    }));

    let app = Router::new()
        .merge(handlers::auth_routes())
        .merge(handlers::channel_routes())
        .merge(handlers::channel_action_routes())
        .merge(handlers::key_routes())
        .merge(handlers::entry_routes())
        .merge(handlers::log_routes())
        .merge(handlers::settings_routes())
        .merge(handlers::dashboard_routes())
        .merge(handlers::chat_test_routes())
        .merge(handlers::proxy_routes())
        .route("/health", axum::routing::get(handlers::health))
        .fallback_service(ServeDir::new("static"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    tracing::info!("🚀 API Switch running on port {}", port);
    axum::serve(listener, app).await.unwrap();
}
