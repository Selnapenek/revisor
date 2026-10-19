//! File list logic for the PR detail sidebar: glob filters, directory tree, navigation.

use crate::diff::{parser::DiffLineKind, ProcessedFile};
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

/// Lock files and generated code, which reviewers rarely read line by line; they open collapsed.
const GENERATED: &str = "*.lock, *.lockfile, *.lockb, package-lock.json, npm-shrinkwrap.json, pnpm-lock.yaml, go.sum, \
    *.min.js, *.min.css, *.map, *.pb.go, *_pb2.py, *.g.dart, *.generated.*, **/__generated__/**";

pub fn is_generated(path: &str) -> bool {
    // ponytail: fixed list, re-parsed per call; read `.gitattributes` linguist-generated if users need custom rules.
    GlobMatcher::new(GENERATED).is_some_and(|m| m.is_match(path))
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
/// (`client/src`); dirs sort before files, then by name. `keep_order` keeps the input order
/// instead: a file joins a dir only when the file before it is in that dir too, so a dir can show more than once.
pub fn build_file_tree<'a>(paths: impl IntoIterator<Item = &'a str>, keep_order: bool) -> Vec<TreeNode> {
    let mut root = Vec::new();
    for (index, path) in paths.into_iter().enumerate() {
        let parts: Vec<&str> = path.split('/').collect();
        let (file_name, dirs) = parts.split_last().unwrap();
        let mut current = &mut root;
        for i in 0..dirs.len() {
            let dir_path = parts[..=i].join("/");
            let pos = current
                .iter()
                .rposition(|c| matches!(c, TreeNode::Dir { path, .. } if *path == dir_path))
                .filter(|&pos| !keep_order || pos + 1 == current.len())
                .unwrap_or_else(|| {
                    current.push(TreeNode::Dir { name: dirs[i].to_string(), path: dir_path, children: Vec::new() });
                    current.len() - 1
                });
            let TreeNode::Dir { children, .. } = &mut current[pos] else { unreachable!() };
            current = children;
        }
        current.push(TreeNode::File { name: file_name.to_string(), path: path.to_string(), index });
    }
    collapse_and_sort(root, !keep_order)
}

fn collapse_and_sort(nodes: Vec<TreeNode>, sort: bool) -> Vec<TreeNode> {
    let mut nodes: Vec<TreeNode> = nodes
        .into_iter()
        .map(|node| {
            let TreeNode::Dir { mut name, mut path, children } = node else { return node };
            let mut children = collapse_and_sort(children, sort);
            while let [TreeNode::Dir { .. }] = children.as_slice() {
                let Some(TreeNode::Dir { name: n, path: p, children: c }) = children.pop() else { unreachable!() };
                name = format!("{name}/{n}");
                path = p;
                children = c;
            }
            TreeNode::Dir { name, path, children }
        })
        .collect();
    if !sort {
        return nodes;
    }
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

/// Next (`forward`) or previous index in `0..len` where `hit` is true, wrapping around.
/// Searches from `current` (excluded), or from the start/end when `None`. `current` is returned
/// only when it is the sole hit.
pub fn find_wrapping(len: usize, current: Option<usize>, forward: bool, hit: impl Fn(usize) -> bool) -> Option<usize> {
    let start = match (current, forward) {
        (Some(c), _) => c.min(len.saturating_sub(1)),
        (None, true) => len.saturating_sub(1),
        (None, false) => 0,
    };
    (1..=len)
        .map(|step| if forward { (start + step) % len } else { (start + len - step % len) % len })
        .find(|&i| hit(i))
}

/// How a changed file relates to the file under review. Sorts strongest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Relation {
    /// A test of the current file.
    Test,
    /// The code the current test file tests.
    Source,
    /// Same base name: `foo.h` / `foo.c`, `Card.tsx` / `Card.css`.
    SameName,
    /// Either file's changed lines name the other.
    Mentions,
    SameFolder,
}

