//! Root view: routing, session, status bar.

use crate::{appearance, backend, theme};
use crate::screens::{
    auth::AuthScreen, pr_detail::{self, state::PrModel, PrDetailScreen}, pr_list::{self, PrListScreen}, repos::ReposScreen, settings::SettingsScreen,
};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    kbd::Kbd,
    notification::Notification,
    popover::Popover,
    spinner::Spinner,
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::provider::types::{ProviderUser, RateLimitInfo};
use revisor_core::ai_usage::{self, Usage, WindowUsage};
use revisor_core::demo::{self, Progress};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

actions!(revisor_nav, [GoBack, OpenSettings, ToggleTips]);

#[derive(Clone, PartialEq, Debug)]
pub enum Route {
    Auth,
    Repos,
    PrList { owner: SharedString, repo: SharedString },
    PrDetail { owner: SharedString, repo: SharedString, number: u64 },
    /// Back returns to the screen settings was opened from.
    Settings { back: Box<Route> },
}

/// One quick tip: keys, then what they do.
type Tip = (Vec<Box<dyn Action>>, &'static str);

impl Route {
    /// Most useful shortcuts on this screen, with the key context their bindings need.
    fn tips(&self) -> (Option<&'static str>, Vec<Tip>) {
        use pr_detail::*;
        match self {
            Route::PrDetail { .. } => (
                Some(pr_detail::CONTEXT),
                vec![
                    (vec![Box::new(NextChange), Box::new(PrevChange)], "to move between changes"),
                    (vec![Box::new(NextThread)], "for next thread"),
                    (vec![Box::new(NextUnreviewed)], "for next unreviewed file"),
                    (vec![Box::new(ToggleViewed)], "to mark viewed"),
                    (vec![Box::new(ToggleSearch)], "to search"),
                    (vec![Box::new(ShowShortcuts)], "for all shortcuts"),
                ],
            ),
            Route::PrList { .. } => (
                Some(pr_list::CONTEXT),
                vec![
                    (vec![Box::new(pr_list::SelectNext), Box::new(pr_list::SelectPrev)], "to move between PRs"),
                    (vec![Box::new(pr_list::OpenSelected)], "to open"),
                    (vec![Box::new(GoBack)], "to go back"),
                    (vec![Box::new(OpenSettings)], "for settings"),
                ],
            ),
            Route::Repos => (None, vec![(vec![Box::new(OpenSettings)], "for settings")]),
            Route::Settings { .. } => (None, vec![(vec![Box::new(GoBack)], "to go back")]),
            Route::Auth => (None, vec![]),
        }
    }

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

/// Start loading one PR while its list row is hovered or selected.
pub fn preload_pr(route: Route, window: &mut Window, cx: &mut App) {
    let root = cx.global::<Nav>().0.clone();
    root.update(cx, |root, cx| root.preload_pr(route, window, cx)).ok();
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
        root.sample = None;
        root.go(route, window, cx);
        if root.user.is_some() { root.refresh_usage(window, cx); } else { root.usage = None; }
    })
    .ok();
}

pub fn current_user(cx: &App) -> Option<ProviderUser> {
    let root = cx.global::<Nav>().0.upgrade()?;
    root.read(cx).user.clone()
}

/// Open the bundled sample repository; no GitHub account or AI CLI needed. Restarts a running sample.
pub fn start_sample(window: &mut Window, cx: &mut App) {
    let task = backend::call(cx, |s| async move { demo::start(&s).await });
    window
        .spawn(cx, async move |cx| {
            let result = task.await;
            cx.update(|window, cx| match result {
                Ok(user) => {
                    let root = cx.global::<Nav>().0.clone();
                    root.update(cx, |root, cx| root.enter_sample(user, window, cx)).ok();
                }
                Err(e) => window.push_notification(Notification::error(format!("Failed to open the sample: {e}")), cx),
            })
            .ok();
        })
        .detach();
}

pub fn in_sample(cx: &App) -> bool {
    cx.global::<Nav>().0.upgrade().is_some_and(|root| root.read(cx).sample.is_some())
}

