//! Shared visual marks and the colors the stock theme lacks, all derived from `theme.json`.
//! `theme.primary` is the marking pencil for actions and active state; `theme.link` / `theme.info`
//! is for notes, links and "reviewed".

use gpui_kit::component::{avatar::Avatar, ActiveTheme as _, Sizable as _, Theme};
use gpui_kit::*;

/// A floating panel on the window canvas.
pub fn island(cx: &App) -> Div {
    let theme = cx.theme();
    div().size_full().overflow_hidden().rounded(px(12.)).bg(theme.colors.list)
        .border_1().border_color(theme.foreground.opacity(0.06)).shadow_md()
}

pub fn diff_add_bg(cx: &App) -> Hsla { cx.theme().success.opacity(0.12) }
pub fn diff_add_word(cx: &App) -> Hsla { cx.theme().success.opacity(0.32) }
pub fn diff_del_bg(cx: &App) -> Hsla { cx.theme().danger.opacity(0.12) }
pub fn diff_del_word(cx: &App) -> Hsla { cx.theme().danger.opacity(0.35) }
pub fn diff_hunk_bg(cx: &App) -> Hsla { cx.theme().info.opacity(0.08) }
pub fn search_match(cx: &App) -> Hsla { cx.theme().warning.opacity(0.4) }
pub fn search_match_active(cx: &App) -> Hsla { cx.theme().warning.opacity(0.7) }

/// Parse a GitHub label color like `d73a4a`.
pub fn hex(color: &str) -> Hsla {
    rgb(u32::from_str_radix(color, 16).unwrap_or(0x9a937f)).into()
}

/// Small round user avatar; initials when GitHub gave no URL (an empty image source would render broken).
pub fn avatar(login: &str, url: &str) -> Avatar {
    let avatar = Avatar::new().name(login.to_string()).xsmall();
    if url.is_empty() { avatar } else { avatar.src(url.to_string()) }
}

/// The app's signature: a 2px bar on the left edge of a `relative()` parent.
/// For hover-only marks: `.invisible().group_hover(GROUP, |s| s.visible())`.
pub fn margin_mark(color: Hsla) -> Div {
    div().absolute().left_0().top_0().bottom_0().w(px(2.)).bg(color)
}

/// Small uppercase mono label above or beside content ("FILES", "7 / 12 VIEWED").
pub fn eyebrow(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(rems(0.625))
        .font_weight(FontWeight::MEDIUM)
        .text_color(cx.theme().muted_foreground)
        .child(text.into().to_uppercase())
}

/// Outlined, square, uppercase proof stamp (PR and review states).
pub fn stamp(label: impl Into<SharedString>, color: impl Into<Hsla>, theme: &Theme) -> Div {
    let color = color.into();
    div()
        .flex_shrink_0()
        .px_1p5()
        .border_1()
        .border_color(color.opacity(0.75))
        .rounded(px(2.))
        .font_family(theme.mono_font_family.clone())
        .text_size(rems(0.625))
        .line_height(rems(1.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(label.into().to_uppercase())
}

/// Stamp for a PR's state: open (success), merged (info), closed (danger), draft (muted).
pub fn pr_state_stamp(merged: bool, state: &str, draft: bool, cx: &App) -> Div {
    let theme = cx.theme();
    if merged {
        stamp("Merged", theme.info, theme)
    } else if state == "closed" {
        stamp("Closed", theme.danger, theme)
    } else if draft {
        stamp("Draft", theme.muted_foreground, theme)
    } else {
        stamp("Open", theme.success, theme)
    }
}

/// File status letter and color, shared by the file tree and the diff headers.
pub fn file_status(status: &str, cx: &App) -> (&'static str, Hsla) {
    let theme = cx.theme();
    match status {
        "added" => ("A", theme.success),
        "removed" => ("D", theme.danger),
        "renamed" => ("R", theme.warning),
        _ => ("M", theme.info),
    }
}
