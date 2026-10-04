//! File list logic for the PR detail sidebar: glob filters, directory tree, navigation.

use crate::diff::ProcessedFile;
use std::cmp::Ordering;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tok {
    Lit(char),
    /// `?`: one non-slash char.
    One,
    /// `*`: any run of non-slash chars.
    Star,
    /// `**`: anything, slashes included.
    Any,
}

struct Glob {
    toks: Vec<Tok>,
    /// Patterns with `/` match the full path; others match any trailing segment(s).
    full: bool,
}

/// Comma-separated globs (`*`, `**`, `?`), case-insensitive.
/// Patterns without `/` match a path suffix starting at a segment boundary (`*.ts` matches `src/a.ts`).
pub struct GlobMatcher(Vec<Glob>);

impl GlobMatcher {
    /// `None` when `patterns` holds no non-empty pattern (caller decides the default).
    pub fn new(patterns: &str) -> Option<Self> {
        let globs: Vec<Glob> = patterns
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(|p| Glob { toks: tokenize(&p.to_lowercase()), full: p.contains('/') })
            .collect();
        (!globs.is_empty()).then_some(Self(globs))
    }

    pub fn is_match(&self, path: &str) -> bool {
        let path: Vec<char> = path.to_lowercase().chars().collect();
        self.0.iter().any(|g| {
            if g.full {
                return match_toks(&g.toks, &path);
            }
            (0..=path.len())
                .filter(|&i| i == 0 || path[i - 1] == '/')
                .any(|i| match_toks(&g.toks, &path[i..]))
        })
    }
}

fn tokenize(pattern: &str) -> Vec<Tok> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' if chars.get(i + 1) == Some(&'*') => {
                toks.push(Tok::Any);
                i += 2;
                // `**/` swallows its slash, so `**/x` also matches `x` at the root (and `ax`, as in the TS regex).
                if chars.get(i) == Some(&'/') {
                    i += 1;
                }
                continue;
            }
            '*' => toks.push(Tok::Star),
            '?' => toks.push(Tok::One),
            c => toks.push(Tok::Lit(c)),
        }
        i += 1;
    }
    toks
}

// ponytail: backtracking, exponential on many stars; fine for short user globs vs file paths.
fn match_toks(toks: &[Tok], s: &[char]) -> bool {
    match toks.split_first() {
        None => s.is_empty(),
        Some((Tok::Lit(c), rest)) => s.first() == Some(c) && match_toks(rest, &s[1..]),
        Some((Tok::One, rest)) => s.first().is_some_and(|&c| c != '/') && match_toks(rest, &s[1..]),
        Some((Tok::Star, rest)) => {
            let run = s.iter().take_while(|&&c| c != '/').count();
            (0..=run).any(|n| match_toks(rest, &s[n..]))
        }
        Some((Tok::Any, rest)) => (0..=s.len()).any(|n| match_toks(rest, &s[n..])),
    }
}

/// Sidebar filter: indexes of `files` matching case-insensitive substring `query` and the include / exclude glob fields.
pub fn filter_files(files: &[ProcessedFile], query: &str, include: &str, exclude: &str) -> Vec<usize> {
    let q = query.to_lowercase();
    let inc = GlobMatcher::new(include);
    let exc = GlobMatcher::new(exclude);
    (0..files.len())
        .filter(|&i| {
            let name = &files[i].filename;
            (q.is_empty() || name.to_lowercase().contains(&q))
                && inc.as_ref().is_none_or(|m| m.is_match(name))
                && exc.as_ref().is_none_or(|m| !m.is_match(name))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub enum TreeNode {
    Dir { name: String, path: String, children: Vec<TreeNode> },
    /// `index` is the file's position in the input of `build_file_tree`.
    File { name: String, path: String, index: usize },
}

impl TreeNode {
    pub fn name(&self) -> &str {
        match self {
            TreeNode::Dir { name, .. } | TreeNode::File { name, .. } => name,
        }
    }

    fn file_count(&self) -> usize {
        match self {
            TreeNode::File { .. } => 1,
            TreeNode::Dir { children, .. } => children.iter().map(TreeNode::file_count).sum(),
        }
    }
}

/// Directory tree from file paths. Single-child dir chains collapse into one node
/// (`client/src`); dirs sort before files, then by name.
pub fn build_file_tree<'a>(paths: impl IntoIterator<Item = &'a str>) -> Vec<TreeNode> {
    let mut root = Vec::new();
    for (index, path) in paths.into_iter().enumerate() {
        let parts: Vec<&str> = path.split('/').collect();
        let (file_name, dirs) = parts.split_last().unwrap();
        let mut current = &mut root;
        for i in 0..dirs.len() {
            let dir_path = parts[..=i].join("/");
            let pos = current
                .iter()
                .position(|c| matches!(c, TreeNode::Dir { path, .. } if *path == dir_path))
                .unwrap_or_else(|| {
                    current.push(TreeNode::Dir { name: dirs[i].to_string(), path: dir_path, children: Vec::new() });
                    current.len() - 1
                });
            let TreeNode::Dir { children, .. } = &mut current[pos] else { unreachable!() };
            current = children;
        }
        current.push(TreeNode::File { name: file_name.to_string(), path: path.to_string(), index });
    }
    collapse_and_sort(root)
}

fn collapse_and_sort(nodes: Vec<TreeNode>) -> Vec<TreeNode> {
    let mut nodes: Vec<TreeNode> = nodes
        .into_iter()
        .map(|node| {
            let TreeNode::Dir { mut name, mut path, children } = node else { return node };
            let mut children = collapse_and_sort(children);
            while let [TreeNode::Dir { .. }] = children.as_slice() {
                let Some(TreeNode::Dir { name: n, path: p, children: c }) = children.pop() else { unreachable!() };
                name = format!("{name}/{n}");
                path = p;
                children = c;
            }
            TreeNode::Dir { name, path, children }
        })
        .collect();
    nodes.sort_by(|a, b| match (a, b) {
        (TreeNode::Dir { .. }, TreeNode::File { .. }) => Ordering::Less,
        (TreeNode::File { .. }, TreeNode::Dir { .. }) => Ordering::Greater,
        _ => locale_cmp(a.name(), b.name()),
    });
    nodes
}

// ponytail: approximates JS localeCompare (case-insensitive, lowercase first on ties); ICU collation if punctuation order matters.
fn locale_cmp(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| b.cmp(a))
}

#[derive(Debug, Clone, PartialEq)]
pub enum RowKind {
    /// `files` counts all files below the dir (badge).
    Dir { files: usize, collapsed: bool },
    File { index: usize },
}

/// One line of the tree view, for a virtual list.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub depth: usize,
    pub name: String,
    pub path: String,
    pub kind: RowKind,
}

