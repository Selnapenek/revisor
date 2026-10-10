//! Colors the stock theme does not have (diff palette tuned to `theme.json`).

use gpui_kit::*;

pub fn diff_add_bg() -> Hsla { rgba(0x3fb95024).into() }
pub fn diff_add_word() -> Hsla { rgba(0x3fb9505c).into() }
pub fn diff_del_bg() -> Hsla { rgba(0xf8514924).into() }
pub fn diff_del_word() -> Hsla { rgba(0xf851495c).into() }
pub fn diff_hunk_bg() -> Hsla { rgba(0x7c6cff1f).into() }
pub fn search_match() -> Hsla { rgba(0xbb800966).into() }
pub fn search_match_active() -> Hsla { rgba(0xf2cc60aa).into() }

/// Parse a GitHub label color like `d73a4a`.
pub fn hex(color: &str) -> Hsla {
    rgb(u32::from_str_radix(color, 16).unwrap_or(0x8b949e)).into()
}
