//! One-shot Claude Code and Codex CLI calls with structured JSON output.
//! The agent gets everything in the prompt and has no tools, so it cannot read or change files.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Claude,
    Codex,
}

impl Backend {
    pub fn executable(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        [Self::Claude, Self::Codex].into_iter().find(|b| b.executable() == name)
    }
}

/// GUI apps do not get the login shell's PATH, so also look where the installers put the CLIs.
fn executable_path(backend: Backend) -> Option<PathBuf> {
    let name = backend.executable();
    let home = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/bin"));
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .chain(home)
        .chain([PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin")])
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

pub fn installed() -> Vec<Backend> {
    [Backend::Claude, Backend::Codex].into_iter().filter(|b| executable_path(*b).is_some()).collect()
}

/// The CLI selected from PATH or a standard installer directory.
pub fn cli_info(backend: Backend) -> Option<(PathBuf, String)> {
    let path = executable_path(backend)?;
    let version = std::process::Command::new(&path)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "Installed".into());
    Some((path, version))
}

/// `None` means the installed CLI did not give a usable answer.
pub fn auth_status(backend: Backend) -> Option<bool> {
    let path = executable_path(backend)?;
    let args: &[&str] = match backend {
        Backend::Claude => &["auth", "status", "--json"],
        Backend::Codex => &["login", "status"],
    };
    let output = std::process::Command::new(path).args(args).output().ok()?;
    parse_auth_status(backend, output.status.success(), &output.stdout)
}

fn parse_auth_status(backend: Backend, success: bool, stdout: &[u8]) -> Option<bool> {
    match backend {
        Backend::Claude => serde_json::from_slice::<Value>(stdout).ok()?["loggedIn"].as_bool(),
        // Codex returns a failing status when there is no saved login.
        Backend::Codex => Some(success),
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Script opened by macOS Terminal for an interactive CLI sign-in.
pub fn login_script(path: &Path, backend: Backend) -> String {
    let args = match backend {
        Backend::Claude => "auth login",
        Backend::Codex => "login --device-auth",
    };
    format!("#!/bin/zsh\nrm -f -- \"$0\"\n{} {args}\nstatus=$?\nprintf '\\nReturn to Revisor and select Check status.\\n'\nif [ \"$status\" -ne 0 ]; then printf 'Login exited with status %s.\\n' \"$status\"; fi\nprintf 'Press Return to close this window. '\nread -r reply\n", shell_quote(&path.to_string_lossy()))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Effort {
    pub id: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub id: String,
    pub label: String,
    pub efforts: Vec<Effort>,
    pub default_effort: String,
}

// ponytail: fixed list, Claude Code has no command that lists models; fetch a catalog if it goes stale too often.
const CLAUDE_MODELS: [(&str, &str); 8] = [
    ("claude-opus-5-5", "Opus 5.5"),
    ("claude-sonnet-5-5", "Sonnet 5.5"),
    ("claude-haiku-5-5", "Haiku 5.5"),
    ("claude-fable-5-1", "Fable 5.1"),
    ("claude-opus-5", "Opus 5"),
    ("claude-sonnet-5", "Sonnet 5"),
    ("claude-fable-5", "Fable 5"),
    ("claude-opus-4-8", "Opus 4.8"),
];
const CLAUDE_EFFORTS: [(&str, &str); 5] =
    [("low", "Light"), ("medium", "Moderate"), ("high", "Deep"), ("xhigh", "Extra deep"), ("max", "No limits")];

fn effort_label(id: &str) -> String {
    match id {
        "xhigh" => "Extra high".into(),
        _ => id[..1].to_uppercase() + &id[1..],
    }
}

/// `model` and `effort` when `models` offers them; else the first model and its default effort,
/// so the pickers always show what will run.
pub fn pick(models: &[Model], model: &str, effort: &str) -> (String, String) {
    let Some(model) = models.iter().find(|m| m.id == model).or(models.first()) else { return (String::new(), effort.into()) };
    let effort = if model.efforts.iter().any(|e| e.id == effort) { effort.into() } else { model.default_effort.clone() };
    (model.id.clone(), effort)
}

/// Models the CLI offers, default first. Blocking for Codex: reads its model catalog.
pub fn models(backend: Backend) -> Vec<Model> {
    match backend {
        Backend::Claude => CLAUDE_MODELS
            .map(|(id, label)| Model {
                id: id.into(),
                label: label.into(),
                efforts: CLAUDE_EFFORTS
                    .map(|(id, description)| Effort { id: id.into(), label: effort_label(id), description: description.into() })
                    .to_vec(),
                // Claude Code's own default.
                default_effort: "high".into(),
            })
            .to_vec(),
        Backend::Codex => executable_path(backend)
            .and_then(|exe| std::process::Command::new(exe).args(["debug", "models"]).output().ok())
            .map(|out| codex_models(&out.stdout))
            .unwrap_or_default(),
    }
}

fn codex_models(json: &[u8]) -> Vec<Model> {
    let catalog: Value = serde_json::from_slice(json).unwrap_or_default();
    catalog["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["visibility"] == "list")
        .filter_map(|m| {
            let efforts: Vec<Effort> = m["supported_reasoning_levels"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|e| {
                    let id = e["effort"].as_str()?;
                    let description = e["description"].as_str().unwrap_or_default().into();
                    Some(Effort { id: id.into(), label: effort_label(id), description })
                })
                // "ultra" delegates to sub-agents; a review with no tools cannot use it.
                .filter(|e| e.id != "ultra")
                .collect();
            let default_effort = m["default_reasoning_level"].as_str().map(String::from);
            Some(Model {
                id: m["slug"].as_str()?.into(),
                label: m["display_name"].as_str().or(m["slug"].as_str())?.into(),
                default_effort: default_effort.or_else(|| efforts.first().map(|e| e.id.clone()))?,
                efforts,
            })
        })
        .collect()
}

/// No tools, and no user settings, hooks, MCP servers, skills or CLAUDE.md: they cost tokens and time.
fn claude_args(system: &str, schema: &str, model: Option<&str>, effort: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = [
        "-p", "--output-format", "json", "--no-session-persistence", "--tools", "",
        "--setting-sources", "", "--strict-mcp-config", "--disable-slash-commands",
        "--system-prompt", system, "--json-schema", schema,
    ]
    .map(str::to_string)
    .to_vec();
    args.extend(model.into_iter().flat_map(|m| ["--model".into(), m.into()]));
    args.extend(effort.into_iter().flat_map(|e| ["--effort".into(), e.into()]));
    args
}

fn codex_args(schema: &Path, output: &Path, dir: &Path, model: Option<&str>, effort: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = [
        "exec", "--skip-git-repo-check", "--ephemeral", "--ignore-user-config", "--ignore-rules",
        "--sandbox", "read-only", "--output-schema",
    ]
    .map(str::to_string)
    .to_vec();
    args.push(schema.display().to_string());
    args.extend(model.into_iter().flat_map(|m| ["-m".into(), m.into()]));
    args.extend(effort.into_iter().flat_map(|e| ["-c".into(), format!("model_reasoning_effort={e:?}")]));
    args.extend(["-o".into(), output.display().to_string(), "-C".into(), dir.display().to_string(), "-".into()]);
    args
}

#[derive(Debug)]
pub struct AgentOutput {
    pub value: Value,
    pub cost_usd: Option<f64>,
    pub duration: Duration,
}

fn parse_claude(stdout: &[u8]) -> Result<(Value, Option<f64>), String> {
    let result: Value = serde_json::from_slice(stdout).map_err(|e| format!("Claude output: {e}"))?;
    if result.get("is_error").and_then(Value::as_bool) == Some(true) {
        return Err(result.get("result").and_then(Value::as_str).unwrap_or("Claude returned an error").to_string());
    }
    let value = result.get("structured_output").filter(|v| v.is_object()).cloned().ok_or("Claude returned no structured output")?;
    Ok((value, result.get("total_cost_usd").and_then(Value::as_f64)))
}

/// Run one prompt. `None` model or effort uses the CLI default. Dropping the future kills the CLI.
pub async fn run(
    backend: Backend, model: Option<&str>, effort: Option<&str>, system: &str, prompt: &str, schema: &Value, timeout: Duration,
) -> Result<AgentOutput, String> {
    let executable = executable_path(backend).ok_or_else(|| format!("{} CLI not found", backend.label()))?;
    // An empty working directory: Codex can run shell commands, so give it nothing to read.
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let schema_text = serde_json::to_string(schema).map_err(|e| e.to_string())?;
    let (schema_path, output_path) = (tmp.path().join("schema.json"), tmp.path().join("output.json"));
    let (args, input) = match backend {
        Backend::Claude => (claude_args(system, &schema_text, model, effort), prompt.to_string()),
        Backend::Codex => {
            std::fs::write(&schema_path, &schema_text).map_err(|e| e.to_string())?;
            (codex_args(&schema_path, &output_path, tmp.path(), model, effort), format!("{system}\n\n{prompt}"))
        }
    };
    let start = Instant::now();
    let mut child = Command::new(executable)
        .args(args)
        .current_dir(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("No stdin")?;
    stdin.write_all(input.as_bytes()).await.map_err(|e| e.to_string())?;
    drop(stdin);
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| format!("{} did not finish in {} minutes", backend.label(), timeout.as_secs() / 60))?
        .map_err(|e| e.to_string())?;
    let (value, cost_usd) = match backend {
        // Claude reports most errors, such as "Not logged in", as a JSON result on stdout.
        Backend::Claude => parse_claude(&output.stdout).map_err(|e| failure(backend, &output, e))?,
        Backend::Codex => {
            let text = std::fs::read(&output_path).map_err(|e| failure(backend, &output, e.to_string()))?;
            (serde_json::from_slice(&text).map_err(|e| format!("Codex output: {e}"))?, None)
        }
    };
    Ok(AgentOutput { value, cost_usd, duration: start.elapsed() })
}

fn failure(backend: Backend, output: &std::process::Output, error: String) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    match stderr.trim().lines().last() {
        Some(line) if !output.status.success() => format!("{} failed: {line}", backend.label()),
        _ => error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_has_no_tools_and_no_user_settings() {
        let args = claude_args("system", "{}", Some("opus"), Some("high"));
        assert!(args.windows(2).any(|w| w == ["--tools", ""]));
        assert!(args.windows(2).any(|w| w == ["--setting-sources", ""]));
        assert!(args.windows(2).any(|w| w == ["--model", "opus"]));
        assert!(args.windows(2).any(|w| w == ["--effort", "high"]));
        assert!(!claude_args("s", "{}", None, None).contains(&"--model".to_string()));
        let codex = codex_args(Path::new("s"), Path::new("o"), Path::new("d"), Some("gpt-x"), Some("low"));
        assert!(codex.windows(2).any(|w| w == ["--sandbox", "read-only"]));
        assert!(codex.windows(2).any(|w| w == ["-m", "gpt-x"]));
        assert!(codex.windows(2).any(|w| w == ["-c", "model_reasoning_effort=\"low\""]));
        assert_eq!(codex.last().unwrap(), "-");
    }

    #[test]
    fn codex_catalog_lists_visible_models() {
        let json = br#"{"models":[{"slug":"a","display_name":"A","visibility":"list","default_reasoning_level":"high",
            "supported_reasoning_levels":[{"effort":"low","description":"Fast"},{"effort":"xhigh"},{"effort":"ultra"}]},
            {"slug":"hidden","visibility":"hide"}]}"#;
        let models = codex_models(json);
        assert_eq!(models.len(), 1);
        assert_eq!((models[0].id.as_str(), models[0].default_effort.as_str()), ("a", "high"));
        let efforts: Vec<_> = models[0].efforts.iter().map(|e| (e.label.as_str(), e.description.as_str())).collect();
        assert_eq!(efforts, [("Low", "Fast"), ("Extra high", "")]);
        assert!(codex_models(b"not json").is_empty());
    }

    #[test]
    fn pick_keeps_offered_choice_or_falls_back_to_defaults() {
        let models = models(Backend::Claude);
        assert_eq!(pick(&models, "claude-sonnet-5-5", "low"), ("claude-sonnet-5-5".into(), "low".into()));
        assert_eq!(pick(&models, "gone", "low"), ("claude-opus-5-5".into(), "low".into()));
        assert_eq!(pick(&models, "claude-sonnet-5-5", "ultra"), ("claude-sonnet-5-5".into(), "high".into()));
        assert_eq!(pick(&[], "", "low"), (String::new(), "low".into()));
    }

    #[test]
    fn claude_result_and_error() {
        let (value, cost) = parse_claude(br#"{"type":"result","structured_output":{"ok":true},"total_cost_usd":0.02}"#).unwrap();
        assert_eq!(value["ok"], true);
        assert_eq!(cost, Some(0.02));
        let error = parse_claude(br#"{"type":"result","is_error":true,"result":"Not logged in"}"#).unwrap_err();
        assert_eq!(error, "Not logged in");
        assert!(parse_claude(br#"{"type":"result","result":"text only"}"#).is_err());
    }

    #[test]
    fn claude_auth_status_uses_logged_in_field() {
        assert_eq!(parse_auth_status(Backend::Claude, true, br#"{"loggedIn":true}"#), Some(true));
        assert_eq!(parse_auth_status(Backend::Claude, true, br#"{"loggedIn":false}"#), Some(false));
        assert_eq!(parse_auth_status(Backend::Claude, true, b"invalid"), None);
        assert_eq!(parse_auth_status(Backend::Codex, true, b""), Some(true));
        assert_eq!(parse_auth_status(Backend::Codex, false, b""), Some(false));
    }

    #[test]
    fn login_script_quotes_cli_path_and_uses_device_auth_for_codex() {
        let path = Path::new("/tmp/it's codex/bin/codex");
        let script = login_script(path, Backend::Codex);
        assert!(script.contains("'/tmp/it'\\''s codex/bin/codex' login --device-auth"));
        assert!(script.contains("rm -f -- \"$0\""));
        assert!(login_script(Path::new("/tmp/claude"), Backend::Claude).contains("'/tmp/claude' auth login"));
    }
}
