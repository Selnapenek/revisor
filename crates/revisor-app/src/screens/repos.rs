//! Saved repositories and a browser for the user's GitHub repositories.

use crate::app::{self, Route};
use crate::backend;
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState},
    notification::Notification,
    scroll::ScrollableElement as _,
    spinner::Spinner,
    tab::{Tab, TabBar},
    tag::Tag,
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::{cache::queries, provider::types::RepoInfo, time::relative_time};
use std::time::Duration;

const SAVED: usize = 0;
const BROWSE: usize = 1;

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
        let repo_input = cx.new(|cx| InputState::new(window, cx).placeholder("owner/repo or GitHub URL"));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search repositories..."));
        let subscriptions = vec![
            cx.subscribe_in(&repo_input, window, |this, _, event, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.add_repo(window, cx);
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

    fn add_repo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.repo_input.read(cx).value();
        let Some((owner, repo)) = parse_repo(value.trim()) else {
            self.input_error = Some("Enter as owner/repo (e.g. facebook/react)".into());
            cx.notify();
            return;
        };
        self.input_error = None;
        self.save(owner.to_string(), repo.to_string(), false, window, cx);
        self.repo_input.update(cx, |input, cx| input.set_value("", window, cx));
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
        let theme = cx.theme();
        let list = if self.saved.is_empty() {
            div()
                .py_12()
                .text_center()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("No repositories saved yet. Add one above or browse your repos.")
        } else {
            v_flex().gap_1().children(self.saved.iter().map(|(owner, repo)| {
                let id = SharedString::from(format!("{owner}/{repo}"));
                let route = Route::PrList { owner: owner.clone().into(), repo: repo.clone().into() };
                let (o, r) = (owner.clone(), repo.clone());
                h_flex()
                    .id(id.clone())
                    .justify_between()
                    .px_3()
                    .py_2p5()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.list_hover))
                    .on_click(move |_, window, cx| app::navigate(route.clone(), window, cx))
                    .child(
                        h_flex()
                            .text_sm()
                            .child(div().text_color(theme.muted_foreground).child(owner.clone()))
                            .child(div().mx_0p5().text_color(theme.muted_foreground).child("/"))
                            .child(div().font_medium().child(repo.clone())),
                    )
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
                            .label("Add")
                            .on_click(cx.listener(|this, _, window, cx| this.add_repo(window, cx))),
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
                .child(v_flex().gap_1().children(self.browse.iter().map(|repo| {
                    let saved = self.saved.iter().any(|(o, r)| *o == repo.owner && *r == repo.name);
                    let (owner, name) = (repo.owner.clone(), repo.name.clone());
                    h_flex()
                        .id(SharedString::from(repo.full_name.clone()))
                        .gap_3()
                        .px_3()
                        .py_2p5()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.list_hover))
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
                                        .child(div().text_sm().font_medium().truncate().child(repo.full_name.clone()))
                                        .when(repo.private, |el| el.child(Tag::warning().small().child("Private")))
                                        .when(repo.fork, |el| el.child(Tag::secondary().small().child("Fork")))
                                        .when(saved, |el| el.child(Tag::success().small().child("Saved"))),
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
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .child(
                h_flex()
                    .gap_3()
                    .child(div().text_sm().font_semibold().child("Repositories"))
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
                    .label("Sign out")
                    .on_click(|_, window, cx| app::sign_out(window, cx)),
            );
        let content = if self.tab == SAVED { self.render_saved(cx) } else { self.render_browse(cx) };
        v_flex().size_full().child(header).child(
            div()
                .flex_1()
                .overflow_hidden()
                .child(div().size_full().overflow_y_scrollbar().p_4().child(div().max_w(px(672.)).mx_auto().child(content))),
        )
    }
}

/// Parse `owner/repo` or `https://github.com/owner/repo[/]`.
/// Port of `/^(?:https?:\/\/github\.com\/)?([^/\s]+)\/([^/\s]+?)\/?$/`.
fn parse_repo(value: &str) -> Option<(&str, &str)> {
    let rest = ["https://github.com/", "http://github.com/"]
        .iter()
        .find_map(|p| value.strip_prefix(p))
        .unwrap_or(value);
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    let (owner, repo) = rest.split_once('/')?;
    let ok = |s: &str| !s.is_empty() && !s.contains(|c: char| c == '/' || c.is_whitespace());
    (ok(owner) && ok(repo)).then_some((owner, repo))
}

#[cfg(test)]
mod tests {
    use super::parse_repo;

    #[test]
    fn parses_repo_input() {
        assert_eq!(parse_repo("facebook/react"), Some(("facebook", "react")));
        assert_eq!(parse_repo("facebook/react/"), Some(("facebook", "react")));
        assert_eq!(parse_repo("https://github.com/facebook/react"), Some(("facebook", "react")));
        assert_eq!(parse_repo("http://github.com/facebook/react/"), Some(("facebook", "react")));
        assert_eq!(parse_repo("facebook"), None);
        assert_eq!(parse_repo("facebook/react//"), None);
        assert_eq!(parse_repo("a/b/c"), None);
        assert_eq!(parse_repo("https://github.com/facebook"), None);
        assert_eq!(parse_repo("https://github.com/facebook/react/pulls"), None);
        assert_eq!(parse_repo("face book/react"), None);
        assert_eq!(parse_repo("/react"), None);
        assert_eq!(parse_repo(""), None);
    }
}
