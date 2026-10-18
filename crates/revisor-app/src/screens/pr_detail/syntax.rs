//! Syntax colors of diff lines, from the tree-sitter highlighter in `gpui-component`.

use gpui_kit::base::input::Rope;
use gpui_kit::component::highlighter::{HighlightTheme, LanguageRegistry, SyntaxHighlighter};
use gpui_kit::*;
use revisor_core::diff::parser::{DiffHunk, DiffLine, DiffLineKind};
use std::collections::HashMap;
use std::ops::Range;

pub type Styles = Vec<(Range<usize>, HighlightStyle)>;

/// Styles of a file's diff lines by line number: old side for removed lines, new side for the rest.
#[derive(Default)]
pub struct FileSyntax {
    old: HashMap<u64, Styles>,
    new: HashMap<u64, Styles>,
}

impl FileSyntax {
    // ponytail: parses hunks, not the whole file; a hunk that starts inside a block comment or string
    // colors wrong until expanded. Parse full old/new file contents if that matters.
    pub fn new(path: &str, hunks: &[DiffHunk], theme: &HighlightTheme) -> Self {
        let Some(lang) = language(path) else { return Self::default() };
        let lines = || hunks.iter().flat_map(|h| &h.lines);
        let side = |keep: DiffLineKind, number: fn(&DiffLine) -> Option<u64>| {
            highlight(&lang, lines().filter(|l| l.kind != keep).filter_map(|l| Some((number(l)?, &l.content))), theme)
        };
        Self {
            old: side(DiffLineKind::Added, |l| l.old_lineno),
            new: side(DiffLineKind::Removed, |l| l.new_lineno),
        }
    }

    pub fn line(&self, l: &DiffLine) -> &[(Range<usize>, HighlightStyle)] {
        let found = match l.kind {
            DiffLineKind::Removed => l.old_lineno.and_then(|n| self.old.get(&n)),
            _ => l.new_lineno.and_then(|n| self.new.get(&n)),
        };
        found.map_or(&[], Vec::as_slice)
    }
}

/// Registered language name for the file extension.
fn language(path: &str) -> Option<String> {
    let ext = path.rsplit_once('.')?.1.to_lowercase();
    let ext = match ext.as_str() {
        "jsx" | "mjs" | "cjs" => "javascript",
        "mts" | "cts" => "typescript",
        "h" => "c",
        "hpp" | "hh" | "cc" | "cxx" => "cpp",
        "htm" => "html",
        other => other,
    };
    // A language without a grammar gives an inert highlighter: no styles.
    LanguageRegistry::singleton().language(ext).map(|c| c.name.to_string())
}

/// Parse the lines as one text and cut the styles back into lines.
fn highlight<'a>(lang: &str, lines: impl Iterator<Item = (u64, &'a String)>, theme: &HighlightTheme) -> HashMap<u64, Styles> {
    let mut text = String::new();
    let mut spans = vec![];
    for (number, content) in lines {
        spans.push((number, text.len()..text.len() + content.len()));
        text.push_str(content);
        text.push('\n');
    }
    let mut highlighter = SyntaxHighlighter::new(lang);
    highlighter.update(None, &Rope::from(text.as_str()), None);
    spans
        .into_iter()
        .map(|(number, range)| {
            let start = range.start;
            let styles = highlighter
                .styles(&range, theme)
                .into_iter()
                .filter(|(r, s)| !r.is_empty() && *s != HighlightStyle::default())
                .map(|(r, s)| (r.start - start..r.end - start, s))
                .collect();
            (number, styles)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{DiffHunk, DiffLine, DiffLineKind, FileSyntax, HighlightTheme, Range};

    fn line(kind: DiffLineKind, old: Option<u64>, new: Option<u64>, content: &str) -> DiffLine {
        DiffLine { kind, content: content.into(), old_lineno: old, new_lineno: new }
    }

    #[test]
    fn colors_each_side() {
        let hunk = DiffHunk {
            header: String::new(),
            old_start: 1,
            old_count: 2,
            new_start: 1,
            new_count: 2,
            lines: vec![
                line(DiffLineKind::Context, Some(1), Some(1), "fn a() {"),
                line(DiffLineKind::Removed, Some(2), None, "    let s = 1;"),
                line(DiffLineKind::Added, None, Some(2), "    let s = \"x\";"),
            ],
        };
        let theme = HighlightTheme::default_dark();
        let syntax = FileSyntax::new("src/a.rs", std::slice::from_ref(&hunk), &theme);
        let colored = |l: &DiffLine, range: Range<usize>| syntax.line(l).iter().any(|(r, s)| *r == range && s.color.is_some());
        assert!(colored(&hunk.lines[0], 0..2), "fn keyword");
        assert!(colored(&hunk.lines[1], 4..7), "let keyword on the old side");
        assert!(colored(&hunk.lines[2], 12..15), "string on the new side");

        let plain = FileSyntax::new("notes.unknownext", std::slice::from_ref(&hunk), &theme);
        assert!(plain.line(&hunk.lines[0]).is_empty());
    }
}
