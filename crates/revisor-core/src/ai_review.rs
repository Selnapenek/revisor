//! One-click AI review: a recap of the change and suggested line comments.
//! The diff goes in the prompt, so no local clone is needed.
//! Recap layout from the `pr` skill (summary, evidence, merge danger) and the separate spec and
//! standards axes from the `code-review` skill: https://github.com/mattpocock/skills (MIT).

use crate::agent::{self, Backend};
use crate::cache::queries;
use crate::diff::{fetch_pr_files, parser::DiffLineKind, ProcessedFile};
use crate::provider::GitProvider;
use crate::AppState;
use futures::future::join_all;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::time::Duration;

/// About 60k tokens. Files past it are listed as skipped.
// ponytail: whole files in diff order; rank by risk or split into several calls if big PRs need it.
const MAX_DIFF_CHARS: usize = 240_000;
const MAX_STANDARDS_CHARS: usize = 160_000;
const MAX_STANDARDS_FILES: usize = 64;
const TIMEOUT: Duration = Duration::from_secs(600);

/// Files where repositories write their coding rules, read from the base commit.
pub const STANDARDS_FILES: [&str; 6] =
    ["CLAUDE.md", "AGENTS.md", "REVIEW.md", "CONTRIBUTING.md", "CODING_STANDARDS.md", ".github/copilot-instructions.md"];

