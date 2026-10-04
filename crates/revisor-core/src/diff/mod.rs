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

/// Process raw PR files into frontend-ready format with parsed diffs
pub fn process_pr_files(files: Vec<PrFile>) -> Vec<ProcessedFile> {
    files
        .into_iter()
        .map(|file| {
            let diff = file.patch.as_deref().map(|patch| {
                let parsed = parser::parse_patch(patch);

                if parsed.is_binary || parsed.is_too_large {
                    return ProcessedDiff {
                        hunks: vec![],
                        side_by_side: vec![],
                        word_diffs: vec![],
                        is_binary: parsed.is_binary,
                        is_too_large: parsed.is_too_large,
                    };
                }

                let sbs_rows = side_by_side::to_side_by_side(&parsed.hunks);

                let word_diffs: Vec<WordDiffPair> = sbs_rows
                    .iter()
                    .filter(|row| !row.is_hunk_header)
                    .filter_map(|row| {
                        match (&row.left, &row.right) {
                            (Some(left), Some(right))
                                if left.kind == parser::DiffLineKind::Removed
                                    && right.kind == parser::DiffLineKind::Added =>
                            {
                                let (old_spans, new_spans) =
                                    word_diff::compute_word_diff(&left.content, &right.content);
                                Some(WordDiffPair {
                                    old_lineno: left.old_lineno,
                                    new_lineno: right.new_lineno,
                                    old_spans,
                                    new_spans,
                                })
                            }
                            _ => None,
                        }
                    })
                    .collect();

                ProcessedDiff {
                    hunks: parsed.hunks,
                    side_by_side: sbs_rows,
                    word_diffs,
                    is_binary: false,
                    is_too_large: false,
                }
            });

            ProcessedFile {
                sha: file.sha,
                filename: file.filename,
                status: file.status,
                additions: file.additions,
                deletions: file.deletions,
                diff,
            }
        })
        .collect()
}
