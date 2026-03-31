use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordDiffSpan {
    pub start: usize,
    pub end: usize,
    pub kind: WordDiffKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WordDiffKind {
    Equal,
    Insert,
    Delete,
}

/// Compute word-level diff between an old line and a new line.
/// Returns (old_spans, new_spans) indicating which character ranges changed.
pub fn compute_word_diff(old: &str, new: &str) -> (Vec<WordDiffSpan>, Vec<WordDiffSpan>) {
    let diff = TextDiff::from_words(old, new);
    let mut old_spans = Vec::new();
    let mut new_spans = Vec::new();
    let mut old_pos = 0usize;
    let mut new_pos = 0usize;

    for change in diff.iter_all_changes() {
        let len = change.value().len();
        match change.tag() {
            ChangeTag::Equal => {
                old_spans.push(WordDiffSpan {
                    start: old_pos,
                    end: old_pos + len,
                    kind: WordDiffKind::Equal,
                });
                new_spans.push(WordDiffSpan {
                    start: new_pos,
                    end: new_pos + len,
                    kind: WordDiffKind::Equal,
                });
                old_pos += len;
                new_pos += len;
            }
            ChangeTag::Delete => {
                old_spans.push(WordDiffSpan {
                    start: old_pos,
                    end: old_pos + len,
                    kind: WordDiffKind::Delete,
                });
                old_pos += len;
            }
            ChangeTag::Insert => {
                new_spans.push(WordDiffSpan {
                    start: new_pos,
                    end: new_pos + len,
                    kind: WordDiffKind::Insert,
                });
                new_pos += len;
            }
        }
    }

    (old_spans, new_spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_diff_simple() {
        let (old_spans, new_spans) = compute_word_diff("hello world", "hello rust");
        // Should have some equal parts and some changed parts
        assert!(!old_spans.is_empty());
        assert!(!new_spans.is_empty());
    }

    #[test]
    fn test_word_diff_identical() {
        let (old_spans, new_spans) = compute_word_diff("same text", "same text");
        assert!(old_spans.iter().all(|s| matches!(s.kind, WordDiffKind::Equal)));
        assert!(new_spans.iter().all(|s| matches!(s.kind, WordDiffKind::Equal)));
    }
}
