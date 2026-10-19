//! Saved repositories and a browser for the user's GitHub repositories.

use crate::app::{self, Route};
use crate::{backend, theme};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState},
    notification::Notification,
    scroll::ScrollableElement as _,
    spinner::Spinner,
    tab::{Tab, TabBar},
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::{cache::queries, provider::types::RepoInfo, time::relative_time};
use std::time::Duration;

const SAVED: usize = 0;
const BROWSE: usize = 1;
const ROW: &str = "repo-row";

pub struct ReposScreen {
    tab: usize,
    repo_input: Entity<InputState>,
    input_error: Option<SharedString>,
    saved: Vec<(String, String)>,
    search: Entity<InputState>,
    browse: Vec<RepoInfo>,
    browse_loading: bool,
    page: u32,
    has_next: bool,
    _load: Task<()>,
    _debounce: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl ReposScreen {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let repo_input = cx.new(|cx| InputState::new(window, cx).placeholder("owner/repo, repository link, or pull request link"));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search repositories..."));
        let subscriptions = vec![
            cx.subscribe_in(&repo_input, window, |this, _, event, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.submit(window, cx);
                }
            }),
            cx.subscribe_in(&search, window, |this, _, event, window, cx| {
                if let InputEvent::Change = event {
                    this._debounce = cx.spawn_in(window, async move |this, cx| {
                        cx.background_executor().timer(Duration::from_millis(300)).await;
                        this.update_in(cx, |this, window, cx| this.load_browse(1, window, cx)).ok();
                    });
                }
            }),
        ];
        backend::spawn(
            window,
            cx,
            |s| async move {
                let cache = s.cache.lock().await;
                queries::get_saved_repos(cache.conn()).map_err(backend::err)
            },
            |this: &mut Self, result, window, cx| match result {
                // First run: nothing saved yet, so show the user's repos instead of an empty tab.
                Ok(saved) if saved.is_empty() && this.tab == SAVED => this.select_tab(BROWSE, window, cx),
                Ok(saved) => this.saved = saved,
                Err(e) => window.push_notification(Notification::error(format!("Failed to load saved repos: {e}")), cx),
            },
        )
        .detach();
        Self {
            tab: SAVED,
            repo_input,
            input_error: None,
            saved: Vec::new(),
            search,
            browse: Vec::new(),
            browse_loading: false,
            page: 1,
            has_next: false,
            _load: Task::ready(()),
            _debounce: Task::ready(()),
            _subscriptions: subscriptions,
        }
    }

    fn select_tab(&mut self, tab: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.tab = tab;
        if tab == BROWSE && self.browse.is_empty() {
            self.load_browse(1, window, cx);
        }
        cx.notify();
    }

    /// Page 1 replaces the list, later pages append.
    fn load_browse(&mut self, page: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.browse_loading = true;
        let query = self.search.read(cx).value().trim().to_string();
        self._load = backend::spawn(
            window,
            cx,
            move |s| async move {
                let p = s.provider().await?;
                if query.is_empty() { p.list_user_repos(page, 30).await } else { p.search_repos(&query, page, 20).await }
                    .map_err(backend::err)
            },
            move |this, result, window, cx| {
                this.browse_loading = false;
                match result {
                    Ok(result) => {
                        if page == 1 {
                            this.browse = result.items;
                        } else {
                            this.browse.extend(result.items);
                        }
                        this.page = page;
                        this.has_next = result.has_next_page;
                    }
                    Err(e) => window.push_notification(Notification::error(format!("Failed to load repos: {e}")), cx),
                }
            },
        );
        cx.notify();
    }

    /// Save a repo, or open a pull request link without saving (access is unknown until it loads).
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.repo_input.read(cx).value();
        match parse_target(value.trim()) {
            Err(e) => {
                self.input_error = Some(e.into());
                cx.notify();
            }
            Ok(Target::Pr(owner, repo, number)) => {
                let route = Route::PrDetail { owner: owner.to_string().into(), repo: repo.to_string().into(), number };
                app::navigate(route, window, cx);
            }
            Ok(Target::Repo(owner, repo)) => {
                self.input_error = None;
                self.save(owner.to_string(), repo.to_string(), false, window, cx);
                self.repo_input.update(cx, |input, cx| input.set_value("", window, cx));
            }
        }
    }

    /// Save (or bump) a repo, move it to the top of the saved list, and optionally open it.
    fn save(&mut self, owner: String, repo: String, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (o, r) = (owner.clone(), repo.clone());
        backend::spawn(
            window,
            cx,
            move |s| async move {
                let cache = s.cache.lock().await;
                queries::save_repo(cache.conn(), &o, &r).map_err(backend::err)
            },
            move |this, result, window, cx| {
                match result {
                    Ok(()) => {
                        this.saved.retain(|(o, r)| !(*o == owner && *r == repo));
                        this.saved.insert(0, (owner.clone(), repo.clone()));
                    }
                    Err(e) => window.push_notification(Notification::error(format!("Failed to save repo: {e}")), cx),
                }
                if open {
                    app::navigate(Route::PrList { owner: owner.into(), repo: repo.into() }, window, cx);
                }
            },
        )
        .detach();
    }

    fn remove(&mut self, owner: String, repo: String, window: &mut Window, cx: &mut Context<Self>) {
        let (o, r) = (owner.clone(), repo.clone());
        backend::spawn(
            window,
            cx,
            move |s| async move {
                let cache = s.cache.lock().await;
                queries::remove_repo(cache.conn(), &o, &r).map_err(backend::err)
            },
            move |this, result, window, cx| match result {
                Ok(()) => this.saved.retain(|(o, r)| !(*o == owner && *r == repo)),
                Err(e) => window.push_notification(Notification::error(format!("Failed to remove repo: {e}")), cx),
            },
        )
        .detach();
    }

    fn render_saved(&self, cx: &mut Context<Self>) -> Div {
        let is_pr = matches!(parse_target(self.repo_input.read(cx).value().trim()), Ok(Target::Pr(..)));
        let theme = cx.theme();
        let list = if self.saved.is_empty() {
            v_flex()
                .py_12()
                .gap_1()
                .items_center()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(div().font_family(theme.mono_font_family.clone()).italic().text_xl().text_color(theme.foreground).child("No repositories saved yet."))
                .child("Add one above or browse your repos.")
        } else {
            v_flex().children(self.saved.iter().map(|(owner, repo)| {
                let id = SharedString::from(format!("{owner}/{repo}"));
                let route = Route::PrList { owner: owner.clone().into(), repo: repo.clone().into() };
                let (o, r) = (owner.clone(), repo.clone());
                ledger_row(cx)
                    .id(id.clone())
                    .justify_between()
                    .on_click(move |_, window, cx| app::navigate(route.clone(), window, cx))
                    .child(repo_name(owner, repo, cx))
                    .child(
                        Button::new(SharedString::from(format!("remove:{id}"))).ghost().xsmall().label("Remove").on_click(cx.listener(
                            move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.remove(o.clone(), r.clone(), window, cx);
                            },
                        )),
                    )
            }))
        };
        v_flex()
            .child(
                h_flex()
                    .gap_2()
                    .mb_6()
                    .items_start()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(Input::new(&self.repo_input))
                            .children(self.input_error.clone().map(|e| div().text_xs().text_color(theme.danger).child(e))),
                    )
                    .child(
                        Button::new("add-repo")
                            .primary()
                            .label(if is_pr { "Open" } else { "Add" })
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                    ),
            )
            .child(list)
    }

    fn render_browse(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let body = if self.browse_loading && self.browse.is_empty() {
            div().flex().justify_center().py_12().child(Spinner::new().large().color(theme.muted_foreground))
        } else {
            v_flex()
                .child(v_flex().children(self.browse.iter().map(|repo| {
                    let saved = self.saved.iter().any(|(o, r)| *o == repo.owner && *r == repo.name);
                    let (owner, name) = (repo.owner.clone(), repo.name.clone());
                    ledger_row(cx)
                        .id(SharedString::from(repo.full_name.clone()))
                        .gap_3()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.save(owner.clone(), name.clone(), true, window, cx)
                        }))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(repo_name(&repo.owner, &repo.name, cx))
                                        .when(repo.private, |el| el.child(theme::stamp("Private", theme.warning, theme)))
                                        .when(repo.fork, |el| el.child(theme::stamp("Fork", theme.muted_foreground, theme)))
                                        .when(saved, |el| el.child(theme::stamp("Saved", theme.success, theme))),
                                )
                                .children(repo.description.clone().filter(|d| !d.is_empty()).map(|d| {
                                    div().mt_0p5().text_xs().text_color(theme.muted_foreground).truncate().child(d)
                                })),
                        )
                        .child(
                            h_flex()
                                .flex_shrink_0()
                                .gap_3()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!("★ {}", repo.stargazers_count))
                                .child(relative_time(&repo.updated_at)),
                        )
                })))
                .when(self.has_next, |el| {
                    el.child(
                        div().flex().justify_center().py_4().child(
                            Button::new("load-more")
                                .secondary()
                                .small()
                                .label("Load more")
                                .loading(self.browse_loading)
                                .on_click(cx.listener(|this, _, window, cx| this.load_browse(this.page + 1, window, cx))),
                        ),
                    )
                })
        };
        v_flex().child(div().mb_4().child(Input::new(&self.search))).child(body)
    }
}

