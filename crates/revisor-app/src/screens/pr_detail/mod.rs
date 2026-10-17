//! PR detail: header, resizable sidebar (files / conversations / semantic) and the diff.

mod code_text;
mod comments;
mod commits;
mod conversations;
mod diff;
mod files;
mod markdown_editor;
mod review;
mod semantic;
mod syntax;
pub mod state;

use crate::app;
use crate::{backend, theme, video};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    kbd::Kbd,
    resizable::{h_resizable, resizable_panel},
    spinner::Spinner,
    tab::{Tab, TabBar},
    scroll::ScrollableElement as _,
    text::TextView,
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::{cache::queries, time::relative_time};
use state::PrModel;
use std::rc::Rc;

actions!(
    pr_detail,
    [
        ToggleSearch,
        FocusFileFilter,
        ToggleConversations,
        PrevFile,
        NextFile,
        CloseSearch,
        NextChange,
        PrevChange,
        NextThread,
        PrevThread,
        NextUnreviewed,
        PrevUnreviewed,
        ToggleViewed,
        ShowShortcuts,
    ]
);

pub(crate) const CONTEXT: &str = "PrDetail";
/// Letter keys must not fire while typing in an input.
const NOT_TYPING: &str = "PrDetail && !Input";
const SIDEBAR_WIDTH_KEY: &str = "sidebar-width";
const SIDEBAR_DEFAULT: f32 = 260.;
const DESCRIPTION_WIDTH: f32 = 340.;

pub fn init(cx: &mut App) {
    markdown_editor::init(cx);
    cx.bind_keys([
        KeyBinding::new("cmd-f", ToggleSearch, Some(CONTEXT)),
        KeyBinding::new("cmd-p", FocusFileFilter, Some(CONTEXT)),
        KeyBinding::new("cmd-shift-c", ToggleConversations, Some(CONTEXT)),
        KeyBinding::new("left", PrevFile, Some(CONTEXT)),
        KeyBinding::new("right", NextFile, Some(CONTEXT)),
        KeyBinding::new("escape", CloseSearch, Some(CONTEXT)),
        KeyBinding::new("j", NextChange, Some(NOT_TYPING)),
        KeyBinding::new("k", PrevChange, Some(NOT_TYPING)),
        KeyBinding::new("n", NextThread, Some(NOT_TYPING)),
        KeyBinding::new("shift-n", PrevThread, Some(NOT_TYPING)),
        KeyBinding::new("u", NextUnreviewed, Some(NOT_TYPING)),
        KeyBinding::new("shift-u", PrevUnreviewed, Some(NOT_TYPING)),
        KeyBinding::new("v", ToggleViewed, Some(NOT_TYPING)),
        KeyBinding::new("?", ShowShortcuts, Some(NOT_TYPING)),
    ]);
}

/// Keys are read from the live bindings, so the list cannot drift from `init`.
/// Context lookup, not focus: Settings opens it too, with no PR on screen.
pub fn open_shortcuts(window: &mut Window, cx: &mut App) {
    let groups: [(&str, Vec<(&str, Box<dyn Action>)>); 3] = [
        ("Navigate", vec![
            ("Next / previous change", Box::new(NextChange)),
            ("", Box::new(PrevChange)),
            ("Next / previous comment thread", Box::new(NextThread)),
            ("", Box::new(PrevThread)),
            ("Next / previous unreviewed file", Box::new(NextUnreviewed)),
            ("", Box::new(PrevUnreviewed)),
            ("Next / previous file", Box::new(NextFile)),
            ("", Box::new(PrevFile)),
            ("Back to previous place", Box::new(app::GoBack)),
        ]),
        ("Review", vec![
            ("Mark file viewed", Box::new(ToggleViewed)),
            ("Filter files", Box::new(FocusFileFilter)),
            ("Search in diff", Box::new(ToggleSearch)),
            ("Close search", Box::new(CloseSearch)),
            ("Switch files / conversations", Box::new(ToggleConversations)),
        ]),
        ("App", vec![("Settings", Box::new(app::OpenSettings)), ("Show shortcuts", Box::new(ShowShortcuts))]),
    ];
    // A row with an empty label adds its key to the row above ("j k").
    let mut sections: Vec<(SharedString, Vec<(SharedString, Vec<Kbd>)>)> = Vec::new();
    for (title, actions) in groups {
        let mut rows: Vec<(SharedString, Vec<Kbd>)> = Vec::new();
        for (label, action) in actions {
            let Some(kbd) = Kbd::binding_for_action(action.as_ref(), Some(CONTEXT), window) else { continue };
            match rows.last_mut() {
                Some((_, keys)) if label.is_empty() => keys.push(kbd),
                _ => rows.push((label.into(), vec![kbd])),
            }
        }
        sections.push((title.into(), rows));
    }
    window.open_dialog(cx, move |dialog, _, cx| {
        let muted = cx.theme().muted_foreground;
        dialog.title("Keyboard shortcuts").w(px(420.)).child(v_flex().gap_4().text_sm().children(sections.iter().map(
            |(title, rows)| {
                v_flex().gap_1().child(div().text_xs().text_color(muted).child(title.clone())).children(rows.iter().map(
                    |(label, keys)| {
                        h_flex()
                            .justify_between()
                            .child(label.clone())
                            .child(h_flex().gap_1().children(keys.iter().cloned()))
                    },
                ))
            },
        )))
    });
}

