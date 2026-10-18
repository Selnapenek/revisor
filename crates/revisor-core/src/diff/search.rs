//! Code search across diff lines. All offsets are UTF-8 byte offsets on char
//! boundaries, matching `WordDiffSpan`.

use super::word_diff::{WordDiffKind, WordDiffSpan};
use super::ProcessedFile;
use std::ops::Range;

/// One occurrence of the query in `files[file].diff.hunks[hunk].lines[line].content`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeMatch {
    pub file: usize,
    pub hunk: usize,
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

/// A styled piece of a rendered line.
#[derive(Debug, Clone)]
pub struct Segment {
    pub range: Range<usize>,
    pub kind: WordDiffKind,
    pub matched: bool,
    pub active: bool,
}

/// Non-overlapping, case-insensitive occurrences of `query` in `content`.
pub fn find_matches(content: &str, query: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    if query.is_empty() {
        return out;
    }
    let mut from = 0;
    while from < content.len() {
        let Some(end) = match_at(&content[from..], query) else {
            from += content[from..].chars().next().map_or(1, char::len_utf8);
            continue;
        };
        out.push(from..from + end);
        from += end;
    }
    out
}

/// Byte length of the prefix of `s` that case-insensitively equals `query`.
fn match_at(s: &str, query: &str) -> Option<usize> {
    let mut it = s.char_indices();
    for q in query.chars() {
        let (_, c) = it.next()?;
        if !c.to_lowercase().eq(q.to_lowercase()) {
            return None;
        }
    }
    Some(it.next().map_or(s.len(), |(i, _)| i))
}

/// All matches across every file/hunk/line, in navigation order.
pub fn search_files<'a>(files: impl IntoIterator<Item = &'a ProcessedFile>, query: &str) -> Vec<CodeMatch> {
    let mut out = Vec::new();
    for (file, f) in files.into_iter().enumerate() {
        let Some(diff) = &f.diff else { continue };
        for (hunk, h) in diff.hunks.iter().enumerate() {
            for (line, l) in h.lines.iter().enumerate() {
                out.extend(find_matches(&l.content, query).into_iter().map(|r| CodeMatch {
                    file,
                    hunk,
                    line,
                    start: r.start,
                    end: r.end,
                }));
            }
        }
    }
    out
}

/// Index range into `results` (from `search_files`) of the matches on one line.
pub fn line_matches(results: &[CodeMatch], file: usize, hunk: usize, line: usize) -> Range<usize> {
    let key = (file, hunk, line);
    let start = results.partition_point(|m| (m.file, m.hunk, m.line) < key);
    let end = results.partition_point(|m| (m.file, m.hunk, m.line) <= key);
    start..end
}

/// Next (Enter) or previous (Shift+Enter) active match index, wrapping around.
pub fn step(active: usize, total: usize, forward: bool) -> usize {
    match total {
        0 => 0,
        _ if forward => (active + 1) % total,
        _ => (active % total + total - 1) % total,
    }
}