impl Render for ReposScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let header = h_flex()
            .flex_shrink_0()
            .justify_between()
            .px_4()
            .py_3()
            .child(
                h_flex()
                    .gap_3()
                    .child(div().font_family(theme.mono_font_family.clone()).text_2xl().child("Repositories"))
                    .child(theme::eyebrow(format!("{} saved", self.saved.len()), cx))
                    .child(
                        TabBar::new("repo-tabs")
                            .segmented()
                            .small()
                            .selected_index(self.tab)
                            .on_click(cx.listener(|this, ix: &usize, window, cx| this.select_tab(*ix, window, cx)))
                            .child(Tab::new().label("Saved"))
                            .child(Tab::new().label("Browse")),
                    ),
            )
            // The status bar already shows who is signed in.
            .child(
                Button::new("sign-out")
                    .ghost()
                    .small()
                    .label(if app::in_sample(cx) { "Leave sample" } else { "Sign out" })
                    .on_click(|_, window, cx| app::sign_out(window, cx)),
            );
        let content = if self.tab == SAVED { self.render_saved(cx) } else { self.render_browse(cx) };
        v_flex().size_full().child(header).child(
            div()
                .flex_1()
                .overflow_hidden()
                .mt_2()
                .child(theme::island(cx).child(
                    div().size_full().overflow_y_scrollbar().p_4().child(div().max_w(px(672.)).mx_auto().child(content)),
                )),
        )
    }
}

