//! Root view: routing, session, status bar.

use crate::{backend, theme};
use crate::screens::{
    auth::AuthScreen, pr_detail::PrDetailScreen, pr_list::PrListScreen, repos::ReposScreen, settings::SettingsScreen,
};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    notification::Notification,
    spinner::Spinner,
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::provider::types::{ProviderUser, RateLimitInfo};
use std::time::Duration;

actions!(revisor_nav, [GoBack, OpenSettings]);

#[derive(Clone, PartialEq, Debug)]
pub enum Route {
    Auth,
    Repos,
    PrList { owner: SharedString, repo: SharedString },
    PrDetail { owner: SharedString, repo: SharedString, number: u64 },
    /// Back returns to the screen settings was opened from.
    Settings { back: Box<Route> },
}

impl Route {
    /// Screen that "Back" returns to.
    fn parent(&self) -> Option<Route> {
        match self {
            Route::PrDetail { owner, repo, .. } => Some(Route::PrList { owner: owner.clone(), repo: repo.clone() }),
            Route::PrList { .. } => Some(Route::Repos),
            Route::Settings { back } => Some((**back).clone()),
            Route::Auth | Route::Repos => None,
        }
    }
}

struct Nav(WeakEntity<AppRoot>);
impl Global for Nav {}

/// Go to `route` from anywhere in the app.
pub fn navigate(route: Route, window: &mut Window, cx: &mut App) {
    let root = cx.global::<Nav>().0.clone();
    root.update(cx, |root, cx| root.go(route, window, cx)).ok();
}

/// Go one level up (PR detail → PR list → repos).
pub fn go_back(window: &mut Window, cx: &mut App) {
    let root = cx.global::<Nav>().0.clone();
    root.update(cx, |root, cx| {
        if let Some(route) = root.route.parent() {
            root.go(route, window, cx);
        }
    })
    .ok();
}

/// Open settings over the current screen; Back returns to it (rebuilt, so new sizes apply).
pub fn open_settings(window: &mut Window, cx: &mut App) {
    let root = cx.global::<Nav>().0.clone();
    root.update(cx, |root, cx| {
        if !matches!(root.route, Route::Settings { .. }) {
            root.go(Route::Settings { back: Box::new(root.route.clone()) }, window, cx);
        }
    })
    .ok();
}

pub fn set_user(user: Option<ProviderUser>, window: &mut Window, cx: &mut App) {
    let root = cx.global::<Nav>().0.clone();
    root.update(cx, |root, cx| {
        let route = if user.is_some() { Route::Repos } else { Route::Auth };
        root.user = user;
        root.go(route, window, cx);
    })
    .ok();
}

pub fn current_user(cx: &App) -> Option<ProviderUser> {
    let root = cx.global::<Nav>().0.upgrade()?;
    root.read(cx).user.clone()
}

/// Sign out and return to the auth screen.
pub fn sign_out(window: &mut Window, cx: &mut App) {
    let task = backend::call(cx, |s| async move { revisor_core::auth::logout(&s).await });
    window
        .spawn(cx, async move |cx| {
            let result = task.await;
            cx.update(|window, cx| match result {
                Ok(()) => set_user(None, window, cx),
                Err(e) => window.push_notification(Notification::error(e), cx),
            })
            .ok();
        })
        .detach();
}

pub struct AppRoot {
    route: Route,
    screen: Option<AnyView>,
    user: Option<ProviderUser>,
    rate: Option<RateLimitInfo>,
    nav: usize,
    _poll: Task<()>,
    _restore: Task<()>,
}