/// Split a line into segments by word-diff spans and search matches.
/// Empty `word_spans` means the whole line is one `Equal` span.
/// `active` indexes into `matches`.
pub fn segments(
    content_len: usize,
    word_spans: &[WordDiffSpan],
    matches: &[Range<usize>],
    active: Option<usize>,
) -> Vec<Segment> {
    let whole = [WordDiffSpan { start: 0, end: content_len, kind: WordDiffKind::Equal }];
    let base = if word_spans.is_empty() {
        if content_len == 0 {
            return Vec::new();
        }
        &whole[..]
    } else {
        word_spans
    };

    let mut out = Vec::new();
    for span in base {
        let mut cuts = vec![span.start, span.end];
        for m in matches {
            cuts.extend([m.start, m.end].into_iter().filter(|&c| c > span.start && c < span.end));
        }
        cuts.sort_unstable();
        cuts.dedup();
        for w in cuts.windows(2) {
            let hit = matches.iter().position(|m| m.start <= w[0] && m.end >= w[1]);
            out.push(Segment {
                range: w[0]..w[1],
                kind: span.kind,
                matched: hit.is_some(),
                active: hit.is_some() && hit == active,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::parser::{DiffHunk, DiffLine, DiffLineKind};
    use crate::diff::ProcessedDiff;

    #[test]
    fn finds_case_insensitive_non_overlapping() {
        assert_eq!(find_matches("Foo foo FOO", "foo"), vec![0..3, 4..7, 8..11]);
        assert_eq!(find_matches("aaaa", "aa"), vec![0..2, 2..4]);
        assert!(find_matches("abc", "").is_empty());
        assert!(find_matches("", "a").is_empty());
    }

    #[test]
    fn unicode_byte_offsets() {
        let s = "привет Мир мир";
        let m = find_matches(s, "мир");
        assert_eq!(m.len(), 2);
        assert_eq!(&s[m[0].clone()], "Мир");
        assert_eq!(&s[m[1].clone()], "мир");
        assert_eq!(find_matches("日本語 x", "x"), vec![10..11]);
    }

    #[test]
    fn segments_merge_spans_and_matches() {
        // "hello world": "hello " equal, "world" insert; match "lo wo", active.
        let spans = [
            WordDiffSpan { start: 0, end: 6, kind: WordDiffKind::Equal },
            WordDiffSpan { start: 6, end: 11, kind: WordDiffKind::Insert },
        ];
        let segs = segments(11, &spans, &[3..8], Some(0));
        let got: Vec<_> = segs
            .iter()
            .map(|s| (s.range.clone(), matches!(s.kind, WordDiffKind::Insert), s.matched, s.active))
            .collect();
        assert_eq!(
            got,
            vec![
                (0..3, false, false, false),
                (3..6, false, true, true),
                (6..8, true, true, true),
                (8..11, true, false, false),
            ]
        );
    }

    #[test]
    fn segments_without_word_diff() {
        assert!(segments(0, &[], &[], None).is_empty());
        let segs = segments(9, &[], &[0..3, 6..9], Some(1));
        let got: Vec<_> = segs.iter().map(|s| (s.range.clone(), s.matched, s.active)).collect();
        assert_eq!(got, vec![(0..3, true, false), (3..6, false, false), (6..9, true, true)]);
    }

    #[test]
    fn search_order_lookup_and_wrap() {
        let line = |c: &str| DiffLine {
            kind: DiffLineKind::Context,
            content: c.into(),
            old_lineno: None,
            new_lineno: None,
        };
        let file = |lines: Vec<DiffLine>| ProcessedFile {
            sha: String::new(),
            filename: String::new(),
            previous_filename: None,
            status: String::new(),
            additions: 0,
            deletions: 0,
            diff: Some(ProcessedDiff {
                hunks: vec![DiffHunk {
                    header: String::new(),
                    old_start: 0,
                    old_count: 0,
                    new_start: 0,
                    new_count: 0,
                    lines,
                }],
                side_by_side: vec![],
                word_diffs: vec![],
                is_binary: false,
                is_too_large: false,
            }),
        };
        let files = [file(vec![line("x x"), line("y")]), file(vec![line("X")])];
        let r = search_files(&files, "x");
        assert_eq!(r.len(), 3);
        assert_eq!((r[1].file, r[1].line, r[1].start), (0, 0, 2));
        assert_eq!((r[2].file, r[2].line), (1, 0));
        assert_eq!(line_matches(&r, 0, 0, 0), 0..2);
        assert_eq!(line_matches(&r, 0, 0, 1), 2..2);
        assert_eq!(line_matches(&r, 1, 0, 0), 2..3);

        assert_eq!(step(2, 3, true), 0);
        assert_eq!(step(0, 3, false), 2);
        assert_eq!(step(1, 3, false), 0);
        assert_eq!(step(0, 0, true), 0);
    }
}
