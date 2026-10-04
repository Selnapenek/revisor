mod app;
mod backend;
mod screens;
mod theme;

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

actions!(revisor, [Quit]);

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            backend::init(cx);
            screens::pr_detail::init(cx);

            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
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
                titlebar: Some(TitlebarOptions {
                    title: Some("Revisor".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| app::AppRoot::new(window, cx))
            })
            .expect("Failed to open window");
            cx.activate(true);
        });
}
