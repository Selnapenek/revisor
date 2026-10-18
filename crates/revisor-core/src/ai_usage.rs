//! Read subscription usage from the installed Claude Code and Codex accounts.
//! Credentials stay in this process and are only sent to their provider.

use crate::agent::{self, Backend};
use serde_json::Value;
use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, PartialEq)]
pub struct WindowUsage {
    pub used_percent: f64,
    pub resets_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Usage {
    pub backend: Backend,
    pub plan: Option<String>,
    pub session: Option<WindowUsage>,
    pub weekly: Option<WindowUsage>,
    pub error: Option<String>,
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| value.as_str()?.parse().ok())
}

fn timestamp(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value.as_str().and_then(|s| {
            s.parse().ok().or_else(|| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|d| d.timestamp() as u64)
            })
        })
    })
}

fn claude_window(value: &Value) -> Option<WindowUsage> {
    Some(WindowUsage {
        used_percent: number(&value["utilization"])?,
        resets_at: timestamp(&value["resets_at"]),
    })
}

fn codex_window(value: &Value, now: u64) -> Option<(WindowUsage, Option<u64>)> {
    let used_percent = number(&value["used_percent"])?;
    let resets_at = timestamp(&value["reset_at"])
        .or_else(|| timestamp(&value["reset_after_seconds"]).map(|s| now.saturating_add(s)));
    Some((
        WindowUsage {
            used_percent,
            resets_at,
        },
        timestamp(&value["limit_window_seconds"]),
    ))
}

