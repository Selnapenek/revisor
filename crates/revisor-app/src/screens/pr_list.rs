//! Pull requests of one repository: state tabs, client-side search, advanced filters, infinite scroll,
//! keyboard selection.

use crate::app::{self, Route};
use crate::{backend, theme};
use gpui_kit::component::{
    badge::Badge,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    input::{Escape, Input, InputEvent, InputState},
    notification::Notification,
    popover::Popover,
    scroll::ScrollableElement as _,
    spinner::Spinner,
    tab::{Tab, TabBar},
    tag::Tag,
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::cache::queries;
use revisor_core::pr_filters::{self, REVIEW_REQUESTED_ME, REVIEW_STATUS_OPTIONS};
use revisor_core::provider::types::{Label, PrSearchFilters, PrState, PrUser, PullRequest};
use revisor_core::time::relative_time;
use std::{collections::HashMap, ops::Range, time::Duration};

const PER_PAGE: u32 = 30;
const STATES: [PrState; 3] = [PrState::Open, PrState::Closed, PrState::All];
const ROW: &str = "pr-row";

actions!(pr_list, [SelectPrev, SelectNext, OpenSelected]);

pub(crate) const CONTEXT: &str = "PrList";

/// Inputs bind up/down/enter in their own deeper `Input` context, so typing is not affected.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", SelectPrev, Some(CONTEXT)),
        KeyBinding::new("down", SelectNext, Some(CONTEXT)),
        KeyBinding::new("enter", OpenSelected, Some(CONTEXT)),
    ]);
}

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
    /// PR number to when the user last had it open.
    views: HashMap<u64, String>,
    /// Indexes into `prs` matching the search, refreshed on render.
    visible: Vec<usize>,
    /// Number of the PR picked with the arrow keys; survives filtering and paging.
    selected: Option<u64>,
    focus: FocusHandle,
    labels: Vec<Label>,
    authors: Vec<PrUser>,
    assignees: Vec<PrUser>,
    page: u32,
    has_next: bool,
    loading: bool,
    loaded: bool,
    /// A return from PR detail refreshes saved rows without a loading indicator.
    refreshing: bool,
    /// Page 1 failed to load; shown in place of the list.
    error: Option<SharedString>,
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
        let mut subscriptions: Vec<_> = [&search, &author_search, &label_search, &assignee_search]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |_, _, event: &InputEvent, cx| {
                    if let InputEvent::Change = event {
                        cx.notify();
                    }
                })
            })
            .collect();

        let focus = cx.focus_handle();
        focus.focus(window, cx);
        // A focused input went away (filter popover closed): keep the arrow keys working.
        subscriptions.push(cx.on_focus_lost(window, |this, window, cx| this.focus.focus(window, cx)));

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
            views: HashMap::new(),
            visible: Vec::new(),
            selected: None,
            focus,
            labels: Vec::new(),
            authors: Vec::new(),
            assignees: Vec::new(),
            page: 1,
            has_next: false,
            loading: true,
            loaded: false,
            refreshing: false,
            error: None,
            scroll: UniformListScrollHandle::new(),
            _load: load,
            _debounce: Task::ready(()),
            _subscriptions: subscriptions,
        }
    }

    pub fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        self.load(1, window, cx);
        self.refreshing = self.loaded;
    }

    /// Page 1 replaces the list, later pages append. Replacing `_load` drops the stale request.
    fn load(&mut self, page: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.loading = true;
        self.refreshing = false;
        let (owner, repo, state, filters) =
            (self.owner.to_string(), self.repo.to_string(), STATES[self.state], self.filters.clone());
        self._load = backend::spawn(
            window,
            cx,
            move |s| async move {
                let p = s.provider().await?;
                let result = if filters.is_empty() {
                    p.list_pull_requests(&owner, &repo, state, page, PER_PAGE).await
                } else {
                    p.search_pull_requests(&owner, &repo, state, &filters, page, PER_PAGE).await
                }
                .map_err(backend::err)?;
                // Views are non-critical: on failure every PR shows as new.
                let views = queries::get_pr_views(s.cache.lock().await.conn(), &owner, &repo).unwrap_or_default();
                Ok((result, views))
            },
            move |this, result, window, cx| {
                this.loading = false;
                let refreshing = std::mem::take(&mut this.refreshing);
                match result {
                    Ok((result, views)) => {
                        this.views = views;
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
                        this.loaded = true;
                        this.error = None;
                    }
                    Err(e) if refreshing => {
                        window.push_notification(Notification::error(format!("Failed to refresh PRs: {e}")), cx);
                    }
                    // Page 1 replaces the list: drop rows of the previous tab/filters, show the error inline.
                    Err(e) if page == 1 => {
                        this.prs.clear();
                        this.has_next = false;
                        this.loaded = false;
                        this.error = Some(e.into());
                    }
                    Err(e) => window.push_notification(Notification::error(format!("Failed to load more PRs: {e}")), cx),
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

    /// Position of the selected PR among the visible rows.
    fn selected_pos(&self) -> Option<usize> {
        self.visible.iter().position(|&ix| Some(self.prs[ix].number) == self.selected)
    }

    /// Move the selection by `delta` rows; the first press selects the top row.
    fn move_selection(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(last) = self.visible.len().checked_sub(1) else { return };
        let pos = self.selected_pos().map_or(0, |p| p.saturating_add_signed(delta).min(last));
        self.selected = Some(self.prs[self.visible[pos]].number);
        self.scroll.scroll_to_item(pos, ScrollStrategy::Nearest);
        app::preload_pr(Route::PrDetail { owner: self.owner.clone(), repo: self.repo.clone(), number: self.prs[self.visible[pos]].number }, window, cx);
        cx.notify();
    }

    fn open_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pos) = self.selected_pos() {
            let number = self.prs[self.visible[pos]].number;
            app::navigate(Route::PrDetail { owner: self.owner.clone(), repo: self.repo.clone(), number }, window, cx);
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
                    .children(color.map(|c| div().size_2().flex_shrink_0().bg(c)))
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
        let mine = login.as_ref().is_some_and(|l| f.authors == [l.clone()]);
        let awaiting = f.review_status.as_deref() == Some(REVIEW_REQUESTED_ME);
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
                    .selected(mine)
                    .disabled(login.is_none())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if mine {
                            this.update_filters(|f| f.authors.clear(), window, cx);
                        } else if let Some(login) = login.clone() {
                            this.update_filters(|f| *f = pr_filters::with_author_preset(f, &login), window, cx);
                        }
                    })),
            )
            .child(
                Button::new("preset-review")
                    .ghost()
                    .small()
                    .label("Needs review from me")
                    .selected(awaiting)
                    .disabled(app::current_user(cx).is_none())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if awaiting {
                            this.update_filters(|f| f.review_status = None, window, cx);
                        } else {
                            this.update_filters(|f| *f = pr_filters::with_review_requested_preset(f), window, cx);
                        }
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
                let hover_route = route.clone();
                let comments = pr.comments_count + pr.review_comments_count;
                // Both timestamps are `YYYY-MM-DDTHH:MM:SSZ`, so string order is time order.
                let freshness = match self.views.get(&pr.number) {
                    None => Some(theme::stamp("New", theme.primary, theme)),
                    Some(viewed) if pr.updated_at > *viewed => Some(theme::stamp("Updated", theme.warning, theme)),
                    Some(_) => None,
                };
                let selected = self.selected == Some(pr.number);
                h_flex()
                    .id(("pr", pr.number))
                    .group(ROW)
                    .relative()
                    .w_full()
                    .h(rems(3.5))
                    .px_4()
                    .border_b_1()
                    .border_color(theme.border)
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.list_hover))
                    .on_hover(move |hovered, window, cx| {
                        if *hovered { app::preload_pr(hover_route.clone(), window, cx); }
                    })
                    .when(selected, |el| el.bg(theme.list_active))
                    .on_click(move |_, window, cx| app::navigate(route.clone(), window, cx))
                    .child(
                        theme::margin_mark(theme.primary)
                            .when(!selected, |el| el.invisible().group_hover(ROW, |s| s.visible())),
                    )
                    .child(
                        div()
                            .w(px(64.))
                            .flex_shrink_0()
                            .font_family(theme.mono_font_family.clone())
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!("#{}", pr.number)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                h_flex()
                                    .w_full()
                                    .gap_2()
                                    .child(div().flex_1().min_w_0().text_sm().font_medium().truncate().child(pr.title.clone()))
                                    .children(freshness)
                                    .child(theme::pr_state_stamp(pr.merged_at.is_some(), &pr.state, pr.draft, cx)),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .gap_2()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(
                                        h_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .gap_2()
                                            .overflow_hidden()
                                            .child(theme::avatar(&pr.author_login, &pr.author_avatar))
                                            .child(pr.author_login.clone())
                                            .child("·")
                                            .child(relative_time(&pr.updated_at))
                                            .children(pr.labels.iter().map(|l| {
                                                let c = theme::hex(&l.color);
                                                Tag::custom(c.opacity(0.15), c, c.opacity(0.4))
                                                    .small()
                                                    .flex_shrink_0()
                                                    .child(l.name.clone())
                                            })),
                                    )
                                    .child(
                                        h_flex()
                                            .flex_shrink_0()
                                            .gap_2()
                                            .when(comments > 0, |el| {
                                                el.child(format!("{comments} comment{}", if comments == 1 { "" } else { "s" }))
                                            })
                                            .when(pr.additions + pr.deletions > 0, |el| {
                                                el.child(
                                                    h_flex()
                                                        .gap_1()
                                                        .font_family(theme.mono_font_family.clone())
                                                        .child(div().text_color(theme.success).child(format!("+{}", pr.additions)))
                                                        .child(div().text_color(theme.danger).child(format!("−{}", pr.deletions))),
                                                )
                                            }),
                                    ),
                            ),
                    )
            })
            .collect()
    }

    fn render_list(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let body = if self.visible.is_empty() {
            let content = if self.loading && !self.refreshing {
                div().child(Spinner::new().large().color(theme.muted_foreground))
            } else if let Some(error) = self.error.clone() {
                v_flex()
                    .items_center()
                    .gap_2()
                    .max_w(px(480.))
                    .text_sm()
                    .child(div().font_family(theme.mono_font_family.clone()).italic().text_xl().child("Couldn't load pull requests"))
                    .child(div().text_xs().text_center().text_color(theme.muted_foreground).child(error))
                    .child(
                        Button::new("retry")
                            .primary()
                            .small()
                            .label("Retry")
                            .on_click(cx.listener(|this, _, window, cx| this.load(1, window, cx))),
                    )
            } else {
                let filtered = !self.filters.is_empty();
                v_flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(
                        div()
                            .font_family(theme.mono_font_family.clone())
                            .italic()
                            .text_xl()
                            .text_color(theme.foreground)
                            .child(if filtered { "No pull requests match the active filters" } else { "No pull requests found" }),
                    )
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
            .when(self.loading && !self.refreshing && !self.visible.is_empty(), |el| {
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
            .child(
                Button::new("back")
                    .ghost()
                    .small()
                    .label("← Repos")
                    .tooltip_with_action("Back", &app::GoBack, None)
                    .on_click(|_, window, cx| app::go_back(window, cx)),
            )
            .child(
                // One mono line on a shared baseline: muted `owner /`, then the repo, then the count.
                h_flex()
                    .items_baseline()
                    .gap_3()
                    .pl_3()
                    .border_l_1()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .items_baseline()
                            .font_family(theme.mono_font_family.clone())
                            .text_lg()
                            .child(div().text_color(theme.muted_foreground).child(format!("{} / ", self.owner)))
                            .child(div().font_medium().child(self.repo.clone())),
                    )
                    .child(theme::eyebrow(
                        format!("{} pull request{}", self.visible.len(), if self.visible.len() == 1 { "" } else { "s" }),
                        cx,
                    )),
            )
            .child(
                h_flex()
                    .ml_auto()
                    .gap_3()
                    // Escape leaves the search; focus goes back to the arrow-key list.
                    .child(
                        div().on_action(|_: &Escape, window, cx| window.blur(cx)).child(Input::new(&self.search).small().w(px(240.))),
                    )
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
        v_flex()
            .size_full()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &SelectPrev, window, cx| this.move_selection(-1, window, cx)))
            .on_action(cx.listener(|this, _: &SelectNext, window, cx| this.move_selection(1, window, cx)))
            .on_action(cx.listener(|this, _: &OpenSelected, window, cx| this.open_selected(window, cx)))
            .child(header)
            .child(
            div().flex_1().overflow_hidden().mt_2().child(
                theme::island(cx).flex().flex_col().child(self.render_filters(cx)).child(self.render_list(cx)),
            ),
        )
    }
}
