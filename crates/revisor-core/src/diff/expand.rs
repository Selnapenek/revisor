//! Reveal unchanged lines hidden between diff hunks.
//!
//! `file_lines` is always the new-side (head) file content; hidden lines are
//! read by their new line number, and old line numbers are derived from the
//! neighbouring hunk. Render expanded hunks with `side_by_side::to_side_by_side`.

use super::parser::{DiffHunk, DiffLine, DiffLineKind};

/// Lines revealed per incremental expand click.
pub const EXPAND_LINES: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapPosition {
    Before,
    Between,
    After,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpandableGap {
    pub position: GapPosition,
    /// Hunk below the gap (`Before`/`Between`) or the last hunk (`After`).
    pub hunk_index: usize,
    /// Hidden line count; `None` while the file length is unknown.
    pub line_count: Option<u64>,
}

/// First line and one-past-last line of a hunk side. An empty side's start
/// names the line before it (`@@ -0,0 +1,3 @@`).
fn range(start: u64, count: u64) -> (u64, u64) {
    let first = if count == 0 { start + 1 } else { start };
    (first, first + count)
}

fn old_range(h: &DiffHunk) -> (u64, u64) {
    range(h.old_start, h.old_count)
}

fn new_range(h: &DiffHunk) -> (u64, u64) {
    range(h.new_start, h.new_count)
}

/// All expandable gaps around `hunks`. `total_lines` is the new file's line count once fetched.
pub fn compute_gaps(hunks: &[DiffHunk], total_lines: Option<usize>) -> Vec<ExpandableGap> {
    let (Some(first), Some(last)) = (hunks.first(), hunks.last()) else {
        return vec![];
    };
    let gap = |position, hunk_index, line_count| ExpandableGap {
        position,
        hunk_index,
        line_count,
    };
    let mut gaps = vec![];

    let before = new_range(first).0 - 1;
    if before > 0 {
        gaps.push(gap(GapPosition::Before, 0, Some(before)));
    }
    for (i, pair) in hunks.windows(2).enumerate() {
        let hidden = new_range(&pair[1]).0.saturating_sub(new_range(&pair[0]).1);
        if hidden > 0 {
            gaps.push(gap(GapPosition::Between, i + 1, Some(hidden)));
        }
    }
    // A deleted file (`+0,0`) has nothing after it.
    if last.new_start > 0 {
        let shown = new_range(last).1 - 1;
        match total_lines {
            Some(total) if total as u64 > shown => {
                gaps.push(gap(GapPosition::After, hunks.len() - 1, Some(total as u64 - shown)))
            }
            Some(_) => {}
            None => gaps.push(gap(GapPosition::After, hunks.len() - 1, None)),
        }
    }
    gaps
}

/// Context lines for new lines `from..=to` (clamped to the file), old numbering starting at `old_from`.
fn context(file_lines: &[impl AsRef<str>], from: u64, to: u64, old_from: u64) -> Vec<DiffLine> {
    (from..=to.min(file_lines.len() as u64))
        .map(|n| DiffLine {
            kind: DiffLineKind::Context,
            content: file_lines[n as usize - 1].as_ref().to_string(),
            old_lineno: Some(old_from + n - from),
            new_lineno: Some(n),
        })
        .collect()
}

/// Reveal up to `count` hidden lines above hunk `idx` (`usize::MAX` for all).
pub fn expand_up(hunks: &mut [DiffHunk], idx: usize, file_lines: &[impl AsRef<str>], count: usize) {
    let (old_first, _) = old_range(&hunks[idx]);
    let (new_first, _) = new_range(&hunks[idx]);
    let floor = if idx == 0 { 1 } else { new_range(&hunks[idx - 1]).1 };
    let from = new_first.saturating_sub(count as u64).max(floor);
    let old_from = old_first - (new_first - from);
    let lines = context(file_lines, from, new_first - 1, old_from);
    if lines.is_empty() {
        return;
    }
    let h = &mut hunks[idx];
    h.old_start = old_from;
    h.new_start = from;
    h.old_count += lines.len() as u64;
    h.new_count += lines.len() as u64;
    h.lines.splice(0..0, lines);
}

/// Reveal up to `count` hidden lines below hunk `idx` (`usize::MAX` for all).
pub fn expand_down(hunks: &mut [DiffHunk], idx: usize, file_lines: &[impl AsRef<str>], count: usize) {
    let (old_first, old_end) = old_range(&hunks[idx]);
    let (new_first, new_end) = new_range(&hunks[idx]);
    let ceil = hunks
        .get(idx + 1)
        .map_or(file_lines.len() as u64, |next| new_range(next).0 - 1);
    let to = ceil.min(new_end.saturating_add(count as u64) - 1);
    let lines = context(file_lines, new_end, to, old_end);
    if lines.is_empty() {
        return;
    }
    let h = &mut hunks[idx];
    h.old_start = old_first;
    h.new_start = new_first;
    h.old_count += lines.len() as u64;
    h.new_count += lines.len() as u64;
    h.lines.extend(lines);
}

/// "Expand all" for `gap`: fills it completely; a `Between` gap merges its two hunks.
pub fn expand_all(hunks: &mut Vec<DiffHunk>, gap: &ExpandableGap, file_lines: &[impl AsRef<str>]) {
    let idx = gap.hunk_index;
    match gap.position {
        GapPosition::Before => expand_up(hunks, idx, file_lines, usize::MAX),
        GapPosition::After => expand_down(hunks, idx, file_lines, usize::MAX),
        GapPosition::Between => {
            if idx == 0 || idx >= hunks.len() {
                return;
            }
            expand_down(hunks, idx - 1, file_lines, usize::MAX);
            let next = hunks.remove(idx);
            let prev = &mut hunks[idx - 1];
            let (old_first, _) = old_range(prev);
            let (new_first, _) = new_range(prev);
            prev.old_start = old_first;
            prev.new_start = new_first;
            prev.old_count = old_range(&next).1 - old_first;
            prev.new_count = new_range(&next).1 - new_first;
            prev.lines.extend(next.lines);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{parser::parse_patch, side_by_side::to_side_by_side};

    /// 40-line head file "L1".."L40". Hunk 1 adds one line after old line 5,
    /// hunk 2 changes old line 30 (new line 31).
    fn setup() -> (Vec<DiffHunk>, Vec<String>) {
        let patch = "@@ -4,4 +4,5 @@\n L4\n L5\n+NEW\n L6\n L7\n@@ -28,5 +29,5 @@\n L28\n L29\n-OLD30\n+L30\n L31\n L32";
        let mut file: Vec<String> = (1..=40).map(|n| format!("L{n}")).collect();
        file.insert(5, "NEW".into());
        file.remove(40);
        (parse_patch(patch).hunks, file)
    }

    fn nums(h: &DiffHunk) -> (u64, u64, u64, u64) {
        (h.old_start, h.old_count, h.new_start, h.new_count)
    }

    #[test]
    fn gaps() {
        let (hunks, file) = setup();
        let g = compute_gaps(&hunks, None);
        assert_eq!(g[0], ExpandableGap { position: GapPosition::Before, hunk_index: 0, line_count: Some(3) });
        assert_eq!(g[1], ExpandableGap { position: GapPosition::Between, hunk_index: 1, line_count: Some(20) });
        assert_eq!(g[2], ExpandableGap { position: GapPosition::After, hunk_index: 1, line_count: None });
        assert_eq!(compute_gaps(&hunks, Some(file.len()))[2].line_count, Some(7));
        assert_eq!(compute_gaps(&hunks, Some(33)).len(), 2);
        assert!(compute_gaps(&[], None).is_empty());
    }

    #[test]
    fn new_and_deleted_files() {
        let added = parse_patch("@@ -0,0 +1,2 @@\n+a\n+b").hunks;
        assert_eq!(compute_gaps(&added, Some(2)), vec![]);
        let deleted = parse_patch("@@ -1,2 +0,0 @@\n-a\n-b").hunks;
        assert_eq!(compute_gaps(&deleted, None), vec![]);
    }

    #[test]
    fn expand_up_reads_new_side_with_old_numbering() {
        let (mut hunks, file) = setup();
        expand_up(&mut hunks, 1, &file, 2);
        assert_eq!(nums(&hunks[1]), (26, 7, 27, 7));
        let l = &hunks[1].lines[0];
        assert_eq!((l.content.as_str(), l.old_lineno, l.new_lineno), ("L26", Some(26), Some(27)));
        assert_eq!(compute_gaps(&hunks, None)[1].line_count, Some(18));
    }

    #[test]
    fn expand_up_stops_at_file_start_and_previous_hunk() {
        let (mut hunks, file) = setup();
        expand_up(&mut hunks, 0, &file, EXPAND_LINES);
        assert_eq!(nums(&hunks[0]), (1, 7, 1, 8));
        assert_eq!(hunks[0].lines[0].content, "L1");
        expand_up(&mut hunks, 0, &file, 5);
        assert_eq!(hunks[0].lines.len(), 8);

        expand_up(&mut hunks, 1, &file, 100);
        assert_eq!(nums(&hunks[1]), (8, 25, 9, 25));
        assert_eq!(hunks[1].lines[0].content, "L8");
        assert!(compute_gaps(&hunks, None).iter().all(|g| g.position == GapPosition::After));
    }

    #[test]
    fn expand_down_after_hunk_and_at_file_end() {
        let (mut hunks, file) = setup();
        expand_down(&mut hunks, 0, &file, 3);
        assert_eq!(nums(&hunks[0]), (4, 7, 4, 8));
        let l = hunks[0].lines.last().unwrap();
        assert_eq!((l.content.as_str(), l.old_lineno, l.new_lineno), ("L10", Some(10), Some(11)));

        expand_down(&mut hunks, 1, &file, EXPAND_LINES);
        assert_eq!(nums(&hunks[1]), (28, 12, 29, 12));
        let l = hunks[1].lines.last().unwrap();
        assert_eq!((l.content.as_str(), l.old_lineno, l.new_lineno), ("L39", Some(39), Some(40)));
        let before = hunks[1].lines.len();
        expand_down(&mut hunks, 1, &file, EXPAND_LINES);
        assert_eq!(hunks[1].lines.len(), before);
        assert_eq!(compute_gaps(&hunks, Some(file.len())).last().unwrap().position, GapPosition::Between);
    }

    #[test]
    fn expand_all_merges_between_and_fills_edges() {
        let (mut hunks, file) = setup();
        let gaps = compute_gaps(&hunks, Some(file.len()));
        expand_all(&mut hunks, &gaps[1], &file);
        assert_eq!(hunks.len(), 1);
        assert_eq!(nums(&hunks[0]), (4, 29, 4, 30));
        assert_eq!(hunks[0].lines.len(), 31);
        expand_all(&mut hunks, &gaps[0], &file);
        let gap = *compute_gaps(&hunks, Some(file.len())).last().unwrap();
        expand_all(&mut hunks, &gap, &file);
        assert_eq!(nums(&hunks[0]), (1, 39, 1, 40));
        assert!(compute_gaps(&hunks, Some(file.len())).is_empty());
        let new_side: Vec<_> = hunks[0].lines.iter().filter(|l| l.new_lineno.is_some()).map(|l| l.content.clone()).collect();
        assert_eq!(new_side, file);
    }

    #[test]
    fn expand_all_merges_adjacent_hunks_without_gap() {
        let (mut hunks, file) = setup();
        expand_down(&mut hunks, 0, &file, EXPAND_LINES);
        expand_all(&mut hunks, &ExpandableGap { position: GapPosition::Between, hunk_index: 1, line_count: Some(0) }, &file);
        assert_eq!(hunks.len(), 1);
        assert_eq!(nums(&hunks[0]), (4, 29, 4, 30));
    }

    #[test]
    fn side_by_side_of_expanded_hunks() {
        let (mut hunks, file) = setup();
        expand_down(&mut hunks, 0, &file, 1);
        let rows = to_side_by_side(&hunks);
        let added = rows.iter().find(|r| r.right.as_ref().is_some_and(|l| l.content == "NEW")).unwrap();
        assert!(added.left.is_none());
        let expanded = &rows[6];
        assert_eq!(expanded.left.as_ref().unwrap().content, "L8");
        assert_eq!(expanded.right.as_ref().unwrap().new_lineno, Some(9));
        assert!(rows[7].is_hunk_header);
    }
}
