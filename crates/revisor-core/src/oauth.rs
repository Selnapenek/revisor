use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

const DEVICE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const SCOPE: &str = "repo";

fn is_github_device_uri(uri: &str) -> bool {
    uri == "https://github.com/login/device"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollResult {
    Pending(u64),
    Failed(String),
}

impl PollResult {
    fn from_error(error: &str, interval: u64) -> Self {
        match error {
            "authorization_pending" => Self::Pending(interval),
            "slow_down" => Self::Pending(interval + 5),
            other => Self::Failed(other.to_owned()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeviceLogin {
    pub attempt_id: String,
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_at: Instant,
}

#[derive(Serialize)]
struct DeviceRequest<'a> {
    client_id: &'a str,
    scope: &'a str,
}

#[derive(Deserialize)]
struct DeviceResponse {
    device_code: Option<String>,
    user_code: Option<String>,
    verification_uri: Option<String>,
    interval: Option<u64>,
    expires_in: Option<u64>,
    error: Option<String>,
}

#[derive(Serialize)]
struct TokenRequest<'a> {
    client_id: &'a str,
    device_code: &'a str,
    grant_type: &'a str,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
}

fn client() -> Result<Client> {
    Client::builder()
        .user_agent("Revisor/1.0.0")
        .timeout(Duration::from_secs(15))
        .build()
        .context("Failed to create OAuth HTTP client")
}

pub async fn begin(client_id: &str, attempt_id: String) -> Result<DeviceLogin> {
    let response: DeviceResponse = client()?
        .post(DEVICE_URL)
        .header("Accept", "application/json")
        .json(&DeviceRequest {
            client_id,
            scope: SCOPE,
        })
        .send()
        .await
        .context("Could not connect to GitHub")?
        .error_for_status()
        .context("GitHub rejected the device login request")?
        .json()
        .await
        .context("GitHub returned an invalid device login response")?;
    if let Some(error) = response.error {
        bail!(friendly_device_error(&error));
    }
    let device_code = response
        .device_code
        .context("GitHub did not return a device code")?;
    let user_code = response
        .user_code
        .context("GitHub did not return a user code")?;
    let verification_uri = response
        .verification_uri
        .context("GitHub did not return a verification address")?;
    let expires_in = response
        .expires_in
        .context("GitHub did not return the code expiry")?;
    if !is_github_device_uri(&verification_uri) {
        bail!("GitHub returned an unexpected device verification address");
    }
    Ok(DeviceLogin {
        attempt_id,
        device_code,
        user_code,
        verification_uri,
        interval: response.interval.unwrap_or(5).max(1),
        expires_at: Instant::now() + Duration::from_secs(expires_in),
    })
}

fn friendly_device_error(error: &str) -> &'static str {
    match error {
        "device_flow_disabled" => "Device flow is not enabled for this GitHub OAuth app.",
        "incorrect_client_credentials" => "The GitHub OAuth client ID is invalid.",
        _ => "GitHub could not start sign-in. Check the app configuration and try again.",
    }
}

pub async fn poll(
    client_id: &str,
    login: &DeviceLogin,
    cancelled: Arc<AtomicBool>,
) -> Result<String> {
    let mut interval = login.interval;
    loop {
        if cancelled.load(Ordering::SeqCst) {
            bail!("GitHub sign-in was cancelled.");
        }
        if Instant::now() >= login.expires_at {
            bail!("GitHub sign-in code expired. Start again.");
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(interval)) => {},
            _ = async { while !cancelled.load(Ordering::SeqCst) { tokio::time::sleep(Duration::from_millis(100)).await; } } => bail!("GitHub sign-in was cancelled."),
        }
        if Instant::now() >= login.expires_at {
            bail!("GitHub sign-in code expired. Start again.");
        }
        let response = client()?
            .post(TOKEN_URL)
            .header("Accept", "application/json")
            .json(&TokenRequest {
                client_id,
                device_code: &login.device_code,
                grant_type: "urn:ietf:params:oauth:grant-type:device_code",
            })
            .send()
            .await
            .context("Could not connect to GitHub while checking sign-in")?;
        let result: TokenResponse = response
            .json()
            .await
            .context("GitHub returned an invalid token response")?;
        if let Some(token) = result.access_token {
            if result.expires_in.is_some() || result.refresh_token.is_some() {
                bail!("This GitHub OAuth app returned expiring credentials, which Revisor does not yet support. Disable token expiration for the app.");
            }
            return Ok(token);
        }
        match PollResult::from_error(result.error.as_deref().unwrap_or("unknown_error"), interval) {
            PollResult::Pending(next_interval) => {
                interval = next_interval;
                continue;
            }
            PollResult::Failed(error) => bail!(match error.as_str() {
                "access_denied" => "GitHub sign-in was denied.",
                "expired_token" => "GitHub sign-in code expired. Start again.",
                "device_flow_disabled" => "Device flow is not enabled for this GitHub OAuth app.",
                "incorrect_client_credentials" => "The GitHub OAuth client ID is invalid.",
                _ => "GitHub could not complete sign-in. Start again.",
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PollResult;

    #[test]
    fn authorization_pending_keeps_polling_interval() {
        assert_eq!(
            PollResult::from_error("authorization_pending", 5),
            PollResult::Pending(5)
        );
    }

    #[test]
    fn slow_down_adds_five_seconds() {
        assert_eq!(
            PollResult::from_error("slow_down", 5),
            PollResult::Pending(10)
        );
    }

    #[test]
    fn terminal_errors_stop_polling() {
        assert_eq!(
            PollResult::from_error("access_denied", 5),
            PollResult::Failed("access_denied".into())
        );
        assert_eq!(
            PollResult::from_error("expired_token", 5),
            PollResult::Failed("expired_token".into())
        );
    }

    #[test]
    fn verification_uri_is_fixed_to_github_device_flow() {
        assert!(super::is_github_device_uri(
            "https://github.com/login/device"
        ));
        assert!(!super::is_github_device_uri(
            "https://attacker.example/login/device"
        ));
        assert!(!super::is_github_device_uri(
            "https://github.com.evil.test/login/device"
        ));
    }

    #[test]
    fn device_flow_disabled_has_a_clear_message() {
        assert_eq!(
            super::friendly_device_error("device_flow_disabled"),
            "Device flow is not enabled for this GitHub OAuth app."
        );
    }
}
