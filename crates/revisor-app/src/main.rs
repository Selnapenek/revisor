mod app;
mod backend;
mod screens;
mod theme;

use gpui_kit::component::{Theme, ThemeMode, ThemeRegistry, TitleBar};
use gpui_kit::*;

actions!(revisor, [Quit]);

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .with_http_client(std::sync::Arc::new(reqwest_client::ReqwestClient::new()))
        .run(|cx| {
            gpui_kit::init(cx);
            let registry = ThemeRegistry::global_mut(cx);
            registry.load_themes_from_str(include_str!("theme.json")).expect("theme.json is valid");
            let dark = registry.themes()["Revisor Dark"].clone();
            Theme::global_mut(cx).dark_theme = dark;
            Theme::change(ThemeMode::Dark, None, cx);
            backend::init(cx);
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
            cx.bind_keys([KeyBinding::new("cmd-q", Quit, None), KeyBinding::new("cmd-[", app::GoBack, None)]);
            cx.set_menus([Menu {
                name: "Revisor".into(),
                items: vec![MenuItem::action("Quit Revisor", Quit)],
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