/// Ruled ledger line with a primary margin mark on hover.
fn ledger_row(cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .group(ROW)
        .relative()
        .px_3()
        .py_2p5()
        .border_b_1()
        .border_color(theme.border)
        .cursor_pointer()
        .hover(|s| s.bg(theme.list_hover))
        .child(theme::margin_mark(theme.primary).invisible().group_hover(ROW, |s| s.visible()))
}

fn repo_name(owner: &str, repo: &str, cx: &App) -> Div {
    h_flex()
        .min_w_0()
        .text_sm()
        .child(div().flex_shrink_0().text_color(cx.theme().muted_foreground).child(format!("{owner} /")))
        .child(div().ml_1().font_medium().truncate().child(repo.to_string()))
}

#[derive(Debug, PartialEq)]
enum Target<'a> {
    Repo(&'a str, &'a str),
    Pr(&'a str, &'a str, u64),
}

const BAD_INPUT: &str = "Enter owner/repo, a repository link, or a pull request link";
const BAD_PR: &str = "Not a valid pull request link, e.g. https://github.com/owner/repo/pull/123";

/// Parse `owner/repo`, `owner/repo#123`, or a github.com repository or pull request link
/// (`…/pull/123/files?w=1#diff` too).
fn parse_target(value: &str) -> Result<Target<'_>, &'static str> {
    let unschemed = value.strip_prefix("https://").or_else(|| value.strip_prefix("http://"));
    let rest = unschemed.unwrap_or(value);
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    // GitHub owners have no dots, so a dotted first segment is a host.
    let (path, is_link) = match rest.split_once('/').unwrap_or((rest, "")) {
        ("github.com", path) => (path, true),
        (first, _) if unschemed.is_some() || first.contains('.') => return Err("Only github.com links are supported"),
        _ => (rest, false),
    };
    let (path, fragment) = path.split_once('#').map_or((path, None), |(p, f)| (p, Some(f)));
    let path = path.split_once('?').map_or(path, |(p, _)| p);
    let path = path.strip_suffix('/').unwrap_or(path);
    let parts: Vec<&str> = path.split('/').collect();
    let ok = |s: &str| !s.is_empty() && !s.contains(char::is_whitespace);
    let number = |n: &str| n.parse::<u64>().ok().filter(|&n| n > 0).ok_or(BAD_PR);
    match parts[..] {
        [owner, repo, ref rest @ ..] if ok(owner) && ok(repo) => match (rest, fragment) {
            ([], Some(n)) if !is_link => Ok(Target::Pr(owner, repo, number(n)?)),
            ([], _) => Ok(Target::Repo(owner, repo)),
            (["pull", n, ..], _) => Ok(Target::Pr(owner, repo, number(n)?)),
            (["pull"], _) => Err(BAD_PR),
            _ => Err(BAD_INPUT),
        },
        _ => Err(BAD_INPUT),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_target, Target::*, BAD_INPUT, BAD_PR};

    #[test]
    fn parses_repo_input() {
        assert_eq!(parse_target("facebook/react"), Ok(Repo("facebook", "react")));
        assert_eq!(parse_target("facebook/react/"), Ok(Repo("facebook", "react")));
        assert_eq!(parse_target("https://github.com/facebook/react"), Ok(Repo("facebook", "react")));
        assert_eq!(parse_target("http://github.com/facebook/react/"), Ok(Repo("facebook", "react")));
        assert_eq!(parse_target("https://www.github.com/facebook/react#readme"), Ok(Repo("facebook", "react")));
        assert_eq!(parse_target("github.com/facebook/react"), Ok(Repo("facebook", "react")));
        for bad in ["facebook", "facebook/react//", "a/b/c", "https://github.com/facebook", "https://github.com/facebook/react/pulls", "face book/react", "/react", ""] {
            assert_eq!(parse_target(bad), Err(BAD_INPUT), "{bad}");
        }
    }

    #[test]
    fn parses_pull_request_input() {
        for good in [
            "https://github.com/facebook/react/pull/123",
            "https://github.com/facebook/react/pull/123/",
            "https://github.com/facebook/react/pull/123/files?w=1#diff-abc",
            "https://github.com/facebook/react/pull/123/commits/abc123",
            "github.com/facebook/react/pull/123",
            "facebook/react/pull/123",
            "facebook/react#123",
        ] {
            assert_eq!(parse_target(good), Ok(Pr("facebook", "react", 123)), "{good}");
        }
        for bad in ["https://github.com/facebook/react/pull", "https://github.com/facebook/react/pull/abc", "facebook/react/pull/0", "facebook/react#x"] {
            assert_eq!(parse_target(bad), Err(BAD_PR), "{bad}");
        }
        for other in ["https://gitlab.com/a/b/-/merge_requests/1", "https://example.com/a/b", "gitlab.com/a/b"] {
            assert_eq!(parse_target(other), Err("Only github.com links are supported"), "{other}");
        }
    }
}