fn parse_claude(body: &Value, credentials: &Value) -> Usage {
    let oauth = &credentials["claudeAiOauth"];
    let plan = oauth["subscriptionType"].as_str().map(|s| {
        let name = s
            .split(['-', '_'])
            .map(|part| {
                let mut chars = part.chars();
                chars
                    .next()
                    .map(|c| c.to_uppercase().to_string() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ");
        let multiplier = oauth["rateLimitTier"]
            .as_str()
            .and_then(|tier| tier.rsplit('_').next())
            .filter(|last| {
                last.ends_with('x') && last[..last.len() - 1].chars().all(|c| c.is_ascii_digit())
            });
        multiplier.map_or(name.clone(), |m| format!("{name} {m}"))
    });
    Usage {
        backend: Backend::Claude,
        plan,
        session: claude_window(&body["five_hour"]),
        weekly: claude_window(&body["seven_day"]),
        error: None,
    }
}

fn parse_codex(body: &Value, now: u64) -> Usage {
    let limits = &body["rate_limit"];
    let primary = codex_window(&limits["primary_window"], now);
    let secondary = codex_window(&limits["secondary_window"], now);
    let long = |window: &Option<(WindowUsage, Option<u64>)>| {
        window
            .as_ref()
            .and_then(|(_, length)| *length)
            .is_some_and(|s| s >= 86_400)
    };
    let (session, weekly) = if long(&primary) && !long(&secondary) {
        (secondary, primary)
    } else {
        (primary, secondary)
    };
    Usage {
        backend: Backend::Codex,
        plan: body["plan_type"].as_str().map(|s| {
            let mut chars = s.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        }),
        session: session.map(|(window, _)| window),
        weekly: weekly.map(|(window, _)| window),
        error: None,
    }
}

fn home() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "Home directory unavailable".into())
}

#[cfg(target_os = "macos")]
fn parse_keychain_value(raw: &str) -> Option<Value> {
    if let Ok(value) = serde_json::from_str(raw.trim()) {
        return Some(value);
    }
    let hex = raw.trim().trim_start_matches("0x").trim_start_matches("0X");
    if hex.len() % 2 != 0 {
        return None;
    }
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect::<Option<Vec<_>>>()?;
    serde_json::from_slice(&bytes).ok()
}

#[cfg(target_os = "macos")]
fn keychain(service: &str, account: Option<&str>) -> Option<Value> {
    let mut command = std::process::Command::new("security");
    command.args(["find-generic-password", "-s", service]);
    if let Some(account) = account {
        command.args(["-a", account]);
    }
    let output = command.arg("-w").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    parse_keychain_value(&raw)
}

fn credentials(backend: Backend) -> Result<Value, String> {
    #[cfg(target_os = "macos")]
    if backend == Backend::Claude {
        // Claude Code keeps its current token in Keychain on macOS.
        if let Some(value) = keychain("Claude Code-credentials", None) {
            if value["claudeAiOauth"]["accessToken"].as_str().is_some() {
                return Ok(value);
            }
        }
    }
    let home = home()?;
    let paths = match backend {
        Backend::Claude => vec![home.join(".claude/.credentials.json")],
        Backend::Codex => {
            let mut paths = Vec::new();
            if let Some(dir) = std::env::var_os("CODEX_HOME") {
                paths.push(PathBuf::from(dir).join("auth.json"));
            }
            paths.extend([
                home.join(".config/codex/auth.json"),
                home.join(".codex/auth.json"),
            ]);
            paths
        }
    };
    if let Some(value) = paths.into_iter().find_map(|p| {
        std::fs::read_to_string(p)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
    }) {
        let token = match backend {
            Backend::Claude => value["claudeAiOauth"]["accessToken"].as_str(),
            Backend::Codex => value["tokens"]["access_token"].as_str(),
        };
        if token.is_some() {
            return Ok(value);
        }
    }
    #[cfg(target_os = "macos")]
    if backend == Backend::Codex {
        if let Some(value) = keychain("Codex Auth", Some("codex")) {
            return Ok(value);
        }
    }
    Err(format!("Sign in with {} to see usage", backend.label()))
}

async fn fetch(backend: Backend, client: &reqwest::Client) -> Result<Usage, String> {
    let credentials = tokio::task::spawn_blocking(move || credentials(backend))
        .await
        .map_err(|_| "Could not read credentials".to_string())??;
    let (url, token) = match backend {
        Backend::Claude => (
            "https://api.anthropic.com/api/oauth/usage",
            credentials["claudeAiOauth"]["accessToken"].as_str(),
        ),
        Backend::Codex => (
            "https://chatgpt.com/backend-api/wham/usage",
            credentials["tokens"]["access_token"].as_str(),
        ),
    };
    let token = token.ok_or_else(|| format!("Sign in with {} to see usage", backend.label()))?;
    let mut request = client.get(url).bearer_auth(token);
    match backend {
        Backend::Claude => {
            request = request
                .header("anthropic-beta", "oauth-2025-04-20")
                .header("User-Agent", "claude-code/2.1.69")
        }
        Backend::Codex => {
            if let Some(account) = credentials["tokens"]["account_id"].as_str() {
                request = request.header("ChatGPT-Account-Id", account);
            }
        }
    }
    let response = request
        .send()
        .await
        .map_err(|_| "Could not load usage".to_string())?;
    if !response.status().is_success() {
        return Err(if response.status().as_u16() == 401 {
            format!("Sign in again with {} to see usage", backend.label())
        } else {
            format!("Usage request failed ({})", response.status())
        });
    }
    let body: Value = response
        .json()
        .await
        .map_err(|_| "Could not read usage data".to_string())?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Ok(match backend {
        Backend::Claude => parse_claude(&body, &credentials),
        Backend::Codex => parse_codex(&body, now),
    })
}

/// Fetch each installed provider independently; a failure keeps its row visible.
pub async fn installed_usage() -> Vec<Usage> {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(client) => client,
        Err(_) => return Vec::new(),
    };
    let mut entries = Vec::new();
    for backend in agent::installed() {
        entries.push(fetch(backend, &client).await.unwrap_or_else(|error| Usage {
            backend,
            plan: None,
            session: None,
            weekly: None,
            error: Some(error),
        }));
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn claude_windows_and_plan() {
        let usage = parse_claude(
            &json!({"five_hour":{"utilization":43.4,"resets_at":"2026-10-18T12:00:00Z"},"seven_day":{"utilization":15}}),
            &json!({"claudeAiOauth":{"subscriptionType":"pro","rateLimitTier":"default_claude_pro_5x"}}),
        );
        assert_eq!(usage.plan.as_deref(), Some("Pro 5x"));
        assert_eq!(usage.session.unwrap().used_percent, 43.4);
        assert_eq!(usage.weekly.unwrap().used_percent, 15.0);
    }

    #[test]
    fn codex_weekly_only_window_is_not_called_session() {
        let usage = parse_codex(
            &json!({"plan_type":"plus","rate_limit":{"primary_window":{"used_percent":15,"reset_after_seconds":3600,"limit_window_seconds":604800}}}),
            100,
        );
        assert_eq!(usage.plan.as_deref(), Some("Plus"));
        assert_eq!(usage.session, None);
        assert_eq!(usage.weekly.unwrap().resets_at, Some(3700));
    }
}