#[derive(Clone, Copy, PartialEq)]
enum SidebarTab {
    Files,
    Conversations,
    Semantic,
}

pub struct PrDetailScreen {
    model: Entity<PrModel>,
    tab: SidebarTab,
    sidebar_width: Option<Pixels>,
    sidebar_open: bool,
    description_open: bool,
    focus: FocusHandle,
    files: Entity<files::FilesPanel>,
    conversations: Entity<conversations::ConversationsPanel>,
    semantic: Entity<semantic::SemanticPanel>,
    diff: Entity<diff::DiffView>,
    _subscriptions: Vec<Subscription>,
    _width_load: Task<()>,
}

/// Record that the user saw this PR now; the PR list compares it with the PR's last update.
fn mark_viewed(owner: String, repo: String, number: u64, cx: &App) {
    let task = backend::call(cx, move |s| async move {
        queries::mark_pr_viewed(s.cache.lock().await.conn(), &owner, &repo, number).map_err(backend::err)
    });
    cx.background_spawn(task).detach();
}

impl PrDetailScreen {
    pub fn new(
        owner: SharedString,
        repo: SharedString,
        number: u64,
        viewer: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        mark_viewed(owner.to_string(), repo.to_string(), number, cx);
        let model = cx.new(|cx| PrModel::new(owner.clone(), repo.clone(), number, viewer, cx));
        let files = cx.new(|cx| files::FilesPanel::new(model.clone(), window, cx));
        let conversations = cx.new(|cx| conversations::ConversationsPanel::new(model.clone(), window, cx));
        let semantic = cx.new(|cx| semantic::SemanticPanel::new(model.clone(), window, cx));
        let diff = cx.new(|cx| diff::DiffView::new(model.clone(), window, cx));
        let width_load = backend::spawn(
            window,
            cx,
            |s| async move { queries::get_setting(s.cache.lock().await.conn(), SIDEBAR_WIDTH_KEY).map_err(backend::err) },
            |this: &mut Self, result, _, _| {
                let width = result.ok().flatten().and_then(|w| w.parse::<f32>().ok());
                this.sidebar_width = Some(px(width.filter(|w| (180. ..=600.).contains(w)).unwrap_or(SIDEBAR_DEFAULT)));
            },
        );
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            _subscriptions: vec![
                cx.observe(&model, |_, _, cx| cx.notify()),
                // A focused input went away (search closed, comment sent or cancelled): keep the shortcuts working.
                cx.on_focus_lost(window, |this, window, cx| this.focus.focus(window, cx)),
                // Mark again on leave: activity made while the PR was open does not count as new.
                cx.on_release(move |_, cx| mark_viewed(owner.to_string(), repo.to_string(), number, cx)),
            ],
            model,
            tab: SidebarTab::Files,
            sidebar_width: None,
            sidebar_open: true,
            description_open: true,
            focus,
            files,
            conversations,
            semantic,
            diff,
            _width_load: width_load,
        }
    }

    fn save_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        let value = f32::from(width).round().to_string();
        let save = backend::call(cx, move |s| async move {
            queries::set_setting(s.cache.lock().await.conn(), SIDEBAR_WIDTH_KEY, &value).map_err(backend::err)
        });
        cx.background_spawn(save).detach();
    }

    fn header(&self, cx: &Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        h_flex()
            .flex_shrink_0()
            .gap_3()
            .px_4()
            .py_2()
            .child(
                Button::new("back")
                    .ghost()
                    .small()
                    .label("← PRs")
                    .tooltip_with_action("Back to pull requests", &app::GoBack, None)
                    .on_click(|_, window, cx| app::go_back(window, cx)),
            )
            .child(Button::new("sidebar-toggle").ghost().small().label("Sidebar").selected(self.sidebar_open).on_click(
                cx.listener(|this, _, _, cx| {
                    this.sidebar_open = !this.sidebar_open;
                    cx.notify();
                }),
            ))
            // Two lines: the PR title (prose, body font) over one quiet meta line.
            .children(model.pr.as_ref().map(|pr| {
                let theme = cx.theme();
                let mono = theme.mono_font_family.clone();
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .pl_3()
                    .border_l_1()
                    .border_color(theme.border)
                    .child(div().text_base().font_medium().truncate().child(pr.title.clone()))
                    .child(
                        h_flex()
                            .min_w_0()
                            .gap_2()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(theme::pr_state_stamp(pr.merged_at.is_some(), &pr.state, pr.draft, cx))
                            .child(div().flex_shrink_0().font_family(mono.clone()).child(format!("#{}", pr.number)))
                            .child(
                                h_flex()
                                    .flex_shrink_0()
                                    .gap_1()
                                    .child(theme::avatar(&pr.author_login, &pr.author_avatar))
                                    .child(pr.author_login.clone()),
                            )
                            .child(div().flex_shrink_0().child(relative_time(&pr.updated_at)))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .font_family(mono)
                                    .child(format!("{} ← {}", pr.base_ref, pr.head_ref)),
                            ),
                    )
            }))
            .children(model.pr.as_ref().map(|pr| {
                let url = pr.html_url.clone();
                Button::new("open-github")
                    .ghost()
                    .small()
                    .icon(IconName::ExternalLink)
                    .tooltip("Open on GitHub")
                    .on_click(move |_, _, cx| cx.open_url(&url))
            }))
            .children(model.pr.as_ref().map(|_| {
                let count = model.pending_count();
                let button = Button::new("review").small().tooltip("Submit a review: comment, approve or request changes");
                let button = if model.pending_review.is_some() {
                    button.primary().label(format!("Finish review ({count})"))
                } else {
                    button.ghost().label("Review")
                };
                button.on_click(cx.listener(|this, _, window, cx| this.open_review(window, cx)))
            }))
            .children(model.pr.as_ref().map(|pr| {
                let has_body = pr.body.as_deref().is_some_and(|b| !b.trim().is_empty());
                Button::new("description-toggle")
                    .ghost()
                    .small()
                    .label(if has_body { "Description" } else { "No description" })
                    .selected(has_body && self.description_open)
                    .disabled(!has_body)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.description_open = !this.description_open;
                        cx.notify();
                    }))
            }))
    }

    fn open_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = cx.new(|cx| review::ReviewPanel::new(self.model.clone(), window, cx));
        window.open_dialog(cx, move |dialog, _, _| dialog.title("Finish your review").w(px(520.)).child(panel.clone()));
    }

    /// PR body for the right sidebar; `None` when the PR has no description.
    fn description(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        let body = self.model.read(cx).pr.as_ref()?.body.clone().filter(|b| !b.trim().is_empty())?;
        Some(
            v_flex()
                .id("pr-description-panel")
                .size_full()
                .px_4()
                .py_3()
                .text_sm()
                .child(TextView::markdown("pr-description", body).plugin(video::Videos))
                .overflow_y_scrollbar(),
        )
    }

    /// Shown instead of the split when the PR failed to load.
    fn load_error(&self, error: SharedString, cx: &Context<Self>) -> impl IntoElement {
        let name = pr_name(self.model.read(cx));
        div().flex_1().flex().items_center().justify_center().child(
            v_flex()
                .items_center()
                .gap_2()
                .max_w(px(480.))
                .text_sm()
                .child(div().font_family(cx.theme().mono_font_family.clone()).text_2xl().child(format!("Couldn't load {name}")))
                .child(div().text_xs().text_center().text_color(cx.theme().muted_foreground).child(error))
                .child(
                    h_flex()
                        .gap_2()
                        .mt_2()
                        .child(
                            Button::new("retry")
                                .primary()
                                .small()
                                .label("Retry")
                                .on_click(cx.listener(|this, _, _, cx| this.model.update(cx, |m, cx| m.reload(cx)))),
                        )
                        .child(
                            Button::new("error-back")
                                .ghost()
                                .small()
                                .label("Back to pull requests")
                                .on_click(|_, window, cx| app::go_back(window, cx)),
                        ),
                ),
        )
    }

    fn sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let tabs = [
            format!("Files {}", model.files.len()),
            format!("Conversations {}", model.conversation_count()),
            "Semantic".to_string(),
        ];
        let selected = self.tab as usize;
        v_flex()
            .size_full()
            .when(!model.commits.is_empty(), |el| el.child(commits::CommitSelector::new(self.model.clone())))
            .child(
                TabBar::new("sidebar-tabs")
                    .selected_index(selected)
                    .on_click(cx.listener(|this, ix: &usize, _, cx| {
                        this.tab = [SidebarTab::Files, SidebarTab::Conversations, SidebarTab::Semantic][*ix];
                        cx.notify();
                    }))
                    .children(tabs.into_iter().map(|label| Tab::new().label(label))),
            )
            // Cached: the panels re-render only when they notify, not on every diff scroll frame.
            .child(div().flex_1().min_h_0().child({
                let view: AnyView = match self.tab {
                    SidebarTab::Files => self.files.clone().into(),
                    SidebarTab::Conversations => self.conversations.clone().into(),
                    SidebarTab::Semantic => self.semantic.clone().into(),
                };
                view.cached(StyleRefinement::default().size_full())
            }))
    }
}