const SYSTEM: &str = "You are a senior engineer reviewing a pull request for a colleague. Fill each field:\n\
- summary: what behavior changes and how, for a reviewer who has not read the code. 1-3 sentences, then the smallest sketch \
that makes the change clear (pseudocode, call tree, file tree or a short diff) in a code block. Omit the sketch if the sentences are enough.\n\
- evidence: which changed tests or other proof in the diff show that the new behavior works. Name each test and what it checks. \
If the diff proves nothing, say what proof is missing.\n\
- danger: two-way door if a revert fully undoes the change; one-way door if not (data or schema migration, persisted format, \
public API, messages sent to users). blast_radius is one word. note is one sentence on what could break.\n\
- spec: the PR title and description are the spec. matches, partial or differs, and a note on missing requirements, \
wrong implementations and changes the description does not mention. no_spec if the description states no requirements.\n\
- standards_check: use only the supplied base-commit standards files and visible diff. Name the rules relevant to this change \
in applicable, explain which are satisfied by visible lines in passing, and name rules that need a full checkout, test run, \
or other evidence in unchecked. If there are no supplied rules, say so. Do not claim a full repository check.\n\
- comments: only what a careful human reviewer would post. Axis bug: real bugs, broken edge cases, security, data loss, \
performance traps. Axis spec: a line where the code does not do what the description says. Axis standards: a line that breaks a rule \
written in the repository standards files below. Keep the primary axis bug for a real bug, even when it also breaks a written rule. \
For every comment that breaks a written rule, including a bug comment, put the standards file and exact rule in `rule`; otherwise \
leave `rule` empty. Do not duplicate a comment in two axes. A written rule always wins over general \
taste; never flag what a linter or formatter enforces. No style opinions without a written rule, no praise, no restating the code. \
Skip a comment if you are not confident it is right. Fewer good comments are better than many weak ones; at most 10. \
Write each comment body to the PR author, short and concrete; say why it matters and how to fix it. \
Use a GitHub ```suggestion block only for an exact replacement of the commented line.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub file: String,
    pub line: u64,
    /// "new" for added or unchanged lines, "old" for removed lines.
    pub side: String,
    /// "bug", "spec" or "standards". Kept apart, as the code-review skill asks.
    #[serde(default)]
    pub axis: String,
    /// "high", "medium" or "low".
    pub severity: String,
    /// For standards comments: the file and the rule that is broken.
    #[serde(default)]
    pub rule: String,
    pub body: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Danger {
    /// "one-way" or "two-way".
    pub door: String,
    pub blast_radius: String,
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Spec {
    /// "matches", "partial", "differs" or "no_spec".
    pub status: String,
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StandardsCheck {
    pub applicable: String,
    pub passing: String,
    pub unchecked: String,
}

#[derive(Debug, Clone, Default)]
pub struct StandardsInput {
    pub files: Vec<(String, String)>,
    pub skipped: Vec<(String, String)>,
}

impl StandardsInput {
    fn status(&self) -> &'static str {
        if !self.skipped.is_empty() { "limited" }
        else if self.files.is_empty() { "no_rules" }
        else { "checked" }
    }

    fn include(&mut self, path: String, text: String, limit: usize) -> bool {
        let used: usize = self.files.iter().map(|(_, text)| text.len()).sum();
        if used + text.len() > limit {
            self.skipped.push((path, "standards size limit".into()));
            return false;
        }
        if text.trim().is_empty() {
            return false;
        }
        self.files.push((path, text));
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub summary: String,
    pub evidence: String,
    pub danger: Danger,
    pub spec: Spec,
    pub standards_check: StandardsCheck,
    /// Derived from loaded and skipped base-commit rule files, not from the AI response.
    #[serde(default)]
    pub standards_status: String,
    pub comments: Vec<Comment>,
    /// Commit the review is for; a newer head makes it outdated.
    #[serde(default)]
    pub head_sha: String,
    /// CLI, model and effort, for display.
    #[serde(default)]
    pub backend: String,
    /// Standards files the agent got.
    #[serde(default)]
    pub standards: Vec<String>,
    /// Changed files and standards files the agent did not see, with the reason.
    #[serde(default)]
    pub skipped: Vec<(String, String)>,
    #[serde(default)]
    pub seconds: u64,
    #[serde(default)]
    pub cost_usd: Option<f64>,
}

pub const AXES: [(&str, &str); 3] = [("bug", "Bugs"), ("spec", "Spec"), ("standards", "Standards")];

fn schema() -> Value {
    let text = |description: &str| json!({"type":"string","description":description});
    json!({"type":"object","additionalProperties":false,"required":["summary","evidence","danger","spec","standards_check","comments"],"properties":{
        "summary":text("Markdown"),
        "evidence":text("Markdown"),
        "danger":{"type":"object","additionalProperties":false,"required":["door","blast_radius","note"],"properties":{
            "door":{"type":"string","enum":["one-way","two-way"]},"blast_radius":{"type":"string"},"note":{"type":"string"}}},
        "spec":{"type":"object","additionalProperties":false,"required":["status","note"],"properties":{
            "status":{"type":"string","enum":["matches","partial","differs","no_spec"]},"note":text("Markdown")}},
        "standards_check":{"type":"object","additionalProperties":false,"required":["applicable","passing","unchecked"],"properties":{
            "applicable":text("Relevant written rules and why they apply, or none"),
            "passing":text("Rules supported by visible diff evidence, or none confirmed"),
            "unchecked":text("Rules that need evidence outside the supplied diff, or none")}},
        "comments":{"type":"array","items":{"type":"object","additionalProperties":false,
            "required":["file","line","side","axis","severity","rule","body"],"properties":{
                "file":{"type":"string"},
                "line":{"type":"integer","description":"The number after N or O in the diff line"},
                "side":{"type":"string","enum":["new","old"],"description":"new for N lines, old for O lines"},
                "axis":{"type":"string","enum":["bug","spec","standards"]},
                "severity":{"type":"string","enum":["high","medium","low"]},
                "rule":text("Standards file and exact rule when any comment breaks a written rule, including bugs; else empty"),
                "body":text("Markdown review comment")}}}}})
}

/// Find scoped Markdown rule paths named in a standards file. Reject examples and paths outside the repo.
fn linked_rule_paths(source: &str, text: &str) -> Vec<String> {
    let parent = source.rsplit_once('/').map_or("", |(parent, _)| parent);
    let in_rule_dir = parent == "rules" || parent.ends_with("/rules");
    let mut paths = Vec::new();
    for token in text.split(|c: char| c.is_whitespace() || "[]()`<>\"'".contains(c)) {
        let raw = token.split('#').next().unwrap_or("");
        if !raw.ends_with(".md") || raw.contains(':') || raw.contains('*') || raw.contains('?') || raw.starts_with("//") {
            continue;
        }
        // A bare filename in AGENTS.md can refer to a scoped rule or be an example in a
        // tree. Bare names are unambiguous only inside the rule directory itself.
        let root_standard = STANDARDS_FILES.contains(&raw);
        if !root_standard && !in_rule_dir && !raw.contains("/rules/") && !raw.starts_with("rules/") {
            continue;
        }
        let relative = raw.trim_start_matches('/');
        let mut parts = Vec::new();
        // Explicit repo paths use the root. Plain paths use the referring file's directory.
        let root_path = root_standard || relative.starts_with(".ai/") || relative.starts_with(".github/") || raw.starts_with('/');
        if !root_path {
            parts.extend(parent.split('/').filter(|part| !part.is_empty()));
        }
        let mut valid = true;
        for part in relative.split('/') {
            match part {
                "" | "." => {}
                ".." => { if parts.pop().is_none() { valid = false; break; } }
                _ => parts.push(part),
            }
        }
        if valid && !parts.is_empty() {
            let path = parts.join("/");
            if !paths.contains(&path) { paths.push(path); }
        }
    }
    paths
}

fn load_failure_reason(error: &anyhow::Error) -> &'static str {
    let message = error.to_string();
    if message.contains("GitHub API error 404 ") { "not found at base commit" }
    else if message.contains("GitHub API error 401 ") || message.contains("GitHub API error 403 ") { "access denied" }
    else if message.contains("rate limit") || message.contains("GitHub API error 429 ") { "GitHub rate limit" }
    else if message.contains("Network request failed") { "network request failed" }
    else { "GitHub request failed" }
}

/// Standards files at `base_sha`, not at the PR head, so a PR cannot change the rules it is reviewed by.
pub async fn standards(provider: &dyn GitProvider, owner: &str, repo: &str, base_sha: &str) -> StandardsInput {
    let get = |path| provider.get_file_content(owner, repo, path, base_sha);
    let [a, b, c, d, e, f] = STANDARDS_FILES;
    let found = tokio::join!(get(a), get(b), get(c), get(d), get(e), get(f));
    let found = [found.0, found.1, found.2, found.3, found.4, found.5];
    let mut input = StandardsInput::default();
    let mut queue = VecDeque::new();
    let mut seen: HashSet<String> = STANDARDS_FILES.iter().map(|path| (*path).to_string()).collect();
    for (path, result) in STANDARDS_FILES.into_iter().zip(found) {
        match result {
            Ok(text) if input.include(path.into(), text.clone(), MAX_STANDARDS_CHARS) => {
                queue.extend(linked_rule_paths(path, &text));
            }
            Ok(_) => {}
            Err(error) if error.to_string().contains(" 404 ") => {}
            Err(error) => input.skipped.push((path.into(), load_failure_reason(&error).into())),
        }
    }
    while !queue.is_empty() {
        let mut batch = Vec::new();
        while batch.len() < 8 {
            let Some(path) = queue.pop_front() else { break };
            if !seen.insert(path.clone()) { continue; }
            if seen.len() > MAX_STANDARDS_FILES {
                input.skipped.push((path, "too many linked standards files".into()));
                continue;
            }
            batch.push(path);
        }
        // join_all drives the requests together and keeps their order in the result.
        // https://docs.rs/futures/latest/futures/future/fn.join_all.html
        let results = join_all(batch.iter().map(|path| provider.get_file_content(owner, repo, path, base_sha))).await;
        for (path, result) in batch.into_iter().zip(results) {
            match result {
                Ok(text) if input.include(path.clone(), text.clone(), MAX_STANDARDS_CHARS) => {
                    queue.extend(linked_rule_paths(&path, &text));
                }
                Ok(_) => {}
                Err(error) => input.skipped.push((path, load_failure_reason(&error).into())),
            }
        }
    }
    input
}

fn skip_reason(file: &ProcessedFile) -> Option<&'static str> {
    const GENERATED: [&str; 7] = [".lock", "-lock.json", "-lock.yaml", ".min.js", ".snap", ".svg", "go.sum"];
    if GENERATED.iter().any(|end| file.filename.ends_with(end)) {
        return Some("lock or generated file");
    }
    match &file.diff {
        _ if file.is_large() => Some("diff too large or not loaded"),
        Some(d) if d.is_binary => Some("binary"),
        _ => None,
    }
}

