use super::parser::{DiffHunk, DiffLine, DiffLineKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideBySideRow {
    pub left: Option<DiffLine>,
    pub right: Option<DiffLine>,
    pub is_hunk_header: bool,
    pub hunk_header: Option<String>,
}

/// Convert hunks into side-by-side rows for split diff view
pub fn to_side_by_side(hunks: &[DiffHunk]) -> Vec<SideBySideRow> {
    let mut rows = Vec::new();

    for hunk in hunks {
        // Add hunk header row
        rows.push(SideBySideRow {
            left: None,
            right: None,
            is_hunk_header: true,
            hunk_header: Some(hunk.header.clone()),
        });

        let mut i = 0;
        let lines = &hunk.lines;

        while i < lines.len() {
            match lines[i].kind {
                DiffLineKind::Context => {
                    rows.push(SideBySideRow {
                        left: Some(lines[i].clone()),
                        right: Some(lines[i].clone()),
                        is_hunk_header: false,
                        hunk_header: None,
                    });
                    i += 1;
                }
                DiffLineKind::Removed | DiffLineKind::Added => {
                    // Collect consecutive removed and added lines
                    let mut removed = Vec::new();
                    let mut added = Vec::new();

                    while i < lines.len() && lines[i].kind != DiffLineKind::Context {
                        match lines[i].kind {
                            DiffLineKind::Removed => removed.push(lines[i].clone()),
                            DiffLineKind::Added => added.push(lines[i].clone()),
                            _ => break,
                        }
                        i += 1;
                    }

                    // Zip them together
                    let max_len = removed.len().max(added.len());
                    for j in 0..max_len {
                        rows.push(SideBySideRow {
                            left: removed.get(j).cloned(),
                            right: added.get(j).cloned(),
                            is_hunk_header: false,
                            hunk_header: None,
                        });
                    }
                }
            }
        }
    }

    rows
}