impl AppRoot {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.set_global(Nav(cx.weak_entity()));
        let restore = backend::spawn(
            window,
            cx,
            |s| async move { revisor_core::auth::restore_session(&s).await },
            |this: &mut Self, result, window, cx| {
                this.user = result.ok().flatten();
                let route = if this.user.is_some() { Route::Repos } else { Route::Auth };
                this.go(route, window, cx);
            },
        );
        let poll = cx.spawn(async move |this, cx| loop {
            let fut = this.update(cx, |this, cx| {
                this.user.is_some().then(|| {
                    backend::call(cx, |s| async move { Ok(s.provider().await?.rate_limit_status()) })
                })
            });
            match fut {
                Err(_) => return,
                Ok(Some(fut)) => {
                    if let Ok(info) = fut.await {
                        this.update(cx, |this, cx| {
                            this.rate = Some(info);
                            cx.notify();
                        })
                        .ok();
                    }
                }
                Ok(None) => {}
            }
            cx.background_executor().timer(Duration::from_secs(30)).await;
        });
        Self { route: Route::Auth, screen: None, user: None, rate: None, nav: 0, _poll: poll, _restore: restore }
    }

    fn go(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        if self.screen.is_some() && route == self.route {
            return;
        }
        self.screen = Some(match &route {
            Route::Auth => cx.new(|cx| AuthScreen::new(window, cx)).into(),
            Route::Repos => cx.new(|cx| ReposScreen::new(window, cx)).into(),
            Route::PrList { owner, repo } => {
                cx.new(|cx| PrListScreen::new(owner.clone(), repo.clone(), window, cx)).into()
            }
            Route::PrDetail { owner, repo, number } => {
                let viewer = self.user.as_ref().map(|u| u.login.clone());
                cx.new(|cx| PrDetailScreen::new(owner.clone(), repo.clone(), *number, viewer, window, cx)).into()
            }
            Route::Settings { .. } => cx.new(|cx| SettingsScreen::new(window, cx)).into(),
        });
        self.route = route;
        self.nav += 1;
        cx.notify();
    }

    fn status_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let rate_color = match &self.rate {
            Some(r) if r.limit > 0 && (r.remaining as f64) < r.limit as f64 * 0.1 => theme.danger,
            Some(r) if r.limit > 0 && (r.remaining as f64) < r.limit as f64 * 0.3 => theme.warning,
            _ => theme.muted_foreground,
        };
        h_flex()
            .h_6()
            .flex_shrink_0()
            .px_3()
            .gap_1()
            .font_family(theme.mono_font_family.clone())
            .text_size(rems(0.625))
            .text_color(theme.muted_foreground)
            .bg(theme.status_bar)
            .child("REVISOR")
            .children(self.rate.as_ref().map(|r| {
                div().text_color(rate_color).child(format!("— API {}/{}", thousands(r.remaining), thousands(r.limit)))
            }))
            .children(self.user.as_ref().map(|u| {
                h_flex()
                    .ml_auto()
                    .gap_1()
                    .child(theme::avatar(&u.login, &u.avatar_url))
                    .child(format!("@{}", u.login))
            }))
            .child(
                Button::new("settings")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Settings)
                    .tooltip("Settings (⌘,)")
                    .when(self.user.is_none(), |b| b.ml_auto())
                    .on_click(|_, window, cx| open_settings(window, cx)),
            )
    }
}

impl Render for AppRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        // No bg here: Root paints the translucent `background` over the blurred window.
        v_flex()
            .size_full()
            .text_color(theme.foreground)
            .child(
                TitleBar::new().child(
                    h_flex()
                        .gap_2()
                        .child(div().w(px(2.)).h(px(12.)).bg(theme.primary))
                        .child(
                            div()
                                .font_family(theme.mono_font_family.clone())
                                .text_base()
                                .text_color(theme.muted_foreground)
                                .child("Revisor"),
                        ),
                ),
            )
            .child(match &self.screen {
                Some(screen) => div()
                    .flex_1()
                    .overflow_hidden()
                    .px_2()
                    .pb_2()
                    .child(screen.clone())
                    .with_animation(
                        ("screen", self.nav),
                        Animation::new(Duration::from_millis(160)).with_easing(ease_out_quint()),
                        |el, d| el.opacity(d),
                    )
                    .into_any_element(),
                None => div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Spinner::new().large())
                    .into_any_element(),
            })
            .when(self.user.is_some(), |el| el.child(self.status_bar(cx)))
    }
}

/// `4812` → `4,812`.
fn thousands(n: impl ToString) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
