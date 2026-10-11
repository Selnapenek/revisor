//! GitHub-style markdown editor: Write / Preview tabs and a formatting toolbar over a textarea.

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    input::{Textarea, TextareaState},
    tab::{Tab, TabBar},
    text::TextView,
    *,
};
use crate::{backend, video};
use gpui_kit::{prelude::*, *};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::{collections::HashMap, ops::Range, path::PathBuf};

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

/// MIME type GitHub stores an attachment under, from its file name.
fn mime_for(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map_or("", |(_, ext)| ext).to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "json" => "application/json",
        "csv" => "text/csv",
        "txt" | "log" => "text/plain",
        "md" => "text/markdown",
        _ => "application/octet-stream",
    }
}

/// Markdown for an uploaded attachment: images as an `<img>` tag (like github.com, easy to resize), everything else links.
fn link_for(name: &str, mime: &str, url: &str) -> String {
    if mime.starts_with("image/") {
        let alt = name.replace('&', "&amp;").replace('"', "&quot;");
        format!(r#"<img alt="{alt}" src="{url}" />"#)
    } else {
        format!("[{name}]({url})")
    }
}

/// Where byte offset `i` lands after `span` is replaced by `new_len` bytes; inside the span goes to its end.
fn shift(i: usize, span: &Range<usize>, new_len: usize) -> usize {
    if i >= span.end {
        i - span.len() + new_len
    } else if i > span.start {
        span.start + new_len
    } else {
        i
    }
}

pub struct MarkdownEditor {
    pub input: Entity<TextareaState>,
    /// Owner and name of the repository attachments are uploaded to.
    repo: (SharedString, SharedString),
    preview: bool,
    disabled: bool,
    /// Attachments still uploading; their placeholders are in the text.
    uploading: usize,
    upload_error: Option<String>,
    /// Uploaded image URL to a `data:` URL of its bytes. Preview shows these, because
    /// private-repo attachment URLs need a github.com browser session to load.
    images: HashMap<String, String>,
}

impl MarkdownEditor {
    pub fn new(
        initial: &str,
        placeholder: &'static str,
        (min_rows, max_rows): (usize, usize),
        repo: (SharedString, SharedString),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial = initial.to_string();
        let input = cx.new(|cx| {
            TextareaState::new(window, cx).auto_grow(min_rows, max_rows).placeholder(placeholder).default_value(initial)
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        Self { input, repo, preview: false, disabled: false, uploading: 0, upload_error: None, images: HashMap::new() }
    }

    pub fn value(&self, cx: &App) -> String {
        self.input.read(cx).value().trim().to_string()
    }

    /// Submitting now would post upload placeholders instead of links.
    pub fn uploading(&self) -> bool {
        self.uploading > 0
    }

    /// Insert a placeholder at the cursor, upload in the background, then swap in the link (or drop it on error).
    /// `local` is the file on disk, if any: uploaded videos play from it.
    fn upload(
        &mut self,
        name: String,
        load: impl FnOnce() -> std::io::Result<Vec<u8>> + Send + 'static,
        local: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let mime = mime_for(&name);
        let placeholder = format!("![Uploading {name}…]()");
        self.uploading += 1;
        self.upload_error = None;
        self.input.update(cx, |input, cx| input.replace(format!("{placeholder}\n"), window, cx));
        cx.notify();

        let (owner, repo, file) = (self.repo.0.to_string(), self.repo.1.to_string(), name.clone());
        backend::spawn(
            window,
            cx,
            move |s| async move {
                let bytes = load().map_err(backend::err)?;
                // Only formats GPUI can decode; others stay links in the preview.
                let data = ImageFormat::from_mime_type(mime).map(|_| format!("data:{mime};base64,{}", BASE64.encode(&bytes)));
                let url = s.provider().await?.upload_attachment(&owner, &repo, &file, mime, bytes).await.map_err(backend::err)?;
                Ok((url, data))
            },
            move |this: &mut Self, result, window, cx| {
                this.uploading -= 1;
                let video = mime.starts_with("video/");
                let link = match result {
                    // Like github.com: a bare URL in its own paragraph, which renders as a player.
                    Ok((url, _)) if video => {
                        if let Some(path) = local {
                            video::remember_local(url.clone(), path, cx);
                        }
                        url
                    }
                    Ok((url, data)) => {
                        let link = link_for(&name, mime, &url);
                        this.images.extend(data.map(|data| (url, data)));
                        link
                    }
                    Err(e) => {
                        this.upload_error = Some(format!("Failed to upload {name}: {e}"));
                        String::new()
                    }
                };
                this.input.update(cx, |input, cx| {
                    // The user may have deleted the placeholder meanwhile.
                    let value = input.value();
                    let Some(start) = value.find(&placeholder) else { return };
                    let link = if video && !link.is_empty() { video::video_block(&value[..start], &link) } else { link };
                    let span = start..start + placeholder.len();
                    let sel = input.selected_range();
                    input.set_selected_range(span.clone(), cx);
                    input.replace(link.clone(), window, cx);
                    input.set_selected_range(shift(sel.start, &span, link.len())..shift(sel.end, &span, link.len()), cx);
                });
            },
        )
        .detach();
    }

    fn upload_paths(&mut self, paths: &[PathBuf], window: &mut Window, cx: &mut Context<Self>) {
        for path in paths.iter().filter(|p| p.is_file()) {
            let name = path.file_name().map_or("file".into(), |n| n.to_string_lossy().into_owned());
            let (path, local) = (path.clone(), path.clone());
            self.upload(name, move || std::fs::read(path), Some(local), window, cx);
        }
    }

    fn pick_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Attach".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                this.update_in(cx, |this, window, cx| this.upload_paths(&paths, window, cx)).ok();
            }
        })
        .detach();
    }

    /// Upload pasted images and copied files; `false` lets plain text paste as usual.
    fn paste(&mut self, item: &ClipboardItem, window: &mut Window, cx: &mut Context<Self>) -> bool {
        // Some apps put an image rendering next to copied text; text wins, like on github.com.
        let has_text = item.entries().iter().any(|e| matches!(e, ClipboardEntry::String(_)));
        let mut handled = false;
        for entry in item.entries() {
            match entry {
                ClipboardEntry::Image(image) if !has_text => {
                    let bytes = image.bytes.clone();
                    self.upload(format!("image.{}", image.format.extension()), move || Ok(bytes), None, window, cx);
                    handled = true;
                }
                ClipboardEntry::ExternalPaths(paths) => {
                    self.upload_paths(paths.paths(), window, cx);
                    handled = true;
                }
                _ => {}
            }
        }
        handled
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
        let drop_target = theme.drop_target;
        let this = cx.entity().downgrade();
        v_flex()
            .gap_2()
            .key_context(CONTEXT)
            .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(drop_target))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| this.upload_paths(paths.paths(), window, cx)))
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
                        .child(
                            Button::new("md-attach")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Paperclip)
                                .tooltip("Attach files (or paste / drop them)")
                                .disabled(self.disabled)
                                .on_click(cx.listener(|this, _, window, cx| this.pick_files(window, cx))),
                        )
                    }),
            )
            .map(|el| {
                if !self.preview {
                    el.child(Textarea::new(&self.input).small().disabled(self.disabled).on_paste(move |item, window, cx| {
                        this.update(cx, |this, cx| this.paste(item, window, cx)).unwrap_or(false)
                    }))
                } else if body.is_empty() {
                    el.child(div().py_2().text_xs().text_color(theme.muted_foreground).child("Nothing to preview"))
                } else {
                    let body = self.images.iter().fold(body, |body, (url, data)| body.replace(url.as_str(), data));
                    el.child(div().text_xs().child(TextView::markdown(("md-preview", cx.entity_id().as_u64()), body).plugin(video::Videos)))
                }
            })
            .children(self.upload_error.clone().map(|e| div().text_xs().text_color(theme.danger).child(e)))
    }
}

#[cfg(test)]
mod tests {
    use super::{format, link_for, mime_for, shift, Format};
    use std::ops::Range;

    #[test]
    fn attachment_mime_and_link() {
        assert_eq!(mime_for("Shot.PNG"), "image/png");
        assert_eq!(mime_for("a.b.jpeg"), "image/jpeg");
        assert_eq!(mime_for("Makefile"), "application/octet-stream");
        assert_eq!(link_for("a.png", "image/png", "u"), r#"<img alt="a.png" src="u" />"#);
        assert_eq!(link_for(r#""x"&y.png"#, "image/png", "u"), r#"<img alt="&quot;x&quot;&amp;y.png" src="u" />"#);
        assert_eq!(link_for("a.pdf", "application/pdf", "u"), "[a.pdf](u)");
    }

    #[test]
    fn cursor_follows_placeholder_swap() {
        let span = 4..10; // 6-byte placeholder becomes 2 bytes
        assert_eq!(shift(2, &span, 2), 2);
        assert_eq!(shift(4, &span, 2), 4);
        assert_eq!(shift(7, &span, 2), 6);
        assert_eq!(shift(10, &span, 2), 6);
        assert_eq!(shift(15, &span, 2), 11);
    }

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