impl Render for PrDetailScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let (loading, error) = (model.loading, model.error.clone());
        let description = self.description(cx);
        let show_description = self.description_open && description.is_some();
        v_flex()
            .size_full()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ToggleSearch, _, cx| this.model.update(cx, |m, cx| m.open_search(cx))))
            .on_action(cx.listener(|this, _: &CloseSearch, _, cx| this.model.update(cx, |m, cx| m.close_search(cx))))
            .on_action(cx.listener(|this, _: &PrevFile, _, cx| this.model.update(cx, |m, cx| m.navigate_file(-1, cx))))
            .on_action(cx.listener(|this, _: &NextFile, _, cx| this.model.update(cx, |m, cx| m.navigate_file(1, cx))))
            .on_action(cx.listener(|this, _: &NextChange, _, cx| this.diff.update(cx, |d, cx| d.step_change(true, cx))))
            .on_action(cx.listener(|this, _: &PrevChange, _, cx| this.diff.update(cx, |d, cx| d.step_change(false, cx))))
            .on_action(cx.listener(|this, _: &NextThread, _, cx| this.diff.update(cx, |d, cx| d.step_thread(true, cx))))
            .on_action(cx.listener(|this, _: &PrevThread, _, cx| this.diff.update(cx, |d, cx| d.step_thread(false, cx))))
            .on_action(cx.listener(|this, _: &NextUnreviewed, _, cx| this.model.update(cx, |m, cx| m.navigate_unreviewed(true, cx))))
            .on_action(cx.listener(|this, _: &PrevUnreviewed, _, cx| this.model.update(cx, |m, cx| m.navigate_unreviewed(false, cx))))
            // ⌘[ goes back in the diff first; with no history the global handler leaves the PR.
            .on_action(cx.listener(|this, _: &app::GoBack, _, cx| {
                if !this.model.update(cx, |m, cx| m.go_back(cx)) {
                    cx.propagate();
                }
            }))
            .on_action(|_: &ShowShortcuts, window, cx| open_shortcuts(window, cx))
            .on_action(cx.listener(|this, _: &ToggleViewed, _, cx| this.model.update(cx, |m, cx| m.toggle_selected_reviewed(cx))))
            .on_action(cx.listener(|this, _: &ToggleConversations, _, cx| {
                this.tab = if this.tab == SidebarTab::Files { SidebarTab::Conversations } else { SidebarTab::Files };
                this.sidebar_open = true;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FocusFileFilter, window, cx| {
                this.tab = SidebarTab::Files;
                this.sidebar_open = true;
                this.files.update(cx, |files, cx| files.focus_filter(window, cx));
                cx.notify();
            }))
            .child(self.header(cx))
            .child(match (loading, error, self.sidebar_width) {
                (false, Some(error), _) => self.load_error(error, cx).into_any_element(),
                (false, None, Some(width)) => div().flex_1().overflow_hidden().mt_1().child(h_resizable("pr-detail-split")
                    .with_handle_appearance(Rc::new(|_, _, _| Some(div().into_any_element())))
                    .on_resize(cx.listener(|this, state: &Entity<ResizableState>, _, cx| {
                        // Hidden left panel has no real width; keep the saved one.
                        if let Some(&width) = state.read(cx).sizes().first().filter(|_| this.sidebar_open) {
                            this.sidebar_width = Some(width);
                            this.save_width(width, cx);
                        }
                    }))
                    // flex_none: sized side panels keep their width when the other one toggles.
                    .child(
                        resizable_panel()
                            .visible(self.sidebar_open)
                            .size(width)
                            .size_range(px(180.)..px(600.))
                            .flex_none()
                            .child(div().size_full().p_1().child(theme::island(cx).child(self.sidebar(cx)))),
                    )
                    .child(resizable_panel().child(
                        div().size_full().p_1().child(theme::island(cx).child(self.diff.clone())),
                    ))
                    .child(
                        resizable_panel()
                            .visible(show_description)
                            .size(px(DESCRIPTION_WIDTH))
                            .size_range(px(240.)..px(700.))
                            .flex_none()
                            .children(description.map(|body| {
                                div().size_full().p_1().child(theme::island(cx).child(body))
                            })),
                    )
                    ).into_any_element(),
                _ => h_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(Spinner::new())
                    .child(format!("Loading {}…", pr_name(self.model.read(cx))))
                    .into_any_element(),
            })
    }
}

/// `owner/repo#123`.
fn pr_name(model: &PrModel) -> String {
    format!("{}/{}#{}", model.owner, model.repo, model.number)
}
