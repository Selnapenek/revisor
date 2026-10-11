mod app;
mod appearance;
mod backend;
mod screens;
mod theme;

use gpui_kit::component::{ThemeRegistry, TitleBar};
use gpui_kit::*;

actions!(revisor, [Quit]);

// Default `Assets` ships only the component icons; add the markdown toolbar ones.
gpui_kit::assets::icon_assets!(EditorIcons, [Heading, Bold, Italic, TextQuote, Code, Link, List, ListOrdered, ListChecks]);

struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        match EditorIcons.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => gpui_kit::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(EditorIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(AppAssets)
        .with_http_client(std::sync::Arc::new(reqwest_client::ReqwestClient::new()))
        .run(|cx| {
            gpui_kit::init(cx);
            cx.text_system()
                .add_fonts(
                    [
                        &include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")[..],
                        include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf"),
                        include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf"),
                        include_bytes!("../assets/fonts/JetBrainsMono-SemiBold.ttf"),
                    ]
                    .map(std::borrow::Cow::Borrowed)
                    .into(),
                )
                .expect("bundled fonts are valid");
            let registry = ThemeRegistry::global_mut(cx);
            registry.load_themes_from_str(include_str!("theme.json")).expect("theme.json is valid");
            backend::init(cx);
            appearance::init(cx);
            screens::pr_detail::init(cx);

            cx.on_action(|_: &Quit, cx| cx.quit());
            // Global, so it works with nothing focused. Deferred: action handlers run inside a window update.
            cx.on_action(|_: &app::GoBack, cx| {
                cx.defer(|cx| {
                    if let Some(window) = cx.active_window() {
                        window.update(cx, |_, window, cx| app::go_back(window, cx)).ok();
                    }
                })
            });
            cx.on_action(|_: &app::OpenSettings, cx| {
                cx.defer(|cx| {
                    if let Some(window) = cx.active_window() {
                        window.update(cx, |_, window, cx| app::open_settings(window, cx)).ok();
                    }
                })
            });
            cx.bind_keys([
                KeyBinding::new("cmd-q", Quit, None),
                KeyBinding::new("cmd-[", app::GoBack, None),
                KeyBinding::new("cmd-,", app::OpenSettings, None),
            ]);
            cx.set_menus([Menu {
                name: "Revisor".into(),
                items: vec![
                    MenuItem::action("Settings…", app::OpenSettings),
                    MenuItem::separator(),
                    MenuItem::action("Quit Revisor", Quit),
                ],
                disabled: false,
            }]);
            cx.on_window_closed(|cx, _| cx.quit()).detach();

            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1400.), px(900.)),
                    cx,
                ))),
                window_min_size: Some(size(px(900.), px(600.))),
                window_background: WindowBackgroundAppearance::Blurred,
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| app::AppRoot::new(window, cx))
            })
            .expect("Failed to open window");
            cx.activate(true);
        });
}
