use crate::diff::parser;
use crate::diff::side_by_side;
use crate::diff::word_diff;
use crate::provider::types::PrFile;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

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

#[tauri::command]
pub async fn get_pr_files(
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    number: u64,
) -> Result<Vec<ProcessedFile>, String> {
    let guard = state.provider.read().await;
    let provider = guard.as_ref().ok_or("Not authenticated")?;

    // Fetch all pages of files
    let mut all_files: Vec<PrFile> = Vec::new();
    let mut page = 1u32;
    loop {
        let result = provider
            .get_pr_files(&owner, &repo, number, page)
            .await
            .map_err(|e| e.to_string())?;

        all_files.extend(result.items);
        if !result.has_next_page {
            break;
        }
        page += 1;
    }

    // Process each file: parse patch, compute side-by-side, compute word diffs
    let processed: Vec<ProcessedFile> = all_files
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

                // Compute word-level diffs for paired removed/added lines
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
        .collect();

    Ok(processed)
}