/// Names too common to relate files by (`mod.rs`, `index.ts`); they relate within one folder only.
const GENERIC_STEMS: &[&str] = &[
    "index", "mod", "lib", "main", "init", "__init__", "utils", "util", "types", "app", "test", "tests", "spec",
    "readme", "package", "cargo", "config", "setup", "style", "styles", "constants",
];

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Lowercase base name without extensions and test affixes, and whether the path is a test.
fn name_parts(path: &str) -> (String, bool) {
    let name = path.rsplit('/').next().unwrap_or(path);
    let in_test_dir = parent(path).split('/').any(|d| matches!(d, "test" | "tests" | "__tests__" | "spec" | "specs"));
    // `glob.test.ts`, `card.spec.tsx`
    let test_ext = name.split('.').skip(1).any(|p| p == "test" || p == "spec");
    // A leading dot is the name itself (`.eslintrc`).
    let base = match name.find('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    };
    let stripped = ["_test", "_tests", "_spec", "-test", "-spec", "Test", "Tests", "Spec"]
        .iter()
        .find_map(|s| base.strip_suffix(s))
        .or_else(|| base.strip_prefix("test_"))
        .filter(|s| !s.is_empty());
    (stripped.unwrap_or(base).to_lowercase(), in_test_dir || test_ext || stripped.is_some())
}

/// `stem` is a whole word in an added or removed line of `file`, case-insensitively.
fn mentions(file: &ProcessedFile, stem: &str) -> bool {
    if stem.len() < 3 || GENERIC_STEMS.contains(&stem) {
        return false;
    }
    let word = |c: char| c.is_alphanumeric() || c == '_';
    file.diff.iter().flat_map(|d| &d.hunks).flat_map(|h| &h.lines).filter(|l| l.kind != DiffLineKind::Context).any(|l| {
        let line = l.content.to_lowercase();
        line.match_indices(stem).any(|(i, _)| {
            !line[..i].chars().next_back().is_some_and(word) && !line[i + stem.len()..].chars().next().is_some_and(word)
        })
    })
}

/// Changed files related to `files[current]` by name, folder or mention in changed lines, strongest first.
/// Unrelated non-test files are left out.
// ponytail: names and changed lines only, no repository index; add symbol analysis if names miss real relations.
pub fn related_files(files: &[ProcessedFile], current: usize) -> Vec<(usize, Relation)> {
    let Some(cur) = files.get(current) else { return vec![] };
    let (stem, is_test) = name_parts(&cur.filename);
    let dir = parent(&cur.filename);
    let mut out: Vec<(usize, Relation)> = files
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != current)
        .filter_map(|(i, f)| {
            let (s, test) = name_parts(&f.filename);
            let same_dir = parent(&f.filename) == dir;
            let same_stem = s == stem && (same_dir || !GENERIC_STEMS.contains(&s.as_str()));
            let relation = match () {
                _ if same_stem && test && !is_test => Relation::Test,
                _ if same_stem && is_test && !test => Relation::Source,
                _ if same_stem => Relation::SameName,
                _ if mentions(f, &stem) || mentions(cur, &s) => Relation::Mentions,
                _ if same_dir => Relation::SameFolder,
                _ => return None,
            };
            Some((i, relation))
        })
        .collect();
    out.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| files[a.0].filename.cmp(&files[b.0].filename)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_parts_strip_test_affixes() {
        assert_eq!(name_parts("src/lib/glob.test.ts"), ("glob".into(), true));
        assert_eq!(name_parts("pkg/foo_test.go"), ("foo".into(), true));
        assert_eq!(name_parts("tests/test_foo.py"), ("foo".into(), true));
        assert_eq!(name_parts("src/UserCardTest.java"), ("usercard".into(), true));
        assert_eq!(name_parts("crates/x/tests/parser.rs"), ("parser".into(), true));
        assert_eq!(name_parts("src/latest.ts"), ("latest".into(), false));
        assert_eq!(name_parts(".eslintrc"), (".eslintrc".into(), false));
    }

    #[test]
    fn related_files_by_name_mention_and_folder() {
        let mut files: Vec<_> = [
            "src/glob.ts",
            "src/glob.test.ts",
            "src/other.ts",
            "lib/caller.ts",
            "tests/unrelated.test.ts",
            "docs/readme.md",
            "a/mod.rs",
            "b/mod.rs",
        ]
        .map(pf)
        .into();
        files[3].diff = Some(crate::diff::diff_contents("", "import { glob } from '../src/glob';\nconst globby = 1;"));
        let rel = |i| related_files(&files, i).into_iter().map(|(j, r)| (files[j].filename.as_str(), r)).collect::<Vec<_>>();
        assert_eq!(
            rel(0),
            [
                ("src/glob.test.ts", Relation::Test),
                ("lib/caller.ts", Relation::Mentions),
                ("src/other.ts", Relation::SameFolder),
            ]
        );
        assert_eq!(rel(1)[0], ("src/glob.ts", Relation::Source));
        assert!(rel(0).iter().all(|(p, _)| *p != "tests/unrelated.test.ts"));
        // Generic names in different folders are not related.
        assert!(rel(6).iter().all(|(p, _)| *p != "b/mod.rs"));
        assert!(related_files(&files, 99).is_empty());
        assert!(related_files(&[], 0).is_empty());
    }

    #[test]
    fn related_files_have_stable_order_and_whole_word_mentions() {
        let mut files: Vec<_> = ["src/widget.ts", "tests/widget.test.ts", "tests/widget.spec.ts", "b/uses.ts", "a/uses.ts", "c/near.ts"]
            .map(pf)
            .into();
        files[3].diff = Some(crate::diff::diff_contents("", "widget()"));
        files[4].diff = Some(crate::diff::diff_contents("", "widget()"));
        files[5].diff = Some(crate::diff::diff_contents("", "widgetExtra()"));
        let paths: Vec<_> = related_files(&files, 0).into_iter().map(|(i, _)| files[i].filename.as_str()).collect();
        assert_eq!(paths, ["tests/widget.spec.ts", "tests/widget.test.ts", "a/uses.ts", "b/uses.ts"]);
    }

    #[test]
    fn related_files_rank_matching_stems_before_mentions() {
        let mut files: Vec<_> = ["src/card.tsx", "styles/card.css", "src/mentions.ts", "other/card.png"]
            .map(pf)
            .into();
        files[2].diff = Some(crate::diff::diff_contents("", "card"));
        let matches = related_files(&files, 0);
        assert_eq!(matches, [(3, Relation::SameName), (1, Relation::SameName), (2, Relation::Mentions)]);
    }

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

    #[test]
    fn generated_files() {
        for path in ["Cargo.lock", "web/yarn.lock", "package-lock.json", "a/pnpm-lock.yaml", "go.sum", "dist/app.min.js", "api/x.pb.go", "src/__generated__/types.ts", "schema.generated.ts"] {
            assert!(is_generated(path), "{path}");
        }
        for path in ["src/lock.rs", "Cargo.toml", "package.json", "src/generated.rs", "src/map.ts"] {
            assert!(!is_generated(path), "{path}");
        }
    }

    fn pf(name: &str) -> ProcessedFile {
        ProcessedFile {
            sha: String::new(),
            filename: name.into(),
            previous_filename: None,
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
        let tree = build_file_tree(["b.ts", "client/src/x.ts", "client/src/a/y.ts", "A.ts", "lib/z.ts", "lib/q/w.ts"], false);
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
    fn tree_in_input_order_groups_only_neighbours() {
        let tree = build_file_tree(["src/b.ts", "src/a.ts", "x.ts", "src/c.ts"], true);
        let rows = visible_rows(&tree, &HashSet::new());
        let got: Vec<(usize, &str)> = rows.iter().map(|r| (r.depth, r.name.as_str())).collect();
        assert_eq!(got, [(0, "src"), (1, "b.ts"), (1, "a.ts"), (0, "x.ts"), (0, "src"), (1, "c.ts")]);
        assert_eq!(rows[5].kind, RowKind::File { index: 3 });
    }

    #[test]
    fn tree_deep_chain_collapses_to_parent_with_files() {
        let tree = build_file_tree(["a/b/c/d.ts", "a/e.ts"], false);
        let rows = visible_rows(&tree, &HashSet::new());
        let got: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(got, ["a", "b/c", "d.ts", "e.ts"]);
        assert_eq!(rows[1].path, "a/b/c");
    }

    #[test]
    fn visible_rows_hides_collapsed() {
        let tree = build_file_tree(["src/a.ts", "src/b/c.ts", "x.ts"], false);
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

    #[test]
    fn find_wrapping_steps_and_wraps() {
        let hits = [false, true, false, true, false];
        let hit = |i: usize| hits[i];
        assert_eq!(find_wrapping(5, Some(1), true, hit), Some(3));
        assert_eq!(find_wrapping(5, Some(3), true, hit), Some(1));
        assert_eq!(find_wrapping(5, Some(3), false, hit), Some(1));
        assert_eq!(find_wrapping(5, Some(1), false, hit), Some(3));
        assert_eq!(find_wrapping(5, Some(2), true, hit), Some(3));
        assert_eq!(find_wrapping(5, None, true, hit), Some(1));
        assert_eq!(find_wrapping(5, None, false, hit), Some(3));
        // The only hit: stay on it.
        assert_eq!(find_wrapping(3, Some(1), true, |i| i == 1), Some(1));
        assert_eq!(find_wrapping(3, Some(0), true, |_| false), None);
        assert_eq!(find_wrapping(0, None, true, |_| true), None);
        assert_eq!(find_wrapping(0, Some(4), false, |_| true), None);
    }
}
