pub mod expand;
pub mod parser;
pub mod search;
pub mod side_by_side;
pub mod word_diff;

use crate::provider::{types::PrFile, GitProvider};
use serde::{Deserialize, Serialize};

/// Processed file with parsed diff data, ready for frontend rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedFile {
    pub sha: String,
    pub filename: String,
    /// Old path of a renamed file.
    pub previous_filename: Option<String>,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub diff: Option<ProcessedDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedDiff {
    pub hunks: Vec<parser::DiffHunk>,
    pub side_by_side: Vec<side_by_side::SideBySideRow>,
    pub word_diffs: Vec<WordDiffPair>,
    pub is_binary: bool,
    pub is_too_large: bool,
}

/// Word diff for a removed/added line pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordDiffPair {
    pub old_lineno: Option<u64>,
    pub new_lineno: Option<u64>,
    pub old_spans: Vec<word_diff::WordDiffSpan>,
    pub new_spans: Vec<word_diff::WordDiffSpan>,
}

/// Fetch every page of PR files and parse their diffs.
pub async fn fetch_pr_files(
    provider: &dyn GitProvider,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<Vec<ProcessedFile>, String> {
    let mut all_files: Vec<PrFile> = Vec::new();
    let mut page = 1u32;
    loop {
        let result = provider
            .get_pr_files(owner, repo, number, page)
            .await
            .map_err(|e| e.to_string())?;
        all_files.extend(result.items);
        if !result.has_next_page {
            break;
        }
        page += 1;
    }
    Ok(process_pr_files(all_files))
}

/// Patches over this many lines are not parsed.
const MAX_DIFF_LINES: usize = 20_000;

impl ProcessedFile {
    /// GitHub omitted the patch for a changed text file, or it passed our size cap.
    /// Binary files and pure renames have no changed lines.
    pub fn is_large(&self) -> bool {
        match &self.diff {
            None => self.additions + self.deletions > 0,
            Some(d) => d.is_too_large,
        }
    }
}

/// Process raw PR files into frontend-ready format with parsed diffs
pub fn process_pr_files(files: Vec<PrFile>) -> Vec<ProcessedFile> {
    files
        .into_iter()
        .map(|file| {
            let diff = file.patch.as_deref().map(|patch| {
                if patch.lines().count() > MAX_DIFF_LINES {
                    ProcessedDiff {
                        hunks: vec![],
                        side_by_side: vec![],
                        word_diffs: vec![],
                        is_binary: false,
                        is_too_large: true,
                    }
                } else {
                    process_patch(patch)
                }
            });

            ProcessedFile {
                sha: file.sha,
                filename: file.filename,
                previous_filename: file.previous_filename,
                status: file.status,
                additions: file.additions,
                deletions: file.deletions,
                diff,
            }
        })
        .collect()
}

/// Diff two versions of a file, for diffs that GitHub omits or that pass the size cap.
pub fn diff_contents(old: &str, new: &str) -> ProcessedDiff {
    let patch = similar::TextDiff::from_lines(old, new).unified_diff().context_radius(3).to_string();
    process_patch(&patch)
}

fn process_patch(patch: &str) -> ProcessedDiff {
    let parsed = parser::parse_patch(patch);

    if parsed.is_binary {
        return ProcessedDiff {
            hunks: vec![],
            side_by_side: vec![],
            word_diffs: vec![],
            is_binary: true,
            is_too_large: false,
        };
    }

    let sbs_rows = side_by_side::to_side_by_side(&parsed.hunks);

    let word_diffs: Vec<WordDiffPair> = sbs_rows
        .iter()
        .filter(|row| !row.is_hunk_header)
        .filter_map(|row| match (&row.left, &row.right) {
            (Some(left), Some(right))
                if left.kind == parser::DiffLineKind::Removed && right.kind == parser::DiffLineKind::Added =>
            {
                let (old_spans, new_spans) = word_diff::compute_word_diff(&left.content, &right.content);
                Some(WordDiffPair {
                    old_lineno: left.old_lineno,
                    new_lineno: right.new_lineno,
                    old_spans,
                    new_spans,
                })
            }
            _ => None,
        })
        .collect();

    ProcessedDiff {
        hunks: parsed.hunks,
        side_by_side: sbs_rows,
        word_diffs,
        is_binary: false,
        is_too_large: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_contents_numbers_lines() {
        let old = "a\nb\nc\nd\ne\nf\ng\nh\n";
        let new = "a\nb\nc\nd\nE\nf\ng\nh\n";
        let diff = diff_contents(old, new);
        assert_eq!(diff.hunks.len(), 1);
        let changed: Vec<_> = diff.hunks[0]
            .lines
            .iter()
            .filter(|l| l.kind != parser::DiffLineKind::Context)
            .map(|l| (l.kind, l.content.as_str(), l.old_lineno, l.new_lineno))
            .collect();
        assert_eq!(
            changed,
            [
                (parser::DiffLineKind::Removed, "e", Some(5), None),
                (parser::DiffLineKind::Added, "E", None, Some(5)),
            ]
        );
        assert_eq!(diff.word_diffs.len(), 1);
    }

    #[test]
    fn diff_contents_added_file() {
        let diff = diff_contents("", "x\ny");
        let lines = &diff.hunks[0].lines;
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.kind == parser::DiffLineKind::Added));
        assert_eq!(lines[1].new_lineno, Some(2));
    }

    #[test]
    fn is_large_only_for_omitted_or_capped_diffs() {
        let file = |additions, diff| ProcessedFile {
            sha: String::new(),
            filename: String::new(),
            previous_filename: None,
            status: "modified".into(),
            additions,
            deletions: 0,
            diff,
        };
        assert!(file(5, None).is_large());
        // Binary file or pure rename.
        assert!(!file(0, None).is_large());
        assert!(!file(5, Some(diff_contents("a", "b"))).is_large());
        let capped = process_pr_files(vec![PrFile {
            sha: String::new(),
            filename: String::new(),
            previous_filename: None,
            status: "modified".into(),
            additions: 1,
            deletions: 0,
            patch: Some("+x\n".repeat(MAX_DIFF_LINES + 1)),
        }]);
        assert!(capped[0].is_large());
    }
}
