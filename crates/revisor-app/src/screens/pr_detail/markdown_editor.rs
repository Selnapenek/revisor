//! GitHub-style markdown editor: Write / Preview tabs and a formatting toolbar over a textarea.

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    input::{Textarea, TextareaState},
    tab::{Tab, TabBar},
    text::TextView,
    *,
};
use gpui_kit::{prelude::*, *};
use std::ops::Range;

actions!(markdown_editor, [Bold, Italic, Code, Link]);

const CONTEXT: &str = "MarkdownEditor";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-b", Bold, Some(CONTEXT)),
        KeyBinding::new("cmd-i", Italic, Some(CONTEXT)),
        KeyBinding::new("cmd-e", Code, Some(CONTEXT)),
        KeyBinding::new("cmd-k", Link, Some(CONTEXT)),
    ]);
}

#[derive(Clone, Copy, Debug)]
enum Format {
    Heading,
    Bold,
    Italic,
    Quote,
    Code,
    Link,
    List,
    Ordered,
    Task,
}

const TOOLBAR: [(Format, IconName, &str); 9] = [
    (Format::Heading, IconName::Heading, "Heading"),
    (Format::Bold, IconName::Bold, "Bold (⌘B)"),
    (Format::Italic, IconName::Italic, "Italic (⌘I)"),
    (Format::Quote, IconName::TextQuote, "Quote"),
    (Format::Code, IconName::Code, "Code (⌘E)"),
    (Format::Link, IconName::Link, "Link (⌘K)"),
    (Format::List, IconName::List, "Unordered list"),
    (Format::Ordered, IconName::ListOrdered, "Numbered list"),
    (Format::Task, IconName::ListChecks, "Task list"),
];

/// Markdown edit for `kind` over byte selection `sel`: (span to replace, replacement, new selection in the new text).
fn format(text: &str, sel: Range<usize>, kind: Format) -> (Range<usize>, String, Range<usize>) {
    let selected = &text[sel.clone()];
    let wrap = match kind {
        Format::Bold => Some(("**", "**")),
        Format::Italic => Some(("_", "_")),
        Format::Code if selected.contains('\n') => Some(("```\n", "\n```")),
        Format::Code => Some(("`", "`")),
        Format::Link => Some(("[", "](url)")),
        _ => None,
    };
    if let Some((pre, post)) = wrap {
        let start = sel.start + pre.len();
        let new_sel = match kind {
            // Select the `url` placeholder once there is link text.
            Format::Link if !selected.is_empty() => start + selected.len() + 2..start + selected.len() + 5,
            _ => start..start + selected.len(),
        };
        return (sel, format!("{pre}{selected}{post}"), new_sel);
    }

    // Line prefixes apply to every line the selection touches; a trailing newline does not pull in the next line.
    let end = if sel.end > sel.start && text[..sel.end].ends_with('\n') { sel.end - 1 } else { sel.end };
    let line_start = text[..sel.start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i);
    let prefix = |i: usize| match kind {
        Format::Heading => "### ".to_string(),
        Format::Quote => "> ".to_string(),
        Format::Ordered => format!("{}. ", i + 1),
        Format::Task => "- [ ] ".to_string(),
        _ => "- ".to_string(),
    };
    let new: String = text[line_start..line_end]
        .split('\n')
        .enumerate()
        .map(|(i, line)| prefix(i) + line)
        .collect::<Vec<_>>()
        .join("\n");
    let new_sel = if sel.is_empty() {
        let cursor = sel.start + prefix(0).len();
        cursor..cursor
    } else {
        line_start..line_start + new.len()
    };
    (line_start..line_end, new, new_sel)
}

pub struct MarkdownEditor {
    pub input: Entity<TextareaState>,
    preview: bool,
    disabled: bool,
}

