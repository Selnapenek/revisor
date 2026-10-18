use crate::provider::types::{ReviewComment, ThreadResolutionInfo};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct CommentThread {
    pub root: ReviewComment,
    pub replies: Vec<ReviewComment>,
    pub line: u64,
    pub side: Side,
    pub is_resolved: bool,
    pub thread_node_id: Option<String>,
}

/// Group review comments into inline threads, sorted by line. Replies whose
/// parent is missing become roots; file-level comments (no line) are dropped.
pub fn build_threads(
    comments: &[ReviewComment],
    resolutions: &[ThreadResolutionInfo],
) -> Vec<CommentThread> {
    let ids: HashSet<u64> = comments.iter().map(|c| c.id).collect();
    let mut replies: HashMap<u64, Vec<ReviewComment>> = HashMap::new();
    let mut roots = Vec::new();
    for c in comments {
        match c.in_reply_to_id.filter(|id| ids.contains(id)) {
            Some(parent) => replies.entry(parent).or_default().push(c.clone()),
            None => roots.push(c),
        }
    }

    let mut threads: Vec<CommentThread> = roots
        .into_iter()
        .filter_map(|root| {
            let side = if root.side.as_deref() == Some("LEFT") { Side::Left } else { Side::Right };
            let line = if side == Side::Left { root.original_line } else { root.line }?;
            let mut replies = replies.remove(&root.id).unwrap_or_default();
            replies.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            let res = resolutions.iter().find(|r| r.root_comment_id == root.id);
            Some(CommentThread {
                root: root.clone(),
                replies,
                line,
                side,
                is_resolved: res.is_some_and(|r| r.is_resolved),
                thread_node_id: res.map(|r| r.thread_node_id.clone()),
            })
        })
        .collect();
    threads.sort_by_key(|t| t.line);
    threads
}

/// Threads keyed by (path, side, line) for placement next to diff lines.
pub fn threads_by_line(threads: Vec<CommentThread>) -> HashMap<(String, Side, u64), Vec<CommentThread>> {
    let mut map: HashMap<_, Vec<_>> = HashMap::new();
    for t in threads {
        map.entry((t.root.path.clone(), t.side, t.line)).or_default().push(t);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: u64, reply_to: Option<u64>, line: Option<u64>, side: &str, at: &str) -> ReviewComment {
        ReviewComment {
            id,
            path: "a.rs".into(),
            line,
            original_line: line.map(|l| l + 100),
            side: Some(side.into()),
            diff_hunk: String::new(),
            body: String::new(),
            author_login: String::new(),
            author_avatar: String::new(),
            created_at: at.into(),
            updated_at: String::new(),
            in_reply_to_id: reply_to,
            review_id: None,
            html_url: String::new(),
            outdated: false,
        }
    }

    #[test]
    fn builds_threads() {
        let comments = [
            c(1, None, Some(20), "RIGHT", "t1"),
            c(2, Some(1), Some(20), "RIGHT", "t3"),
            c(3, Some(1), Some(20), "RIGHT", "t2"),
            c(4, Some(99), Some(5), "RIGHT", "t1"), // orphan reply -> root
            c(5, None, Some(1), "LEFT", "t1"),     // LEFT uses original_line (101)
            c(6, None, None, "RIGHT", "t1"),       // file-level, dropped
        ];
        let res = [ThreadResolutionInfo {
            root_comment_id: 1,
            thread_node_id: "T1".into(),
            is_resolved: true,
        }];
        let t = build_threads(&comments, &res);
        let ids: Vec<_> = t.iter().map(|t| (t.root.id, t.line)).collect();
        assert_eq!(ids, vec![(4, 5), (1, 20), (5, 101)]);
        let r: Vec<_> = t[1].replies.iter().map(|c| c.id).collect();
        assert_eq!(r, vec![3, 2]);
        assert!(t[1].is_resolved);
        assert_eq!(t[1].thread_node_id.as_deref(), Some("T1"));
        assert!(!t[0].is_resolved && t[0].thread_node_id.is_none());
        assert_eq!(t[2].side, Side::Left);

        let map = threads_by_line(t);
        assert_eq!(map[&("a.rs".into(), Side::Right, 20)].len(), 1);
        assert_eq!(map[&("a.rs".into(), Side::Left, 101)][0].root.id, 5);
        assert!(!map.contains_key(&("a.rs".into(), Side::Right, 101)));
    }
}