/// The prompt and the files left out. Each diff line has its line number so comments can point at it:
/// `N12 +` added, `O7 -` removed, `N13  ` unchanged.
pub fn prompt(title: &str, body: &str, standards: &StandardsInput, files: &[ProcessedFile]) -> (String, Vec<(String, String)>) {
    let mut out = format!("# Spec: PR title and description\n\n{title}\n\n{body}\n\n# Repository standards files\n");
    if standards.files.is_empty() {
        out.push_str("\nNone. Do not write standards comments.\n");
    }
    for (path, text) in &standards.files {
        let _ = write!(out, "\n## {path}\n\n{text}\n");
    }
    out.push_str("\n# Diff\n");
    let mut skipped = standards.skipped.clone();
    for file in files {
        let mut text = format!("\n## {} ({})\n", file.filename, file.status);
        let reason = skip_reason(file);
        for line in file.diff.iter().filter(|_| reason.is_none()).flat_map(|d| &d.hunks).flat_map(|h| &h.lines) {
            let _ = match (line.kind, line.old_lineno, line.new_lineno) {
                (DiffLineKind::Removed, Some(n), _) => writeln!(text, "O{n} -{}", line.content),
                (DiffLineKind::Added, _, Some(n)) => writeln!(text, "N{n} +{}", line.content),
                (_, _, Some(n)) => writeln!(text, "N{n}  {}", line.content),
                _ => Ok(()),
            };
        }
        match reason {
            Some(reason) => skipped.push((file.filename.clone(), reason.to_string())),
            None if out.len() + text.len() > MAX_DIFF_CHARS => skipped.push((file.filename.clone(), "over the size limit".into())),
            None => out.push_str(&text),
        }
    }
    (out, skipped)
}

