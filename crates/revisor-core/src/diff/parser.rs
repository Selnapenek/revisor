use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub header: String,
    pub old_start: u64,
    pub old_count: u64,
    pub new_start: u64,
    pub new_count: u64,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
    pub old_lineno: Option<u64>,
    pub new_lineno: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDiff {
    pub hunks: Vec<DiffHunk>,
    pub is_binary: bool,
}

/// Parse a unified diff patch string into structured hunks
pub fn parse_patch(patch: &str) -> ParsedDiff {
    if patch.is_empty() {
        return ParsedDiff {
            hunks: vec![],
            is_binary: false,
        };
    }

    // Check for binary file marker
    if patch.contains("Binary files") || patch.contains("GIT binary patch") {
        return ParsedDiff {
            hunks: vec![],
            is_binary: true,
        };
    }

    let lines: Vec<&str> = patch.lines().collect();

    let mut hunks = Vec::new();
    let mut current_hunk: Option<DiffHunk> = None;
    let mut old_line: u64 = 0;
    let mut new_line: u64 = 0;

    for line in &lines {
        if let Some((header, os, oc, ns, nc)) = parse_hunk_header(line) {
            // Save previous hunk
            if let Some(hunk) = current_hunk.take() {
                hunks.push(hunk);
            }
            old_line = os;
            new_line = ns;
            current_hunk = Some(DiffHunk {
                header,
                old_start: os,
                old_count: oc,
                new_start: ns,
                new_count: nc,
                lines: Vec::new(),
            });
            continue;
        }

        // Skip "no newline" marker
        if line.starts_with("\\ No newline") {
            continue;
        }

        if let Some(ref mut hunk) = current_hunk {
            let (kind, content) = if let Some(rest) = line.strip_prefix('+') {
                (DiffLineKind::Added, rest)
            } else if let Some(rest) = line.strip_prefix('-') {
                (DiffLineKind::Removed, rest)
            } else if let Some(rest) = line.strip_prefix(' ') {
                (DiffLineKind::Context, rest)
            } else {
                // Treat lines without prefix as context
                (DiffLineKind::Context, *line)
            };

            let (old_no, new_no) = match kind {
                DiffLineKind::Context => {
                    let o = old_line;
                    let n = new_line;
                    old_line += 1;
                    new_line += 1;
                    (Some(o), Some(n))
                }
                DiffLineKind::Removed => {
                    let o = old_line;
                    old_line += 1;
                    (Some(o), None)
                }
                DiffLineKind::Added => {
                    let n = new_line;
                    new_line += 1;
                    (None, Some(n))
                }
            };

            hunk.lines.push(DiffLine {
                kind,
                content: content.to_string(),
                old_lineno: old_no,
                new_lineno: new_no,
            });
        }
    }

    // Push last hunk
    if let Some(hunk) = current_hunk {
        hunks.push(hunk);
    }

    ParsedDiff {
        hunks,
        is_binary: false,
    }
}

/// Parse a hunk header like `@@ -1,5 +1,6 @@ fn main()` into components
fn parse_hunk_header(line: &str) -> Option<(String, u64, u64, u64, u64)> {
    if !line.starts_with("@@") {
        return None;
    }

    let header = line.to_string();

    // Find the range info between @@ markers
    let after_first = line.strip_prefix("@@")?;
    let end = after_first.find("@@")?;
    let range_str = after_first[..end].trim();

    let parts: Vec<&str> = range_str.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    let (old_start, old_count) = parse_range(parts[0].strip_prefix('-')?)?;
    let (new_start, new_count) = parse_range(parts[1].strip_prefix('+')?)?;

    Some((header, old_start, old_count, new_start, new_count))
}

fn parse_range(s: &str) -> Option<(u64, u64)> {
    if let Some((start, count)) = s.split_once(',') {
        Some((start.parse().ok()?, count.parse().ok()?))
    } else {
        Some((s.parse().ok()?, 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_patch() {
        let patch = "@@ -1,3 +1,4 @@\n context\n-removed\n+added\n+new line\n context end";
        let result = parse_patch(patch);
        assert_eq!(result.hunks.len(), 1);
        assert_eq!(result.hunks[0].lines.len(), 5);
        assert!(!result.is_binary);
    }

    #[test]
    fn test_binary_file() {
        let patch = "Binary files differ";
        let result = parse_patch(patch);
        assert!(result.is_binary);
        assert!(result.hunks.is_empty());
    }

    #[test]
    fn test_empty_patch() {
        let result = parse_patch("");
        assert!(result.hunks.is_empty());
        assert!(!result.is_binary);
    }
}
