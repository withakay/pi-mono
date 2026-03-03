// Auth storage - shared with TypeScript pi, reads ~/.pi/agent/auth.json

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StoredCredential {
    ApiKey { key: String },
    #[serde(rename = "oauth")]
    OAuth {
        access: String,
        refresh: String,
        expires: i64,
        #[serde(flatten)]
        extra: HashMap<String, serde_json::Value>,
    },
}

pub type AuthData = HashMap<String, StoredCredential>;

/// Get the path to the shared auth.json file (~/.pi/agent/auth.json)
pub fn auth_file_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pi")
        .join("agent")
        .join("auth.json")
}

/// Load credentials from auth.json
pub fn load_auth() -> AuthData {
    let path = auth_file_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Save credentials to auth.json (creates parent dirs, sets 0o600 permissions on Unix)
pub fn save_auth(data: &AuthData) -> Result<()> {
    let path = auth_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(data)?;
    std::fs::write(&path, &json)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Get an API key or token for a provider from auth.json
pub fn get_api_key(provider: &str) -> Option<String> {
    let auth = load_auth();
    match auth.get(provider)? {
        StoredCredential::ApiKey { key } => Some(key.clone()),
        StoredCredential::OAuth { access, expires, .. } => {
            let now_ms = chrono::Utc::now().timestamp_millis();
            if *expires > now_ms {
                Some(access.clone())
            } else {
                None // Expired
            }
        }
    }
}

/// Store an API key credential
pub fn store_api_key(provider: &str, key: String) -> Result<()> {
    let mut data = load_auth();
    data.insert(provider.to_string(), StoredCredential::ApiKey { key });
    save_auth(&data)
}

/// Store OAuth credentials
pub fn store_oauth(provider: &str, access: String, refresh: String, expires_ms: i64) -> Result<()> {
    let mut data = load_auth();
    data.insert(
        provider.to_string(),
        StoredCredential::OAuth {
            access,
            refresh,
            expires: expires_ms,
            extra: HashMap::new(),
        },
    );
    save_auth(&data)
}

/// Remove credentials for a provider
pub fn remove_credential(provider: &str) -> Result<()> {
    let mut data = load_auth();
    data.remove(provider);
    save_auth(&data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_store_and_load_api_key() {
        let temp = TempDir::new().unwrap();
        let test_path = temp.path().join("auth.json");

        let mut data = AuthData::new();
        data.insert(
            "openrouter".to_string(),
            StoredCredential::ApiKey {
                key: "sk-or-test".to_string(),
            },
        );

        let json = serde_json::to_string_pretty(&data).unwrap();
        std::fs::write(&test_path, &json).unwrap();

        let loaded: AuthData = serde_json::from_str(&json).unwrap();
        match loaded.get("openrouter").unwrap() {
            StoredCredential::ApiKey { key } => assert_eq!(key, "sk-or-test"),
            _ => panic!("Expected ApiKey"),
        }
    }

    #[test]
    fn test_oauth_expiry_check() {
        let now_ms = chrono::Utc::now().timestamp_millis();

        let valid = StoredCredential::OAuth {
            access: "token123".to_string(),
            refresh: "refresh123".to_string(),
            expires: now_ms + 3_600_000,
            extra: HashMap::new(),
        };

        let expired = StoredCredential::OAuth {
            access: "old_token".to_string(),
            refresh: "refresh123".to_string(),
            expires: now_ms - 1000,
            extra: HashMap::new(),
        };

        match &valid {
            StoredCredential::OAuth { expires, access, .. } => {
                assert!(*expires > now_ms, "Valid token should not be expired");
                assert_eq!(access, "token123");
            }
            _ => panic!(),
        }

        match &expired {
            StoredCredential::OAuth { expires, .. } => {
                assert!(*expires < now_ms, "Expired token should have past expiry");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_serialization_roundtrip() {
        let json = r#"{"type":"api_key","key":"sk-test"}"#;
        let cred: StoredCredential = serde_json::from_str(json).unwrap();
        match cred {
            StoredCredential::ApiKey { key } => assert_eq!(key, "sk-test"),
            _ => panic!("Expected ApiKey"),
        }

        let oauth_json = r#"{"type":"oauth","access":"acc","refresh":"ref","expires":9999999999999}"#;
        let oauth_cred: StoredCredential = serde_json::from_str(oauth_json).unwrap();
        match oauth_cred {
            StoredCredential::OAuth {
                access,
                refresh,
                expires,
                ..
            } => {
                assert_eq!(access, "acc");
                assert_eq!(refresh, "ref");
                assert_eq!(expires, 9999999999999i64);
            }
            _ => panic!("Expected OAuth"),
        }
    }
}