/// The comment points at a line shown in the diff, so GitHub accepts it.
pub fn in_diff(files: &[ProcessedFile], comment: &Comment) -> bool {
    let Some(file) = files.iter().find(|f| f.filename == comment.file) else { return false };
    file.diff.iter().flat_map(|d| &d.hunks).flat_map(|h| &h.lines).any(|l| match comment.side.as_str() {
        "old" => l.kind == DiffLineKind::Removed && l.old_lineno == Some(comment.line),
        _ => l.kind != DiffLineKind::Removed && l.new_lineno == Some(comment.line),
    })
}

pub struct Choice {
    pub backend: Backend,
    /// `None` uses the CLI default.
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// Settings key of the saved review of a PR. An empty `number` gives the prefix for the whole repo.
// v5: the full base-commit rule catalog fits the bounded review input.
pub fn review_key(owner: &str, repo: &str, number: impl std::fmt::Display) -> String {
    format!("ai-review-v5:{owner}/{repo}#{number}")
}

/// PR number to the head commit of its saved review, for one repo.
pub fn reviewed_heads(conn: &Connection, owner: &str, repo: &str) -> anyhow::Result<HashMap<u64, String>> {
    #[derive(Deserialize)]
    struct Saved {
        #[serde(default)]
        head_sha: String,
    }
    let prefix = review_key(owner, repo, "");
    Ok(queries::get_settings_with_prefix(conn, &prefix)?
        .into_iter()
        .filter_map(|(key, json)| {
            let number = key.strip_prefix(&prefix)?.parse().ok()?;
            Some((number, serde_json::from_str::<Saved>(&json).ok()?.head_sha))
        })
        .collect())
}

pub async fn save(s: &AppState, key: &str, review: &Review) -> Result<(), String> {
    let json = serde_json::to_string(review).map_err(|e| e.to_string())?;
    queries::set_setting(s.cache.lock().await.conn(), key, &json).map_err(|e| e.to_string())
}

/// The CLI, model and effort picked in the AI tab (same setting keys); else the first installed CLI and its defaults.
pub async fn saved_choice(s: &AppState) -> Result<Choice, String> {
    let installed = agent::installed();
    let get = |key: String| async move {
        queries::get_setting(s.cache.lock().await.conn(), &key).map(Option::unwrap_or_default).map_err(|e| e.to_string())
    };
    let preferred = Backend::parse(&get("ai-backend".into()).await?).filter(|b| installed.contains(b));
    let backend = preferred
        .or(installed.first().copied())
        .ok_or("Install Claude Code or Codex and sign in to use AI review.")?;
    let name = backend.executable();
    let (model, effort) = (get(format!("ai-model:{name}")).await?, get(format!("ai-effort:{name}")).await?);
    // Blocking: the Codex model list comes from its CLI.
    let picked = tokio::task::spawn_blocking(move || agent::pick(&agent::models(backend), &model, &effort));
    let (model, effort) = picked.await.map_err(|e| e.to_string())?;
    let some = |s: String| (!s.is_empty()).then_some(s);
    Ok(Choice { backend, model: some(model), effort: some(effort) })
}

/// Review the latest commit of a PR that is not open on screen, and save the review.
pub async fn review_pr(s: &AppState, owner: &str, repo: &str, number: u64) -> Result<Review, String> {
    let choice = saved_choice(s).await?;
    let (pr, files, standards) = {
        let p = s.provider().await?;
        let (pr, files) = tokio::join!(p.get_pull_request(owner, repo, number), fetch_pr_files(&*p, owner, repo, number));
        let (pr, files) = (pr.map_err(|e| e.to_string())?, files?);
        let standards = standards(&*p, owner, repo, &pr.base_sha).await;
        (pr, files, standards)
    };
    let body = pr.body.unwrap_or_default();
    let review = run(choice, pr.head_sha, &pr.title, &body, standards, &files).await?;
    save(s, &review_key(owner, repo, number), &review).await?;
    Ok(review)
}

pub async fn run(
    choice: Choice, head_sha: String, title: &str, body: &str, standards: StandardsInput, files: &[ProcessedFile],
) -> Result<Review, String> {
    let (prompt, skipped) = prompt(title, body, &standards, files);
    let Choice { backend, model, effort } = choice;
    let output = agent::run(backend, model.as_deref(), effort.as_deref(), SYSTEM, &prompt, &schema(), TIMEOUT).await?;
    let mut review: Review = serde_json::from_value(output.value).map_err(|e| format!("Unexpected AI output: {e}"))?;
    let rank = |c: &Comment| {
        let axis = AXES.iter().position(|(a, _)| *a == c.axis).unwrap_or(3);
        (axis, ["high", "medium", "low"].iter().position(|s| *s == c.severity).unwrap_or(3))
    };
    review.comments.sort_by_key(rank);
    review.standards_status = standards.status().into();
    review.standards = standards.files.into_iter().map(|(path, _)| path).collect();
    review.head_sha = head_sha;
    review.backend = [Some(backend.label().to_string()), model, effort.map(|e| format!("{e} effort"))]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    review.skipped = skipped;
    review.seconds = output.duration.as_secs();
    review.cost_usd = output.cost_usd;
    Ok(review)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::diff_contents;

    fn file(name: &str, old: &str, new: &str) -> ProcessedFile {
        ProcessedFile { sha: String::new(), filename: name.into(), previous_filename: None, status: "modified".into(),
            additions: 1, deletions: 1, diff: Some(diff_contents(old, new)) }
    }

    #[test]
    fn prompt_numbers_lines_and_lists_skipped_files() {
        let files = [file("src/a.rs", "keep\nold\n", "keep\nnew\n"), file("Cargo.lock", "a\n", "b\n")];
        let (prompt, skipped) = prompt("Title", "Body", &StandardsInput::default(), &files);
        assert!(prompt.contains("None. Do not write standards comments."));
        assert!(prompt.contains("N1  keep\n"));
        assert!(prompt.contains("O2 -old\n"));
        assert!(prompt.contains("N2 +new\n"));
        assert!(!prompt.contains("Cargo.lock"));
        assert_eq!(skipped, [("Cargo.lock".to_string(), "lock or generated file".to_string())]);
    }

    #[test]
    fn prompt_includes_standards_files() {
        let standards = StandardsInput { files: vec![("CLAUDE.md".to_string(), "Never use unwrap.".to_string())], skipped: vec![] };
        let (prompt, _) = prompt("Title", "Body", &standards, &[]);
        assert!(prompt.contains("## CLAUDE.md\n\nNever use unwrap."));
        assert!(!prompt.contains("None. Do not write standards comments."));
    }

    #[test]
    fn linked_rules_resolve_from_root_and_parent_without_escaping_repo() {
        let text = "See [fallback](.ai/rules/no-silent-fallbacks.md#review), `rules/pagination.md`, and [external](https://example.com/rule.md).";
        assert_eq!(linked_rule_paths("AGENTS.md", text), vec![".ai/rules/no-silent-fallbacks.md", "rules/pagination.md"]);
        assert_eq!(linked_rule_paths("docs/AGENTS.md", "[rule](../.ai/rules/check.md)"), vec![".ai/rules/check.md"]);
        assert!(linked_rule_paths("AGENTS.md", "[bad](../outside.md)").is_empty());
    }

    #[test]
    fn agents_rule_catalog_does_not_fetch_bare_names_or_examples_from_repo_root() {
        let agents = "Skim `.ai/rules/README.md`. See `no-silent-fallbacks.md`. \
            │   └── *.md # files. See `docs/ingestion-guide.md`.";
        assert_eq!(linked_rule_paths("AGENTS.md", agents), vec![".ai/rules/README.md"]);
        let catalog = "Safety: `no-silent-fallbacks.md`, `never-invent-data.md`.";
        assert_eq!(linked_rule_paths(".ai/rules/README.md", catalog),
            vec![".ai/rules/no-silent-fallbacks.md", ".ai/rules/never-invent-data.md"]);
        assert_eq!(linked_rule_paths(".ai/rules/README.md", "Root `AGENTS.md` maps the rules."), vec!["AGENTS.md"]);
    }

    #[test]
    fn documented_rule_catalog_fits_the_review_budget() {
        let mut input = StandardsInput::default();
        input.include("AGENTS.md".into(), "a".repeat(23_000), MAX_STANDARDS_CHARS);
        input.include("CODING_STANDARDS.md".into(), "b".repeat(6_000), MAX_STANDARDS_CHARS);
        for n in 0..50 {
            input.include(format!(".ai/rules/rule-{n}.md"), "c".repeat(2_500), MAX_STANDARDS_CHARS);
        }
        assert_eq!(input.files.len(), 52);
        assert!(input.skipped.is_empty());
    }

    #[test]
    fn load_failure_names_the_http_or_network_cause() {
        assert_eq!(load_failure_reason(&anyhow::anyhow!("GitHub API error 404 Not Found: missing")), "not found at base commit");
        assert_eq!(load_failure_reason(&anyhow::anyhow!("GitHub API error 403 Forbidden: denied")), "access denied");
        assert_eq!(load_failure_reason(&anyhow::anyhow!("Network request failed")), "network request failed");
    }

    #[test]
    fn standards_budget_records_each_skipped_file() {
        let mut input = StandardsInput::default();
        input.include("AGENTS.md".into(), "See [rule](.ai/rules/check.md)".into(), 50);
        input.include(".ai/rules/check.md".into(), "A rule that is too long".into(), 50);
        assert_eq!(input.files.len(), 1);
        assert_eq!(input.skipped, vec![(".ai/rules/check.md".to_string(), "standards size limit".to_string())]);
        assert_eq!(input.status(), "limited");
        let (_, skipped) = prompt("Title", "Body", &input, &[]);
        assert_eq!(skipped, input.skipped);
    }

    #[test]
    fn standards_status_reflects_available_evidence() {
        let mut input = StandardsInput::default();
        assert_eq!(input.status(), "no_rules");
        input.include("AGENTS.md".into(), "Never panic".into(), 100);
        assert_eq!(input.status(), "checked");
    }

    #[test]
    fn ai_output_keeps_rule_citation_on_bug_without_local_metadata() {
        let output = json!({
            "summary":"A change", "evidence":"A test", "danger":{"door":"two-way","blast_radius":"local","note":"Can revert"},
            "spec":{"status":"matches","note":"Matches"},
            "standards_check":{"applicable":"AGENTS.md: no silent fallback","passing":"None confirmed","unchecked":"CI results"},
            "comments":[{"file":"src/a.rs","line":2,"side":"new","axis":"bug","severity":"high",
                "rule":"AGENTS.md: no silent fallback","body":"This hides the error."}]
        });
        let review: Review = serde_json::from_value(output).unwrap();
        assert_eq!(review.comments[0].axis, "bug");
        assert_eq!(review.comments[0].rule, "AGENTS.md: no silent fallback");
        assert!(review.standards_status.is_empty());
    }

    #[test]
    fn reviewed_heads_reads_only_this_repo() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT)", []).unwrap();
        let review = |head: &str| json!({"head_sha": head, "summary": "s"}).to_string();
        queries::set_setting(&conn, &review_key("o", "a_b", 1), &review("abc")).unwrap();
        queries::set_setting(&conn, &review_key("o", "a_b", 2), "not json").unwrap();
        // `_` would match any character with LIKE.
        queries::set_setting(&conn, &review_key("o", "axb", 3), &review("def")).unwrap();
        queries::set_setting(&conn, &review_key("o", "a_b2", 4), &review("ghi")).unwrap();
        let heads = reviewed_heads(&conn, "o", "a_b").unwrap();
        assert_eq!(heads, HashMap::from([(1, "abc".to_string())]));
    }

    #[test]
    fn comment_must_point_at_a_diff_line() {
        let files = [file("src/a.rs", "keep\nold\n", "keep\nnew\n")];
        let comment = |line, side: &str| Comment { file: "src/a.rs".into(), line, side: side.into(), axis: "bug".into(),
            severity: "high".into(), rule: String::new(), body: String::new() };
        assert!(in_diff(&files, &comment(2, "new")));
        assert!(in_diff(&files, &comment(1, "new")));
        assert!(in_diff(&files, &comment(2, "old")));
        assert!(!in_diff(&files, &comment(1, "old")));
        assert!(!in_diff(&files, &comment(40, "new")));
    }
}
