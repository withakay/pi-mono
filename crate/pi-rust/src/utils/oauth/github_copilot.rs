use anyhow::{anyhow, Result};
use serde::Deserialize;

const CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const TOKEN_EXPIRY_BUFFER_MS: i64 = 5 * 60 * 1000;

const COPILOT_HEADERS: &[(&str, &str)] = &[
    ("User-Agent", "GitHubCopilotChat/0.35.0"),
    ("Editor-Version", "vscode/1.107.0"),
    ("Editor-Plugin-Version", "copilot-chat/0.35.0"),
    ("Copilot-Integration-Id", "vscode-chat"),
];

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: u64,
    expires_in: u64,
}

#[derive(Deserialize)]
struct CopilotTokenResponse {
    token: String,
    expires_at: u64,
}

/// Login to GitHub Copilot using Device Code OAuth flow.
/// Returns (access_token, refresh_token, expires_ms)
/// - access_token: Copilot API token (short-lived)
/// - refresh_token: GitHub access token (long-lived, used to refresh)
/// - expires_ms: expiry time in milliseconds
pub async fn login_github_copilot() -> Result<(String, String, i64)> {
    let client = reqwest::Client::new();

    let device_resp: DeviceCodeResponse = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("User-Agent", "GitHubCopilotChat/0.35.0")
        .json(&serde_json::json!({
            "client_id": CLIENT_ID,
            "scope": "read:user"
        }))
        .send()
        .await?
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse device code response: {}", e))?;

    println!("\n  Open this URL in your browser:");
    println!("  {}", device_resp.verification_uri);
    println!("\n  Enter this code: {}", device_resp.user_code);
    println!("\n  Waiting for authorization...");

    let github_token = poll_for_github_token(
        &client,
        &device_resp.device_code,
        device_resp.interval,
        device_resp.expires_in,
    )
    .await?;

    println!("  GitHub authorization successful! Getting Copilot token...");

    let copilot_resp: CopilotTokenResponse = client
        .get("https://api.github.com/copilot_internal/v2/token")
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {}", github_token))
        .headers({
            let mut headers = reqwest::header::HeaderMap::new();
            for (k, v) in COPILOT_HEADERS {
                headers.insert(
                    reqwest::header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                    reqwest::header::HeaderValue::from_str(v).unwrap(),
                );
            }
            headers
        })
        .send()
        .await?
        .json()
        .await
        .map_err(|e| anyhow!("Failed to get Copilot token: {}", e))?;

    // expires_at is in Unix seconds; convert to milliseconds and apply buffer.
    // Matches TypeScript: `expiresAt * 1000 - 5 * 60 * 1000`
    let expires_ms = (copilot_resp.expires_at as i64 * 1000) - TOKEN_EXPIRY_BUFFER_MS;

    Ok((copilot_resp.token, github_token, expires_ms))
}

async fn poll_for_github_token(
    client: &reqwest::Client,
    device_code: &str,
    interval_secs: u64,
    expires_in: u64,
) -> Result<String> {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(expires_in);
    let mut interval = std::time::Duration::from_secs(interval_secs.max(1));

    loop {
        if std::time::Instant::now() >= deadline {
            return Err(anyhow!("Device authorization timed out"));
        }

        tokio::time::sleep(interval).await;

        let resp = client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("User-Agent", "GitHubCopilotChat/0.35.0")
            .json(&serde_json::json!({
                "client_id": CLIENT_ID,
                "device_code": device_code,
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code"
            }))
            .send()
            .await?;

        let body: serde_json::Value = resp.json().await?;

        if let Some(token) = body["access_token"].as_str() {
            return Ok(token.to_string());
        }

        match body["error"].as_str() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                interval += std::time::Duration::from_secs(5);
                continue;
            }
            Some(err) => return Err(anyhow!("GitHub OAuth error: {}", err)),
            None => continue,
        }
    }
}
