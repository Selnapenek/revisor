//! Root view: routing, session, status bar.

use crate::{backend, theme};
use crate::screens::{auth::AuthScreen, pr_detail::PrDetailScreen, pr_list::PrListScreen, repos::ReposScreen};
use gpui_kit::component::{notification::Notification, spinner::Spinner, *};
use gpui_kit::{prelude::*, *};
use revisor_core::provider::types::{ProviderUser, RateLimitInfo};
use std::time::Duration;

#[derive(Clone, PartialEq, Debug)]
pub enum Route {
    Auth,
    Repos,
    PrList { owner: SharedString, repo: SharedString },
    PrDetail { owner: SharedString, repo: SharedString, number: u64 },
}

struct Nav(WeakEntity<AppRoot>);
impl Global for Nav {}

/// Go to `route` from anywhere in the app.
pub fn navigate(route: Route, window: &mut Window, cx: &mut App) {
    let root = cx.global::<Nav>().0.clone();
    root.update(cx, |root, cx| root.go(route, window, cx)).ok();
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
        Self { route: Route::Auth, screen: None, user: None, rate: None, _poll: poll, _restore: restore }
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
            Route::PrDetail { owner, repo, number } => cx
                .new(|cx| PrDetailScreen::new(owner.clone(), repo.clone(), *number, window, cx))
                .into(),
        });
        self.route = route;
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
            .gap_4()
            .text_xs()
            .text_color(theme.muted_foreground)
            .bg(theme.status_bar)
            .border_t_1()
            .border_color(theme.status_bar_border)
            .child("Revisor")
            .children(self.rate.as_ref().map(|r| {
                div().text_color(rate_color).child(format!("API: {}/{}", r.remaining, r.limit))
            }))
            .children(self.user.as_ref().map(|u| {
                h_flex().ml_auto().gap_1().child(theme::avatar(&u.login, &u.avatar_url)).child(u.login.clone())
            }))
    }
}

impl Render for AppRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        // No bg here: Root paints the translucent `background` over the blurred window.
        v_flex()
            .size_full()
            .text_color(theme.foreground)
            .child(TitleBar::new().child(div().text_xs().font_medium().text_color(theme.muted_foreground).child("Revisor")))
            .child(match &self.screen {
                Some(screen) => div().flex_1().overflow_hidden().child(screen.clone()),
                None => div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Spinner::new().large()),
            })
            .when(self.user.is_some(), |el| el.child(self.status_bar(cx)))
    }
}
