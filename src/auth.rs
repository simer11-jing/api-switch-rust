use sha2::{Sha256, Digest};

pub fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(b"api-switch-salt-v1");
    hex::encode(hasher.finalize())
}

pub fn generate_token() -> String {
    let mut hasher = Sha256::new();
    hasher.update(&rand::random::<[u8; 32]>());
    hasher.update(&chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0).to_be_bytes());
    hex::encode(hasher.finalize())
}
