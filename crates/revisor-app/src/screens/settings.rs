//! Settings screen: appearance (theme, fonts, scale).

use crate::{app, theme};
use crate::screens::pr_detail;
use crate::appearance::{self, Field};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    *,
};
use gpui_kit::{prelude::*, *};

// ponytail: curated lists filtered to installed fonts; a searchable picker over all fonts if people ask for more.
const UI_FONTS: &[&str] = &["JetBrains Mono", "Inter", "IBM Plex Sans", "Helvetica Neue", "Avenir Next", "Futura", "Georgia"];
const CODE_FONTS: &[&str] = &[
    "JetBrains Mono",
    "SF Mono",
    "Menlo",
    "Monaco",
    "Fira Code",
    "Cascadia Code",
    "Source Code Pro",
    "IBM Plex Mono",
    "Iosevka",
    "Hack",
    "Courier New",
];
const SCALES: &[&str] = &["80", "90", "100", "110", "125", "150"];

/// Dropdown options as `(value, label)`.
type Options = Vec<(SharedString, SharedString)>;

pub struct SettingsScreen {
    ui_fonts: Options,
    code_fonts: Options,
}

impl SettingsScreen {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        let installed = cx.text_system().all_font_names();
        let fonts = |list: &[&str]| -> Options {
            list.iter()
                .filter(|f| installed.iter().any(|i| i == *f))
                .map(|f| (SharedString::from(*f), SharedString::from(*f)))
                .collect()
        };
        let mut ui_fonts: Options = vec![(appearance::SYSTEM_FONT.into(), "System".into())];
        ui_fonts.extend(fonts(UI_FONTS));
        Self { ui_fonts, code_fonts: fonts(CODE_FONTS) }
    }
}

fn dropdown(field: Field, options: Options) -> SettingField<SharedString> {
    SettingField::dropdown(options, move |cx| appearance::get(field, cx), move |value, cx| appearance::set(field, value, cx))
        .default_value(field.default())
}

impl Render for SettingsScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let mut themes: Options = ThemeRegistry::global(cx)
            .themes()
            .keys()
            .filter_map(|name| Some((name.clone(), SharedString::from(name.strip_prefix("Revisor ")?.to_string()))))
            .collect();
        themes.sort_by(|a, b| a.1.cmp(&b.1));
        let scales = SCALES.iter().map(|s| (SharedString::from(*s), SharedString::from(format!("{s}%")))).collect();

        let appearance = SettingPage::new("Appearance").resettable(true).groups([
            SettingGroup::new().title("Theme").item(
                SettingItem::new("Theme", dropdown(Field::Theme, themes))
                    .description("Colors for the whole app, including code highlighting."),
            ),
            SettingGroup::new()
                .title("Fonts")
                .item(
                    SettingItem::new("Interface font", dropdown(Field::UiFont, self.ui_fonts.clone()))
                        .description("Body text, buttons and lists."),
                )
                .item(
                    SettingItem::new("Code font", dropdown(Field::CodeFont, self.code_fonts.clone()))
                        .description("Diffs, paths, numbers, labels and titles."),
                ),
            SettingGroup::new().title("Scaling").item(
                SettingItem::new("Interface scale", dropdown(Field::Scale, scales)).description("Size of all text and rows."),
            ),
            SettingGroup::new()
                .title("Help")
                .item(
                    SettingItem::new(
                        "Quick tips",
                        SettingField::switch(appearance::tips, appearance::set_tips).default_value(true),
                    )
                    .description("Shortcuts for the current screen, in the status bar (⌘/)."),
                )
                .item(
                    SettingItem::new(
                        "Keyboard shortcuts",
                        SettingField::render(|_, _, _| {
                            Button::new("shortcuts")
                                .small()
                                .outline()
                                .label("Show")
                                .on_click(|_, window, cx| pr_detail::open_shortcuts(window, cx))
                        }),
                    )
                    .description("Every key for reviewing a pull request. Press ? on a pull request to open it there."),
                ),
        ]);

        let header = h_flex()
            .flex_shrink_0()
            .gap_3()
            .px_4()
            .py_3()
            .child(
                Button::new("back")
                    .ghost()
                    .small()
                    .label("← Back")
                    .tooltip_with_action("Back", &app::GoBack, None)
                    .on_click(|_, window, cx| app::go_back(window, cx)),
            )
            .child(div().font_family(theme.mono_font_family.clone()).text_2xl().child("Settings"));
        v_flex()
            .size_full()
            .child(header)
            .child(div().flex_1().overflow_hidden().mt_2().child(theme::island(cx).child(Settings::new("settings").page(appearance))))
    }
}