/// Sign out, or leave the sample, and return to the auth screen.
pub fn sign_out(window: &mut Window, cx: &mut App) {
    let sample = in_sample(cx);
    let task = backend::call(cx, move |s| async move {
        if sample { demo::stop(&s).await } else { revisor_core::auth::logout(&s).await }
    });
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
    pr_list: Option<(SharedString, SharedString, Entity<PrListScreen>)>,
    pr_details: Vec<(Route, Entity<PrDetailScreen>)>,
    preloaded_pr: Option<(Route, Entity<PrModel>, Instant)>,
    pending_preload: Option<Route>,
    _preload_delay: Task<()>,
    user: Option<ProviderUser>,
    rate: Option<RateLimitInfo>,
    usage: Option<Vec<Usage>>,
    nav: usize,
    sample: Option<Sample>,
    _poll: Task<()>,
    _restore: Task<()>,
}

/// Sample mode and its getting-started checklist.
struct Sample {
    opened: bool,
    progress: Progress,
    _poll: Task<()>,
}

impl Sample {
    /// (done, step, how) for the checklist.
    fn steps(&self) -> [(bool, &'static str, &'static str); 5] {
        let p = self.progress;
        [
            (self.opened, "Open pull request #42", "Select it in the list and press Enter."),
            (p.ai, "Run the AI review", "Overview tab, then Run AI review. The sample replays a recorded review."),
            (p.viewed, "Mark a file viewed", "Tick a file in the Files tab, or use the shortcut in the status bar."),
            (p.commented, "Comment on a line", "Use Add comment on an AI finding, or click a line in the diff."),
            (p.submitted, "Submit your review", "Review in the toolbar, then Comment, Approve or Request changes."),
        ]
    }
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
                if this.user.is_some() { this.refresh_usage(window, cx); }
            },
        );
        let poll = cx.spawn(async move |this, cx| {
            let mut ticks = 0u32;
            loop {
                let fut = this.update(cx, |this, cx| {
                    (this.user.is_some() && this.sample.is_none()).then(|| {
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
                        if ticks > 0 && ticks.is_multiple_of(10) {
                            let usage = ai_usage::installed_usage().await;
                            this.update(cx, |this, cx| { if this.user.is_some() { this.usage = Some(usage); cx.notify(); } }).ok();
                        }
                    }
                    Ok(None) => {}
                }
                cx.background_executor().timer(Duration::from_secs(30)).await;
                ticks = ticks.wrapping_add(1);
            }
        });
        Self { route: Route::Auth, screen: None, pr_list: None, pr_details: Vec::new(), preloaded_pr: None, pending_preload: None, _preload_delay: Task::ready(()), user: None, rate: None, usage: None, nav: 0, sample: None, _poll: poll, _restore: restore }
    }

    fn enter_sample(&mut self, user: ProviderUser, window: &mut Window, cx: &mut Context<Self>) {
        // Screens of an earlier run show data the restart removed.
        self.pr_list = None;
        self.pr_details.clear();
        self.preloaded_pr = None;
        self.screen = None;
        self.user = Some(user);
        self.rate = None;
        self.usage = None;
        // ponytail: polls every 2 s because the steps happen on several screens; send events from them if this ever costs anything.
        let poll = cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let Ok(fut) = this.update(cx, |_, cx| backend::call(cx, |s| async move { demo::progress(&s).await })) else { return };
            let Ok(progress) = fut.await else { continue };
            let updated = this.update(cx, |this, cx| {
                if let Some(sample) = this.sample.as_mut().filter(|s| s.progress != progress) {
                    sample.progress = progress;
                    cx.notify();
                }
            });
            if updated.is_err() {
                return;
            }
        });
        self.sample = Some(Sample { opened: false, progress: Progress::default(), _poll: poll });
        self.go(Route::PrList { owner: demo::OWNER.into(), repo: demo::REPO.into() }, window, cx);
        window.push_notification(
            Notification::info("This is a sample repository. Follow Getting started in the status bar to try each feature."),
            cx,
        );
    }

    fn preload_pr(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.route, Route::PrList { .. })
            || self.pr_details.iter().any(|(key, _)| key == &route)
            || self.preloaded_pr.as_ref().is_some_and(|(key, model, at)| {
                key == &route && model.read(cx).error.is_none()
                    && (model.read(cx).loading || at.elapsed() < Duration::from_secs(60))
            })
            || self.pending_preload.as_ref() == Some(&route)
        {
            return;
        }
        self.pending_preload = Some(route.clone());
        self._preload_delay = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(120)).await;
            this.update_in(cx, |this, _, cx| {
                if this.pending_preload.as_ref() != Some(&route) { return; }
                this.pending_preload = None;
                if let Route::PrDetail { owner, repo, number } = &route {
                    let viewer = this.user.as_ref().map(|u| u.login.clone());
                    let model = cx.new(|cx| PrModel::new(owner.clone(), repo.clone(), *number, viewer, cx));
                    this.preloaded_pr = Some((route, model, Instant::now()));
                }
            }).ok();
        });
    }

    fn refresh_usage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let task = backend::call(cx, |_| async { Ok(ai_usage::installed_usage().await) });
        window.spawn(cx, async move |cx| {
            let result = task.await;
            cx.update(|_, cx| {
                let root = cx.global::<Nav>().0.clone();
                root.update(cx, |this, cx| {
                    if let Ok(usage) = result { if this.user.is_some() { this.usage = Some(usage); cx.notify(); } }
                }).ok();
            }).ok();
        }).detach();
    }

    fn go(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        if self.screen.is_some() && route == self.route {
            return;
        }
        self.pending_preload = None;
        self._preload_delay = Task::ready(());
        if let Route::PrDetail { owner, repo, number } = &self.route {
            pr_detail::mark_viewed(owner.to_string(), repo.to_string(), *number, cx);
        }
        if let (Some(sample), Route::PrDetail { number: demo::PR, .. }) = (&mut self.sample, &route) {
            sample.opened = true;
        }
        self.screen = Some(match &route {
            Route::Auth => {
                self.pr_list = None;
                self.pr_details.clear();
                self.preloaded_pr = None;
                cx.new(|cx| AuthScreen::new(window, cx)).into()
            }
            Route::Repos => {
                self.preloaded_pr = None;
                cx.new(|cx| ReposScreen::new(window, cx)).into()
            }
            Route::PrList { owner, repo } => {
                let cached = self.pr_list.as_ref()
                    .filter(|(cached_owner, cached_repo, _)| cached_owner == owner && cached_repo == repo)
                    .map(|(_, _, list)| list.clone());
                if let Some(list) = cached {
                    list.update(cx, |list, cx| list.refresh(window, cx));
                    list.into()
                } else {
                    let list = cx.new(|cx| PrListScreen::new(owner.clone(), repo.clone(), window, cx));
                    self.pr_list = Some((owner.clone(), repo.clone(), list.clone()));
                    list.into()
                }
            }
            Route::PrDetail { owner, repo, number } => {
                if let Some(index) = self.pr_details.iter().position(|(key, _)| key == &route) {
                    let entry = self.pr_details.remove(index);
                    entry.1.update(cx, |detail, cx| detail.refresh(window, cx));
                    let detail = entry.1.clone();
                    self.pr_details.push(entry);
                    detail.into()
                } else {
                    let viewer = self.user.as_ref().map(|u| u.login.clone());
                    let preloaded = self.preloaded_pr.take().and_then(|(key, model, at)| {
                        (key == route && model.read(cx).error.is_none()
                            && (model.read(cx).loading || at.elapsed() < Duration::from_secs(60))).then_some(model)
                    });
                    let detail = cx.new(|cx| PrDetailScreen::new(owner.clone(), repo.clone(), *number, viewer, preloaded, window, cx));
                    if self.pr_details.len() == 5 {
                        self.pr_details.remove(0);
                    }
                    self.pr_details.push((route.clone(), detail.clone()));
                    detail.into()
                }
            }
            Route::Settings { .. } => cx.new(|cx| SettingsScreen::new(window, cx)).into(),
        });
        self.route = route;
        self.nav += 1;
        cx.notify();
    }

    /// Status bar tips: "Hit J K to move between changes · N for next thread · …"; keys come from the live bindings.
    fn tips(&self, window: &Window, cx: &Context<Self>) -> Option<impl IntoElement> {
        if !appearance::tips(cx) {
            return None;
        }
        let (context, tips) = self.route.tips();
        let mut tips = tips.into_iter().filter_map(|(actions, label)| {
            let keys: Vec<Kbd> = actions.iter().filter_map(|a| Kbd::binding_for_action(a.as_ref(), context, window)).collect();
            (!keys.is_empty()).then_some((keys, label))
        });
        let first = tips.next()?;
        let tip = |(keys, label): (Vec<Kbd>, &'static str)| h_flex().flex_shrink_0().gap_1().children(keys).child(label);
        Some(
            // Fills the gap between the API count and the user; clipped, never wrapped, when narrow.
            h_flex()
                .flex_1()
                .min_w_0()
                .justify_center()
                .overflow_hidden()
                .gap_2()
                .px_3()
                .child(h_flex().flex_shrink_0().gap_1().child("Hit").child(tip(first)))
                .children(tips.map(|t| h_flex().flex_shrink_0().gap_2().child("·").child(tip(t)))),
        )
    }

    fn status_bar(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let tips = self.tips(window, cx);
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
            .children(self.sample.as_ref().map(|sample| self.checklist(sample, cx)))
            .children(self.rate.as_ref().map(|r| {
                div().flex_shrink_0().text_color(rate_color).child(format!("— API {}/{}", thousands(r.remaining), thousands(r.limit)))
            }))
            .children(tips)
            .when(self.user.is_some() && self.sample.is_none(), |el| el.child(self.usage_popover(cx)))
            .children(self.user.as_ref().map(|u| {
                h_flex()
                    .flex_shrink_0()
                    .ml_auto()
                    .gap_1()
                    .child(theme::avatar(&u.login, &u.avatar_url))
                    .child(format!("@{}", u.login))
            }))
            .when(self.sample.is_some(), |el| {
                el.child(
                    Button::new("connect-github")
                        .primary()
                        .xsmall()
                        .label("Connect GitHub")
                        .tooltip("Leave the sample and sign in to review your own pull requests")
                        .on_click(|_, window, cx| sign_out(window, cx)),
                )
            })
            .child(
                Button::new("tips")
                    .ghost()
                    .xsmall()
                    .label("Tips")
                    .selected(appearance::tips(cx))
                    .tooltip_with_action(if appearance::tips(cx) { "Hide quick tips" } else { "Show quick tips" }, &ToggleTips, None)
                    .when(self.user.is_none(), |b| b.ml_auto())
                    .on_click(|_, _, cx| appearance::set_tips(!appearance::tips(cx), cx)),
            )
            .child(
                Button::new("settings")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Settings)
                    .tooltip_with_action("Settings", &OpenSettings, None)
                    .on_click(|_, window, cx| open_settings(window, cx)),
            )
    }

    /// Getting-started steps for the sample; each one ticks when its action is done.
    fn checklist(&self, sample: &Sample, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let steps = sample.steps();
        let done = steps.iter().filter(|(done, ..)| *done).count();
        let rows = steps.into_iter().map(|(done, step, how)| {
            h_flex()
                .gap_2()
                .items_start()
                .py_1()
                .child(div().pt_0p5().child(if done {
                    Icon::new(IconName::Check).xsmall().text_color(theme.success).into_any_element()
                } else {
                    div().size_3().border_1().border_color(theme.border).into_any_element()
                }))
                .child(
                    // flex_1 + min_w_0: the hint wraps at the popover width instead of overflowing it.
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_sm().text_color(if done { theme.muted_foreground } else { theme.foreground }).child(step))
                        .when(!done, |el| el.child(div().text_xs().text_color(theme.muted_foreground).child(how))),
                )
        });
        let footer = if done == steps.len() {
            "All done. Connect GitHub to review your own pull requests."
        } else {
            "Sample data: nothing is sent to GitHub or an AI service."
        };
        h_flex()
            .flex_shrink_0()
            .gap_1()
            .child(div().px_1().text_color(theme.warning).border_1().border_color(theme.warning).child("SAMPLE"))
            .child(
                Popover::new("getting-started")
                    .w(px(320.))
                    .anchor(Anchor::BottomLeft)
                    .trigger(Button::new("getting-started-trigger").ghost().xsmall().label(format!("Getting started {done}/{}", steps.len())))
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().font_semibold().child("Getting started"))
                            .children(rows)
                            .child(div().pt_1().text_xs().text_color(theme.muted_foreground).child(footer))
                            .child(
                                h_flex()
                                    .pt_1()
                                    .gap_1()
                                    .child(
                                        Button::new("sample-connect")
                                            .small()
                                            .primary()
                                            .label("Connect GitHub")
                                            .on_click(|_, window, cx| sign_out(window, cx)),
                                    )
                                    .child(
                                        Button::new("sample-restart")
                                            .small()
                                            .ghost()
                                            .label("Start over")
                                            .tooltip("Reset the sample: comments, reviews, viewed files and AI review")
                                            .on_click(|_, window, cx| start_sample(window, cx)),
                                    ),
                            ),
                    ),
            )
    }

    fn usage_popover(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let rows = self.usage.as_ref().map(|entries| entries.iter().map(|entry| {
            let heading = entry.plan.as_ref().map_or_else(|| entry.backend.label().to_string(), |plan| format!("{} · {plan}", entry.backend.label()));
            v_flex()
                .gap_1()
                .py_2()
                .child(div().text_color(theme.foreground).text_sm().child(heading))
                .children(entry.session.as_ref().map(|window| usage_window("Session", window, muted, theme.foreground)))
                .children(entry.weekly.as_ref().map(|window| usage_window("Weekly", window, muted, theme.foreground)))
                .children(entry.error.as_ref().map(|error| div().text_xs().text_color(muted).child(error.clone())))
                .when(entry.session.is_none() && entry.weekly.is_none() && entry.error.is_none(), |el| {
                    el.child(div().text_xs().text_color(muted).child("No usage data"))
                })
        }).collect::<Vec<_>>()).unwrap_or_default();
        Popover::new("ai-usage")
            .w(px(280.))
            .anchor(Anchor::BottomRight)
            .trigger(Button::new("ai-usage-trigger").ghost().xsmall().label("AI usage"))
            .child(v_flex().gap_1().child(div().font_semibold().child("AI usage"))
                .when(self.usage.is_none(), |el| el.child(div().text_xs().text_color(muted).child("Loading usage…")))
                .when(self.usage.as_ref().is_some_and(Vec::is_empty), |el| el.child(div().text_xs().text_color(muted).child("Install and sign in to Claude Code or Codex to see usage.")))
                .children(rows))
    }
}

fn usage_window(label: &'static str, usage: &WindowUsage, muted: Hsla, foreground: Hsla) -> impl IntoElement {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let reset = usage.resets_at.map(|at| {
        let left = at.saturating_sub(now);
        if left == 0 { "resets now".to_string() }
        else if left >= 86_400 { format!("resets in {}d {}h", left / 86_400, left % 86_400 / 3_600) }
        else if left >= 3_600 { format!("resets in {}h {}m", left / 3_600, left % 3_600 / 60) }
        else { format!("resets in {}m", (left / 60).max(1)) }
    }).unwrap_or_default();
    h_flex().gap_2().text_xs().text_color(muted)
        .child(div().w(px(58.)).child(label))
        .child(div().w(px(36.)).text_color(foreground).child(format!("{:.0}%", usage.used_percent)))
        .child(reset)
}

impl Render for AppRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .when(self.user.is_some(), |el| el.child(self.status_bar(window, cx)))
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
