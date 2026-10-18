//! Opt-in TypeSafe features, stored in the app settings table.

use crate::backend;
use gpui_kit::*;
use revisor_core::cache::queries;

const KEY: &str = "experimental.typesafe";

struct TypeSafeEnabled(bool);
impl Global for TypeSafeEnabled {}

pub fn init(cx: &mut App) {
    let enabled = backend::block_on(cx, |state| async move {
        let cache = state.cache.lock().await;
        queries::get_setting(cache.conn(), KEY).map_err(backend::err)
    })
    .ok()
    .flatten()
    .is_some_and(|value| value == "on");
    cx.set_global(TypeSafeEnabled(enabled));
}

pub fn enabled(cx: &App) -> bool {
    cx.global::<TypeSafeEnabled>().0
}

pub fn set_enabled(enabled: bool, cx: &mut App) {
    cx.global_mut::<TypeSafeEnabled>().0 = enabled;
    let save = backend::call(cx, move |state| async move {
        queries::set_setting(
            state.cache.lock().await.conn(),
            KEY,
            if enabled { "on" } else { "off" },
        )
        .map_err(backend::err)
    });
    cx.background_executor().spawn(save).detach();
}