/// Flattens the tree into visible rows; children of dirs whose path is in `collapsed` are hidden.
pub fn visible_rows(tree: &[TreeNode], collapsed: &HashSet<String>) -> Vec<Row> {
    fn walk(nodes: &[TreeNode], depth: usize, collapsed: &HashSet<String>, out: &mut Vec<Row>) {
        for node in nodes {
            match node {
                TreeNode::File { name, path, index } => {
                    out.push(Row { depth, name: name.clone(), path: path.clone(), kind: RowKind::File { index: *index } })
                }
                TreeNode::Dir { name, path, children } => {
                    let is_collapsed = collapsed.contains(path);
                    out.push(Row {
                        depth,
                        name: name.clone(),
                        path: path.clone(),
                        kind: RowKind::Dir { files: node.file_count(), collapsed: is_collapsed },
                    });
                    if !is_collapsed {
                        walk(children, depth + 1, collapsed, out);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(tree, 0, collapsed, &mut out);
    out
}

/// `(reviewed, total)` over the PR's files.
pub fn review_progress(files: &[ProcessedFile], reviewed: &HashSet<String>) -> (usize, usize) {
    (files.iter().filter(|f| reviewed.contains(&f.filename)).count(), files.len())
}

/// Index of the next (`direction` 1) or previous (-1) file, wrapping around.
/// `current` is the selected file's index in the list, `None` if absent. `None` for an empty list.
pub fn navigate_file(len: usize, current: Option<usize>, direction: isize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let idx = current.map_or(-1, |i| i as isize);
    Some((idx + direction).rem_euclid(len as isize) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(patterns: &str, path: &str) -> bool {
        GlobMatcher::new(patterns).unwrap().is_match(path)
    }

    #[test]
    fn glob_empty_is_none() {
        assert!(GlobMatcher::new("").is_none());
        assert!(GlobMatcher::new(" , ,").is_none());
    }

    #[test]
    fn glob_star_and_extension() {
        assert!(m("*.ts", "src/foo.ts"));
        assert!(m("*.ts", "foo.ts"));
        assert!(!m("*.ts", "foo.tsx"));
        assert!(!m("*.ts", "foots"));
        assert!(m("*.TS", "src/Foo.ts"));
        assert!(m("foo*", "src/foobar.rs"));
        assert!(!m("bar", "src/foobar"));
    }

    #[test]
    fn glob_question() {
        assert!(m("a?.rs", "src/ab.rs"));
        assert!(!m("a?.rs", "src/abc.rs"));
        assert!(!m("src?x", "src/x"));
    }

    #[test]
    fn glob_with_slash_matches_full_path() {
        assert!(m("src/*.ts", "src/a.ts"));
        assert!(!m("src/*.ts", "src/lib/a.ts"));
        assert!(!m("src/*.ts", "app/src/a.ts"));
        assert!(m("src/**", "src/lib/a.ts"));
        assert!(m("src/**/*.ts", "src/lib/deep/a.ts"));
        assert!(m("src/**/*.ts", "src/a.ts"));
        assert!(m("**/test/*", "a/b/test/x.ts"));
        assert!(m("**/test/*", "test/x.ts"));
    }

    #[test]
    fn glob_segment_suffix() {
        // Slash-free pattern may span trailing segments through `**`.
        assert!(m("**.lock", "a/b/Cargo.lock"));
        assert!(m("node_modules", "a/node_modules"));
        assert!(!m("node_modules", "a/node_modules/x.js"));
    }

    #[test]
    fn glob_comma_list_and_literals() {
        assert!(m("*.rs, *.toml", "Cargo.toml"));
        assert!(m("*.rs,*.toml", "src/main.rs"));
        assert!(!m("*.rs,*.toml", "README.md"));
        assert!(m("a+(b).ts", "a+(b).ts"));
        assert!(!m("a.ts", "abts"));
    }

    fn pf(name: &str) -> ProcessedFile {
        ProcessedFile {
            sha: String::new(),
            filename: name.into(),
            status: "modified".into(),
            additions: 0,
            deletions: 0,
            diff: None,
        }
    }

    fn names(files: &[ProcessedFile], picked: &[usize]) -> Vec<String> {
        picked.iter().map(|&i| files[i].filename.clone()).collect()
    }

    #[test]
    fn filter_combos() {
        let files: Vec<_> =
            ["src/App.tsx", "src/lib/glob.ts", "src/lib/glob.test.ts", "README.md"].map(pf).into();
        assert_eq!(filter_files(&files, "", "", "").len(), 4);
        assert_eq!(names(&files, &filter_files(&files, "GLOB", "", "")), ["src/lib/glob.ts", "src/lib/glob.test.ts"]);
        assert_eq!(names(&files, &filter_files(&files, "", "*.ts,*.tsx", "*.test.ts")), ["src/App.tsx", "src/lib/glob.ts"]);
        assert_eq!(names(&files, &filter_files(&files, "lib", "", "*.test.ts")), ["src/lib/glob.ts"]);
        assert_eq!(names(&files, &filter_files(&files, "", " ", "src/**")), ["README.md"]);
    }

    #[test]
    fn tree_nesting_collapse_and_order() {
        let tree = build_file_tree(["b.ts", "client/src/x.ts", "client/src/a/y.ts", "A.ts", "lib/z.ts", "lib/q/w.ts"]);
        let rows = visible_rows(&tree, &HashSet::new());
        let got: Vec<(usize, &str, &str)> = rows.iter().map(|r| (r.depth, r.name.as_str(), r.path.as_str())).collect();
        assert_eq!(
            got,
            [
                (0, "client/src", "client/src"),
                (1, "a", "client/src/a"),
                (2, "y.ts", "client/src/a/y.ts"),
                (1, "x.ts", "client/src/x.ts"),
                (0, "lib", "lib"),
                (1, "q", "lib/q"),
                (2, "w.ts", "lib/q/w.ts"),
                (1, "z.ts", "lib/z.ts"),
                (0, "A.ts", "A.ts"),
                (0, "b.ts", "b.ts"),
            ]
        );
        assert_eq!(rows[0].kind, RowKind::Dir { files: 2, collapsed: false });
        assert_eq!(rows[2].kind, RowKind::File { index: 2 });
    }

    #[test]
    fn tree_deep_chain_collapses_to_parent_with_files() {
        let tree = build_file_tree(["a/b/c/d.ts", "a/e.ts"]);
        let rows = visible_rows(&tree, &HashSet::new());
        let got: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(got, ["a", "b/c", "d.ts", "e.ts"]);
        assert_eq!(rows[1].path, "a/b/c");
    }

    #[test]
    fn visible_rows_hides_collapsed() {
        let tree = build_file_tree(["src/a.ts", "src/b/c.ts", "x.ts"]);
        let collapsed = HashSet::from(["src".to_string()]);
        let rows = visible_rows(&tree, &collapsed);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, RowKind::Dir { files: 2, collapsed: true });
        assert_eq!(rows[1].path, "x.ts");
    }

    #[test]
    fn progress_counts_only_present_files() {
        let files: Vec<_> = ["a", "b", "c"].map(pf).into();
        let reviewed = HashSet::from(["a".to_string(), "gone".to_string()]);
        assert_eq!(review_progress(&files, &reviewed), (1, 3));
    }

    #[test]
    fn navigation_wraps() {
        assert_eq!(navigate_file(0, None, 1), None);
        assert_eq!(navigate_file(3, Some(0), 1), Some(1));
        assert_eq!(navigate_file(3, Some(2), 1), Some(0));
        assert_eq!(navigate_file(3, Some(0), -1), Some(2));
        assert_eq!(navigate_file(3, None, 1), Some(0));
        // Same as TS: prev with no selection lands on (len - 2).
        assert_eq!(navigate_file(3, None, -1), Some(1));
        assert_eq!(navigate_file(1, None, -1), Some(0));
    }
}
