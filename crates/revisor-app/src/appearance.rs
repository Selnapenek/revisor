//! Appearance settings (theme, fonts, scale): stored in the `settings` table and applied to the global `Theme`.

use crate::backend;
use gpui_kit::component::{Theme, ThemeRegistry};
use gpui_kit::*;
use revisor_core::cache::queries;

pub const SYSTEM_FONT: &str = ".SystemUIFont";
const DEFAULT_THEME: &str = "Revisor Islands";

#[derive(Clone, Copy)]
pub enum Field {
    Theme,
    UiFont,
    CodeFont,
    Scale,
}

const FIELDS: [Field; 4] = [Field::Theme, Field::UiFont, Field::CodeFont, Field::Scale];

impl Field {
    fn key(self) -> &'static str {
        match self {
            Field::Theme => "appearance.theme",
            Field::UiFont => "appearance.ui_font",
            Field::CodeFont => "appearance.code_font",
            Field::Scale => "appearance.scale",
        }
    }

    pub fn default(self) -> &'static str {
        match self {
            Field::Theme => DEFAULT_THEME,
            Field::UiFont => "JetBrains Mono",
            Field::CodeFont => "JetBrains Mono",
            Field::Scale => "100",
        }
    }
}

struct Appearance([SharedString; 4]);
impl Global for Appearance {}

/// Load saved values and apply them before the first window opens, so the app never flashes the default theme.
pub fn init(cx: &mut App) {
    let saved = backend::block_on(cx, |s| async move {
        let cache = s.cache.lock().await;
        Ok(FIELDS.map(|f| queries::get_setting(cache.conn(), f.key()).ok().flatten()))
    })
    .unwrap_or_default();
    let values = FIELDS.map(|f| saved[f as usize].clone().map_or(f.default().into(), SharedString::from));
    cx.set_global(Appearance(values));
    apply(cx);
}

pub fn get(field: Field, cx: &App) -> SharedString {
    cx.global::<Appearance>().0[field as usize].clone()
}

pub fn set(field: Field, value: SharedString, cx: &mut App) {
    cx.global_mut::<Appearance>().0[field as usize] = value.clone();
    apply(cx);
    let save = backend::call(cx, move |s| async move {
        queries::set_setting(s.cache.lock().await.conn(), field.key(), &value).map_err(backend::err)
    });
    // Best effort: a failed write only means the default comes back next launch.
    cx.background_executor().spawn(save).detach();
}

fn apply(cx: &mut App) {
    let [theme, ui_font, code_font, scale] = cx.global::<Appearance>().0.clone();
    let themes = ThemeRegistry::global(cx).themes();
    let config = themes.get(&theme).or_else(|| themes.get(DEFAULT_THEME)).cloned().expect("theme.json has the default theme");
    let mode = config.mode;
    let current = Theme::global_mut(cx);
    if mode.is_dark() { current.dark_theme = config } else { current.light_theme = config }
    // Loads the theme file, which resets fonts and sizes; the overrides go on top.
    Theme::change(mode, None, cx);
    let scale = scale.parse::<f32>().unwrap_or(100.) / 100.;
    Theme::update(cx, |t| {
        t.font_family = ui_font;
        t.mono_font_family = code_font;
        // Root sets the window rem size from `font_size`, so every rem-based size scales with it.
        t.font_size = px(16. * scale);
        t.mono_font_size = px(13. * scale);
    });
}
