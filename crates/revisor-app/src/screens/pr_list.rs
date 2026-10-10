//! Pull requests of one repository: state tabs, client-side search, advanced filters, infinite scroll.

use crate::app::{self, Route};
use crate::{backend, theme};
use gpui_kit::component::{
    badge::Badge,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
    notification::Notification,
    popover::Popover,
    scroll::ScrollableElement as _,
    spinner::Spinner,
    tab::{Tab, TabBar},
    tag::Tag,
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::pr_filters::{self, REVIEW_STATUS_OPTIONS};
use revisor_core::provider::types::{Label, PrSearchFilters, PrState, PrUser, PullRequest};
use revisor_core::time::relative_time;
use std::{ops::Range, time::Duration};

const PER_PAGE: u32 = 30;
const STATES: [PrState; 3] = [PrState::Open, PrState::Closed, PrState::All];

/// `(value, label, color, checked)` of a dropdown option.
type DropdownOption = (String, String, Option<Hsla>, bool);

pub struct PrListScreen {
    owner: SharedString,
    repo: SharedString,
    /// Index into `STATES`.
    state: usize,
    filters: PrSearchFilters,
    search: Entity<InputState>,
    author_search: Entity<InputState>,
    label_search: Entity<InputState>,
    assignee_search: Entity<InputState>,
    prs: Vec<PullRequest>,
    /// Indexes into `prs` matching the search, refreshed on render.
    visible: Vec<usize>,
    labels: Vec<Label>,
    authors: Vec<PrUser>,
    assignees: Vec<PrUser>,
    page: u32,
    has_next: bool,
    loading: bool,
    scroll: UniformListScrollHandle,
    _load: Task<()>,
    _debounce: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl PrListScreen {
    pub fn new(owner: SharedString, repo: SharedString, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = |placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let search = input("Filter pull requests...", window, cx);
        let author_search = input("Filter authors...", window, cx);
        let label_search = input("Filter labels...", window, cx);
        let assignee_search = input("Filter assignees...", window, cx);
        let subscriptions = [&search, &author_search, &label_search, &assignee_search]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |_, _, event: &InputEvent, cx| {
                    if let InputEvent::Change = event {
                        cx.notify();
                    }
                })
            })
            .collect();

        let key = format!("{owner}/{repo}");
        let load = backend::spawn(
            window,
            cx,
            move |s| async move { Ok(pr_filters::load(s.cache.lock().await.conn(), &key)) },
            |this: &mut Self, result, window, cx| {
                this.filters = result.unwrap_or_default();
                this.load(1, window, cx);
            },
        );
        let (o, r) = (owner.to_string(), repo.to_string());
        backend::spawn(
            window,
            cx,
            move |s| async move { s.provider().await?.list_repo_labels(&o, &r).await.map_err(backend::err) },
            // Labels are non-critical: on failure the dropdown just stays empty.
            |this: &mut Self, result, _, _| this.labels = result.unwrap_or_default(),
        )
        .detach();

        Self {
            owner,
            repo,
            state: 0,
            filters: PrSearchFilters::default(),
            search,
            author_search,
            label_search,
            assignee_search,
            prs: Vec::new(),
            visible: Vec::new(),
            labels: Vec::new(),
            authors: Vec::new(),
            assignees: Vec::new(),
            page: 1,
            has_next: false,
            loading: true,
            scroll: UniformListScrollHandle::new(),
            _load: load,
            _debounce: Task::ready(()),
            _subscriptions: subscriptions,
        }
    }

    /// Page 1 replaces the list, later pages append. Replacing `_load` drops the stale request.
    fn load(&mut self, page: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.loading = true;
        let (owner, repo, state, filters) =
            (self.owner.to_string(), self.repo.to_string(), STATES[self.state], self.filters.clone());
        self._load = backend::spawn(
            window,
            cx,
            move |s| async move {
                let p = s.provider().await?;
                if filters.is_empty() {
                    p.list_pull_requests(&owner, &repo, state, page, PER_PAGE).await
                } else {
                    p.search_pull_requests(&owner, &repo, state, &filters, page, PER_PAGE).await
                }
                .map_err(backend::err)
            },
            move |this, result, window, cx| {
                this.loading = false;
                match result {
                    Ok(result) => {
                        pr_filters::merge_users(
                            &mut this.authors,
                            result.items.iter().map(|pr| PrUser {
                                login: pr.author_login.clone(),
                                avatar_url: pr.author_avatar.clone(),
                            }),
                        );
                        pr_filters::merge_users(
                            &mut this.assignees,
                            result.items.iter().flat_map(|pr| pr.assignees.clone()),
                        );
                        if page == 1 {
                            this.prs = result.items;
                            this.scroll.scroll_to_item(0, ScrollStrategy::Top);
                        } else {
                            this.prs.extend(result.items);
                        }
                        this.page = page;
                        this.has_next = result.has_next_page;
                    }
                    Err(e) => window.push_notification(Notification::error(format!("Failed to load PRs: {e}")), cx),
                }
            },
        );
        cx.notify();
    }

    fn load_more(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_next && !self.loading {
            self.load(self.page + 1, window, cx);
        }
    }

    fn select_state(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.state = ix;
        self.load(1, window, cx);
    }

    /// Apply a filter change: persist it and reload after a 300 ms debounce.
    fn update_filters(&mut self, f: impl FnOnce(&mut PrSearchFilters), window: &mut Window, cx: &mut Context<Self>) {
        f(&mut self.filters);
        let (key, filters) = (format!("{}/{}", self.owner, self.repo), self.filters.clone());
        backend::spawn(
            window,
            cx,
            move |s| async move {
                pr_filters::save(s.cache.lock().await.conn(), &key, &filters);
                Ok(())
            },
            |_, _: Result<(), String>, _, _| {},
        )
        .detach();
        self._debounce = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(300)).await;
            this.update_in(cx, |this, window, cx| this.load(1, window, cx)).ok();
        });
        cx.notify();
    }

    fn clear_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_filters(|f| *f = PrSearchFilters::default(), window, cx);
    }

    /// Multi-select dropdown; `toggle` applies a click on option `value`.
    fn dropdown(
        &self,
        id: &'static str,
        label: &'static str,
        search: Option<&Entity<InputState>>,
        options: Vec<DropdownOption>,
        toggle: fn(&mut PrSearchFilters, &str),
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let query = search.map(|s| s.read(cx).value().to_lowercase()).unwrap_or_default();
        let count = options.iter().filter(|o| o.3).count();
        let items: Vec<_> = options
            .into_iter()
            .filter(|(_, label, ..)| label.to_lowercase().contains(&query))
            .map(|(value, label, color, checked)| {
                h_flex()
                    .gap_2()
                    .py_1()
                    .child(
                        Checkbox::new(SharedString::from(format!("{id}:{value}")))
                            .checked(checked)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.update_filters(|f| toggle(f, &value), window, cx)
                            })),
                    )
                    .children(color.map(|c| div().size_3().flex_shrink_0().rounded_full().bg(c)))
                    .child(div().text_xs().truncate().child(label))
            })
            .collect();
        let empty = items.is_empty();
        let muted = cx.theme().muted_foreground;
        Badge::new().count(count).child(
            Popover::new(id)
                .w(px(240.))
                .trigger(
                    Button::new(SharedString::from(format!("{id}-trigger")))
                        .outline()
                        .small()
                        .label(label)
                        .dropdown_caret(true)
                        .selected(count > 0),
                )
                .when_some(search, |p, s| {
                    p.track_focus(&s.focus_handle(cx)).child(div().mb_2().child(Input::new(s).small()))
                })
                .child(v_flex().max_h(px(260.)).overflow_y_scrollbar().children(items))
                .when(empty, |p| p.child(div().py_3().text_center().text_xs().text_color(muted).child("No matches"))),
        )
    }

    fn render_filters(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let login = app::current_user(cx).map(|u| u.login);
        let f = &self.filters;
        let user_options = |users: &[PrUser], selected: &[String]| -> Vec<DropdownOption> {
            users.iter().map(|u| (u.login.clone(), u.login.clone(), None, selected.contains(&u.login))).collect()
        };
        let mut authors = self.authors.clone();
        if let Some(login) = &login {
            pr_filters::merge_users(&mut authors, [PrUser { login: login.clone(), avatar_url: String::new() }]);
        }
        let label_options = self
            .labels
            .iter()
            .map(|l| (l.name.clone(), l.name.clone(), Some(theme::hex(&l.color)), f.labels.contains(&l.name)))
            .collect();
        let review_options = REVIEW_STATUS_OPTIONS
            .iter()
            .map(|(value, label)| (value.to_string(), label.to_string(), None, f.review_status.as_deref() == Some(*value)))
            .collect();
        let theme = cx.theme();

        h_flex()
            .flex_shrink_0()
            .flex_wrap()
            .gap_2()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .child(
                Button::new("preset-mine")
                    .ghost()
                    .small()
                    .label("Authored by me")
                    .disabled(login.is_none())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(login) = login.clone() {
                            this.update_filters(|f| *f = pr_filters::with_author_preset(f, &login), window, cx);
                        }
                    })),
            )
            .child(
                Button::new("preset-review")
                    .ghost()
                    .small()
                    .label("Needs review from me")
                    .disabled(app::current_user(cx).is_none())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.update_filters(|f| *f = pr_filters::with_review_requested_preset(f), window, cx)
                    })),
            )
            .child(self.dropdown(
                "author",
                "Author",
                Some(&self.author_search),
                user_options(&authors, &f.authors),
                |f, v| pr_filters::toggle(&mut f.authors, v),
                cx,
            ))
            .child(self.dropdown(
                "label",
                "Label",
                Some(&self.label_search),
                label_options,
                |f, v| pr_filters::toggle(&mut f.labels, v),
                cx,
            ))
            .child(self.dropdown(
                "assignee",
                "Assignee",
                Some(&self.assignee_search),
                user_options(&self.assignees, &f.assignees),
                |f, v| pr_filters::toggle(&mut f.assignees, v),
                cx,
            ))
            .child(self.dropdown(
                "review",
                "Reviews",
                None,
                review_options,
                |f, v| f.review_status = (f.review_status.as_deref() != Some(v)).then(|| v.to_string()),
                cx,
            ))
            .when(!f.is_empty(), |el| {
                el.child(
                    Button::new("clear-filters")
                        .ghost()
                        .small()
                        .label("Clear filters")
                        .on_click(cx.listener(|this, _, window, cx| this.clear_filters(window, cx))),
                )
            })
    }

    fn render_rows(&mut self, range: Range<usize>, window: &mut Window, cx: &mut Context<Self>) -> Vec<Stateful<Div>> {
        if range.end + 5 >= self.visible.len() && self.scroll.is_scrollable() {
            cx.defer_in(window, |this, window, cx| this.load_more(window, cx));
        }
        let theme = cx.theme();
        self.visible[range]
            .iter()
            .map(|&ix| {
                let pr = &self.prs[ix];
                let route = Route::PrDetail { owner: self.owner.clone(), repo: self.repo.clone(), number: pr.number };
                let state = if pr.merged_at.is_some() {
                    Tag::color(ColorName::Purple).child("Merged")
                } else if pr.state == "closed" {
                    Tag::color(ColorName::Red).child("Closed")
                } else if pr.draft {
                    Tag::secondary().child("Draft")
                } else {
                    Tag::color(ColorName::Green).child("Open")
                };
                let comments = pr.comments_count + pr.review_comments_count;
                v_flex()
                    .id(("pr", pr.number))
                    .w_full()
                    .h(px(56.))
                    .justify_center()
                    .gap_1()
                    .px_4()
                    .border_b_1()
                    .border_color(theme.border)
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.list_hover))
                    .on_click(move |_, window, cx| app::navigate(route.clone(), window, cx))
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .child(state.small().flex_shrink_0())
                            .child(div().flex_1().min_w_0().text_sm().font_medium().truncate().child(pr.title.clone()))
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("#{}", pr.number)),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .overflow_hidden()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(pr.author_login.clone())
                            .child("·")
                            .child(relative_time(&pr.updated_at))
                            .when(comments > 0, |el| {
                                el.child("·").child(format!("{comments} comment{}", if comments == 1 { "" } else { "s" }))
                            })
                            .children(pr.labels.iter().map(|l| {
                                let c = theme::hex(&l.color);
                                Tag::custom(c.opacity(0.15), c, c.opacity(0.4))
                                    .small()
                                    .rounded_full()
                                    .flex_shrink_0()
                                    .child(l.name.clone())
                            })),
                    )
            })
            .collect()
    }

    fn render_list(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let body = if self.visible.is_empty() {
            let content = if self.loading {
                div().child(Spinner::new().large().color(theme.muted_foreground))
            } else {
                let filtered = !self.filters.is_empty();
                v_flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(if filtered { "No pull requests match the active filters" } else { "No pull requests found" })
                    .when(filtered, |el| {
                        el.child(
                            Button::new("empty-clear")
                                .link()
                                .small()
                                .label("Clear filters")
                                .on_click(cx.listener(|this, _, window, cx| this.clear_filters(window, cx))),
                        )
                    })
            };
            div().size_full().flex().items_center().justify_center().child(content)
        } else {
            div()
                .size_full()
                .child(
                    uniform_list("prs", self.visible.len(), cx.processor(Self::render_rows))
                        .track_scroll(&self.scroll)
                        .size_full(),
                )
                .vertical_scrollbar(&self.scroll)
        };
        v_flex()
            .flex_1()
            .overflow_hidden()
            .child(div().flex_1().overflow_hidden().child(body))
            .when(self.loading && !self.visible.is_empty(), |el| {
                el.child(div().flex().justify_center().py_2().child(Spinner::new().color(theme.muted_foreground)))
            })
    }
}

