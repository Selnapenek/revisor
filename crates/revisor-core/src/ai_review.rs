//! One-click AI review: a recap of the change and suggested line comments.
//! The diff goes in the prompt, so no local clone is needed.
//! Recap layout from the `pr` skill (summary, evidence, merge danger) and the separate spec and
//! standards axes from the `code-review` skill: https://github.com/mattpocock/skills (MIT).

use crate::agent::{self, Backend};
use crate::diff::{parser::DiffLineKind, ProcessedFile};
use crate::provider::GitProvider;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt::Write as _;
use std::time::Duration;

/// About 60k tokens. Files past it are listed as skipped.
// ponytail: whole files in diff order; rank by risk or split into several calls if big PRs need it.
const MAX_DIFF_CHARS: usize = 240_000;
const MAX_STANDARDS_CHARS: usize = 30_000;
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
- comments: only what a careful human reviewer would post. Axis bug: real bugs, broken edge cases, security, data loss, \
performance traps. Axis spec: a line where the code does not do what the description says. Axis standards: a line that breaks a rule \
written in the repository standards files below; put the file name and the rule in `rule`; a written rule always wins over general \
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub summary: String,
    pub evidence: String,
    pub danger: Danger,
    pub spec: Spec,
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
    /// Changed files the agent did not see, with the reason.
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
    json!({"type":"object","additionalProperties":false,"required":["summary","evidence","danger","spec","comments"],"properties":{
        "summary":text("Markdown"),
        "evidence":text("Markdown"),
        "danger":{"type":"object","additionalProperties":false,"required":["door","blast_radius","note"],"properties":{
            "door":{"type":"string","enum":["one-way","two-way"]},"blast_radius":{"type":"string"},"note":{"type":"string"}}},
        "spec":{"type":"object","additionalProperties":false,"required":["status","note"],"properties":{
            "status":{"type":"string","enum":["matches","partial","differs","no_spec"]},"note":text("Markdown")}},
        "comments":{"type":"array","items":{"type":"object","additionalProperties":false,
            "required":["file","line","side","axis","severity","rule","body"],"properties":{
                "file":{"type":"string"},
                "line":{"type":"integer","description":"The number after N or O in the diff line"},
                "side":{"type":"string","enum":["new","old"],"description":"new for N lines, old for O lines"},
                "axis":{"type":"string","enum":["bug","spec","standards"]},
                "severity":{"type":"string","enum":["high","medium","low"]},
                "rule":text("Standards file and rule; empty for other axes"),
                "body":text("Markdown review comment")}}}}})
}

/// Standards files at `base_sha`, not at the PR head, so a PR cannot change the rules it is reviewed by.
pub async fn standards(provider: &dyn GitProvider, owner: &str, repo: &str, base_sha: &str) -> Vec<(String, String)> {
    let get = |path| provider.get_file_content(owner, repo, path, base_sha);
    let [a, b, c, d, e, f] = STANDARDS_FILES;
    let found = tokio::join!(get(a), get(b), get(c), get(d), get(e), get(f));
    let found = [found.0, found.1, found.2, found.3, found.4, found.5];
    let mut total = 0;
    STANDARDS_FILES
        .into_iter()
        .zip(found)
        .filter_map(|(path, text)| Some((path.to_string(), text.ok()?)))
        .filter(|(_, text)| {
            total += text.len();
            !text.trim().is_empty() && total <= MAX_STANDARDS_CHARS
        })
        .collect()
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
pub fn prompt(title: &str, body: &str, standards: &[(String, String)], files: &[ProcessedFile]) -> (String, Vec<(String, String)>) {
    let mut out = format!("# Spec: PR title and description\n\n{title}\n\n{body}\n\n# Repository standards files\n");
    if standards.is_empty() {
        out.push_str("\nNone. Do not write standards comments.\n");
    }
    for (path, text) in standards {
        let _ = write!(out, "\n## {path}\n\n{text}\n");
    }
    out.push_str("\n# Diff\n");
    let mut skipped = Vec::new();
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

pub async fn run(
    choice: Choice, head_sha: String, title: &str, body: &str, standards: Vec<(String, String)>, files: &[ProcessedFile],
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
    review.standards = standards.into_iter().map(|(path, _)| path).collect();
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
        let (prompt, skipped) = prompt("Title", "Body", &[], &files);
        assert!(prompt.contains("None. Do not write standards comments."));
        assert!(prompt.contains("N1  keep\n"));
        assert!(prompt.contains("O2 -old\n"));
        assert!(prompt.contains("N2 +new\n"));
        assert!(!prompt.contains("Cargo.lock"));
        assert_eq!(skipped, [("Cargo.lock".to_string(), "lock or generated file".to_string())]);
    }

    #[test]
    fn prompt_includes_standards_files() {
        let standards = [("CLAUDE.md".to_string(), "Never use unwrap.".to_string())];
        let (prompt, _) = prompt("Title", "Body", &standards, &[]);
        assert!(prompt.contains("## CLAUDE.md\n\nNever use unwrap."));
        assert!(!prompt.contains("None. Do not write standards comments."));
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