impl MarkdownEditor {
    pub fn new(
        initial: &str,
        placeholder: &'static str,
        (min_rows, max_rows): (usize, usize),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial = initial.to_string();
        let input = cx.new(|cx| {
            TextareaState::new(window, cx).auto_grow(min_rows, max_rows).placeholder(placeholder).default_value(initial)
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        Self { input, preview: false, disabled: false }
    }

    pub fn value(&self, cx: &App) -> String {
        self.input.read(cx).value().trim().to_string()
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    fn set_preview(&mut self, preview: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.preview = preview;
        if !preview {
            self.input.update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }

    fn apply(&mut self, kind: Format, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.preview {
            return;
        }
        self.input.update(cx, |input, cx| {
            let (span, new, sel) = format(&input.value(), input.selected_range(), kind);
            input.set_selected_range(span, cx);
            input.replace(new, window, cx);
            input.set_selected_range(sel, cx);
            input.focus(window, cx);
        });
    }
}

impl Render for MarkdownEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let body = self.value(cx);
        v_flex()
            .gap_2()
            .key_context(CONTEXT)
            .on_action(cx.listener(|this, _: &Bold, window, cx| this.apply(Format::Bold, window, cx)))
            .on_action(cx.listener(|this, _: &Italic, window, cx| this.apply(Format::Italic, window, cx)))
            .on_action(cx.listener(|this, _: &Code, window, cx| this.apply(Format::Code, window, cx)))
            .on_action(cx.listener(|this, _: &Link, window, cx| this.apply(Format::Link, window, cx)))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        TabBar::new("md-tabs")
                            .underline()
                            .small()
                            .selected_index(self.preview as usize)
                            .on_click(cx.listener(|this, ix: &usize, window, cx| this.set_preview(*ix == 1, window, cx)))
                            .child(Tab::new().label("Write"))
                            .child(Tab::new().label("Preview")),
                    )
                    .child(div().flex_1())
                    .when(!self.preview, |el| {
                        el.children(TOOLBAR.iter().enumerate().map(|(i, (kind, icon, tip))| {
                            let kind = *kind;
                            Button::new(("md-format", i))
                                .ghost()
                                .xsmall()
                                .icon(icon.clone())
                                .tooltip(*tip)
                                .disabled(self.disabled)
                                .on_click(cx.listener(move |this, _, window, cx| this.apply(kind, window, cx)))
                        }))
                    }),
            )
            .map(|el| {
                if !self.preview {
                    el.child(Textarea::new(&self.input).small().disabled(self.disabled))
                } else if body.is_empty() {
                    el.child(div().py_2().text_xs().text_color(theme.muted_foreground).child("Nothing to preview"))
                } else {
                    el.child(div().text_xs().child(TextView::markdown(("md-preview", cx.entity_id().as_u64()), body)))
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{format, Format};
    use std::ops::Range;

    /// Apply `format` and return the new text plus the newly selected slice.
    fn run(text: &str, sel: Range<usize>, kind: Format) -> (String, String) {
        let (span, new, new_sel) = format(text, sel, kind);
        let mut out = text.to_string();
        out.replace_range(span, &new);
        let picked = out[new_sel].to_string();
        (out, picked)
    }

    #[test]
    fn wraps_selection_and_keeps_it_selected() {
        assert_eq!(run("say hi now", 4..6, Format::Bold), ("say **hi** now".into(), "hi".into()));
        assert_eq!(run("hi", 0..2, Format::Italic), ("_hi_".into(), "hi".into()));
        assert_eq!(run("hi", 0..2, Format::Code), ("`hi`".into(), "hi".into()));
    }

    #[test]
    fn empty_selection_puts_cursor_between_markers() {
        let (span, new, sel) = format("ab", 1..1, Format::Bold);
        assert_eq!((span, new.as_str(), sel), (1..1, "****", 3..3));
    }

    #[test]
    fn multi_line_code_becomes_fence() {
        assert_eq!(run("a\nb", 0..3, Format::Code).0, "```\na\nb\n```");
    }

    #[test]
    fn link_selects_url_placeholder() {
        assert_eq!(run("docs", 0..4, Format::Link), ("[docs](url)".into(), "url".into()));
        let (_, new, sel) = format("", 0..0, Format::Link);
        assert_eq!((new.as_str(), sel), ("[](url)", 1..1));
    }

    #[test]
    fn prefixes_every_touched_line() {
        let text = "one\ntwo\nthree";
        assert_eq!(run(text, 5..10, Format::Ordered).0, "one\n1. two\n2. three");
        assert_eq!(run(text, 0..13, Format::Ordered).0, "1. one\n2. two\n3. three");
        assert_eq!(run(text, 0..4, Format::Quote).0, "> one\ntwo\nthree");
        assert_eq!(run(text, 9..9, Format::Task).0, "one\ntwo\n- [ ] three");
    }

    #[test]
    fn empty_selection_prefix_keeps_cursor_in_place() {
        let (_, new, sel) = format("ab", 1..1, Format::Heading);
        assert_eq!((new.as_str(), sel), ("### ab", 5..5));
    }

    #[test]
    fn utf8_offsets() {
        assert_eq!(run("привет 👋", 0..12, Format::Bold), ("**привет** 👋".into(), "привет".into()));
        assert_eq!(run("👋\nмир", 5..11, Format::List).0, "👋\n- мир");
    }
}