impl Render for PrListScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.visible = pr_filters::filter_prs(&self.prs, self.search.read(cx).value().trim());
        let theme = cx.theme();
        let header = h_flex()
            .flex_shrink_0()
            .gap_3()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .child(
                Button::new("back")
                    .ghost()
                    .small()
                    .label("← Repos")
                    .on_click(|_, window, cx| app::navigate(Route::Repos, window, cx)),
            )
            .child(
                h_flex()
                    .text_sm()
                    .child(div().text_color(theme.muted_foreground).child(self.owner.clone()))
                    .child(div().mx_0p5().text_color(theme.muted_foreground).child("/"))
                    .child(div().font_semibold().child(self.repo.clone())),
            )
            .child(
                h_flex()
                    .ml_auto()
                    .gap_3()
                    .child(Input::new(&self.search).small().w(px(240.)))
                    .child(
                        TabBar::new("pr-state")
                            .segmented()
                            .small()
                            .selected_index(self.state)
                            .on_click(cx.listener(|this, ix: &usize, window, cx| this.select_state(*ix, window, cx)))
                            .child(Tab::new().label("Open"))
                            .child(Tab::new().label("Closed"))
                            .child(Tab::new().label("All")),
                    ),
            );
        v_flex().size_full().child(header).child(self.render_filters(cx)).child(self.render_list(cx))
    }
}
