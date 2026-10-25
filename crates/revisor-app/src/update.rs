//! Looks for a new release at start and every 6 hours. A new version opens a prompt once and
//! keeps an update button in the title bar; updating installs the release and restarts Revisor.

use crate::backend;
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    notification::Notification,
    *,
};
use gpui_kit::*;
use revisor_core::update::{self, Release};
use std::time::Duration;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

actions!(revisor_update, [CheckForUpdates]);

enum Status {
    Current,
    Available(Release),
    Installing(Release),
}

pub struct Updater {
    status: Status,
    /// Version the prompt opened for, so the periodic check does not open it again.
    prompted: Option<String>,
    _poll: Task<()>,
}

struct GlobalUpdater(Entity<Updater>);
impl Global for GlobalUpdater {}

pub fn init(cx: &mut App) {
    let updater = cx.new(|cx: &mut Context<Updater>| {
        let poll = cx.spawn(async move |this, cx| loop {
            let Ok(check) = this.update(cx, |_, cx| check(cx)) else { return };
            // Offline or a server error: try again at the next check.
            if let Ok(Some(release)) = check.await {
                this.update(cx, |this, cx| this.found(release, false, cx)).ok();
            }
            cx.background_executor().timer(Duration::from_secs(6 * 60 * 60)).await;
        });
        Updater { status: Status::Current, prompted: None, _poll: poll }
    });
    cx.set_global(GlobalUpdater(updater));
    cx.on_action(|_: &CheckForUpdates, cx| check_now(cx));
}

pub fn updater(cx: &App) -> Entity<Updater> {
    cx.global::<GlobalUpdater>().0.clone()
}

fn check(cx: &App) -> impl std::future::Future<Output = Result<Option<Release>, String>> {
    backend::call(cx, |_| async { update::check(VERSION).await.map_err(backend::err) })
}

/// Menu command: always tell the result.
fn check_now(cx: &mut App) {
    let check = check(cx);
    cx.spawn(async move |cx| {
        let result = check.await;
        cx.update(|cx| match result {
            Ok(Some(release)) => updater(cx).update(cx, |this, cx| this.found(release, true, cx)),
            Ok(None) => notify(Notification::info(format!("Revisor {VERSION} is the latest version.")), cx),
            Err(e) => notify(Notification::error(format!("Cannot check for updates: {e}")), cx),
        });
    })
    .detach();
}

impl Updater {
    fn found(&mut self, release: Release, prompt: bool, cx: &mut Context<Self>) {
        if matches!(self.status, Status::Installing(_)) {
            return;
        }
        let prompt = prompt || self.prompted.as_ref() != Some(&release.version);
        self.prompted = Some(release.version.clone());
        self.status = Status::Available(release);
        cx.notify();
        if prompt {
            cx.defer(|cx| {
                if let Some(window) = cx.active_window() {
                    window.update(cx, |_, window, cx| open_prompt(window, cx)).ok();
                }
            });
        }
    }

    fn install(&mut self, cx: &mut Context<Self>) {
        let Status::Available(release) = &self.status else { return };
        let release = release.clone();
        self.status = Status::Installing(release.clone());
        cx.notify();
        let install = backend::call(cx, {
            let release = release.clone();
            move |_| async move { update::install(&release).await.map_err(backend::err) }
        });
        cx.spawn(async move |this, cx| {
            let result = install.await.and_then(|app| update::launch(&app).map_err(backend::err));
            this.update(cx, |this, cx| match result {
                Ok(()) => cx.quit(),
                Err(e) => {
                    this.status = Status::Available(release);
                    cx.notify();
                    notify(Notification::error(format!("Update failed: {e}")), cx);
                }
            })
            .ok();
        })
        .detach();
    }
}

fn notify(notification: Notification, cx: &mut App) {
    if let Some(window) = cx.active_window() {
        window.update(cx, |_, window, cx| window.push_notification(notification, cx)).ok();
    }
}

fn open_prompt(window: &mut Window, cx: &mut App) {
    let Status::Available(release) = &updater(cx).read(cx).status else { return };
    let title = format!("Revisor {} is available", release.version);
    // Long release notes would push the buttons off the dialog.
    let notes: Vec<&str> = release.notes.lines().filter(|l| !l.trim().is_empty()).take(12).collect();
    let description = format!("You have version {VERSION}. Revisor restarts after the update.\n\n{}", notes.join("\n"));
    window.open_alert_dialog(cx, move |alert, _, _| {
        alert
            .confirm()
            .title(title.clone())
            .description(description.clone())
            .ok_text("Update and restart")
            .cancel_text("Later")
            .on_ok(|_, _, cx| {
                updater(cx).update(cx, |this, cx| this.install(cx));
                true
            })
    });
}

/// Title bar button while an update is available or installing.
pub fn title_bar_button(cx: &App) -> Option<impl IntoElement> {
    let (label, installing) = match &updater(cx).read(cx).status {
        Status::Current => return None,
        Status::Available(release) => (format!("Update to {}", release.version), false),
        Status::Installing(release) => (format!("Updating to {}…", release.version), true),
    };
    Some(
        Button::new("update")
            .primary()
            .xsmall()
            .label(label)
            .loading(installing)
            .disabled(installing)
            .on_click(|_, window, cx| open_prompt(window, cx)),
    )
}
