//! PR detail: header, resizable sidebar (files / conversations / semantic) and the diff.

mod code_text;
mod comments;
mod commits;
mod conversations;
mod diff;
mod files;
mod semantic;
mod syntax;
pub mod state;

use crate::app::{self, Route};
use crate::{backend, theme};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    resizable::{h_resizable, resizable_panel},
    spinner::Spinner,
    tab::{Tab, TabBar},
    tag::Tag,
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::{cache::queries, time::relative_time};
use state::PrModel;

actions!(pr_detail, [ToggleSearch, FocusFileFilter, ToggleConversations, PrevFile, NextFile, CloseSearch]);

const CONTEXT: &str = "PrDetail";
const SIDEBAR_WIDTH_KEY: &str = "sidebar-width";
const SIDEBAR_DEFAULT: f32 = 260.;

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-f", ToggleSearch, Some(CONTEXT)),
        KeyBinding::new("cmd-p", FocusFileFilter, Some(CONTEXT)),
        KeyBinding::new("cmd-shift-c", ToggleConversations, Some(CONTEXT)),
        KeyBinding::new("left", PrevFile, Some(CONTEXT)),
        KeyBinding::new("right", NextFile, Some(CONTEXT)),
        KeyBinding::new("escape", CloseSearch, Some(CONTEXT)),
    ]);
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
    focus: FocusHandle,
    files: Entity<files::FilesPanel>,
    conversations: Entity<conversations::ConversationsPanel>,
    semantic: Entity<semantic::SemanticPanel>,
    diff: Entity<diff::DiffView>,
    _subscriptions: Vec<Subscription>,
    _width_load: Task<()>,
}

impl PrDetailScreen {
    pub fn new(owner: SharedString, repo: SharedString, number: u64, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let model = cx.new(|cx| PrModel::new(owner, repo, number, cx));
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
            _subscriptions: vec![cx.observe(&model, |_, _, cx| cx.notify())],
            model,
            tab: SidebarTab::Files,
            sidebar_width: None,
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
        let (owner, repo) = (model.owner.clone(), model.repo.clone());
        h_flex()
            .flex_shrink_0()
            .gap_3()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(Button::new("back").ghost().small().label("← PRs").on_click(move |_, window, cx| {
                app::navigate(Route::PrList { owner: owner.clone(), repo: repo.clone() }, window, cx)
            }))
            .children(model.pr.as_ref().map(|pr| {
                let tag = if pr.merged_at.is_some() {
                    Tag::color(ColorName::Purple).child("Merged")
                } else if pr.state == "closed" {
                    Tag::color(ColorName::Red).child("Closed")
                } else if pr.draft {
                    Tag::secondary().child("Draft")
                } else {
                    Tag::color(ColorName::Green).child("Open")
                };
                h_flex()
                    .gap_3()
                    .min_w_0()
                    .child(tag.small())
                    .child(div().text_sm().font_medium().truncate().child(pr.title.clone()))
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .gap_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("#{} ·", pr.number))
                            .child(theme::avatar(&pr.author_login, &pr.author_avatar))
                            .child(format!("{} · {}", pr.author_login, relative_time(&pr.updated_at))),
                    )
            }))
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
        let loading = self.model.read(cx).loading;
        v_flex()
            .size_full()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ToggleSearch, _, cx| this.model.update(cx, |m, cx| m.open_search(cx))))
            .on_action(cx.listener(|this, _: &CloseSearch, _, cx| this.model.update(cx, |m, cx| m.close_search(cx))))
            .on_action(cx.listener(|this, _: &PrevFile, _, cx| this.model.update(cx, |m, cx| m.navigate_file(-1, cx))))
            .on_action(cx.listener(|this, _: &NextFile, _, cx| this.model.update(cx, |m, cx| m.navigate_file(1, cx))))
            .on_action(cx.listener(|this, _: &ToggleConversations, _, cx| {
                this.tab = if this.tab == SidebarTab::Files { SidebarTab::Conversations } else { SidebarTab::Files };
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FocusFileFilter, window, cx| {
                this.tab = SidebarTab::Files;
                this.files.update(cx, |files, cx| files.focus_filter(window, cx));
                cx.notify();
            }))
            .child(self.header(cx))
            .child(match (loading, self.sidebar_width) {
                (false, Some(width)) => h_resizable("pr-detail-split")
                    .on_resize(cx.listener(|this, state: &Entity<ResizableState>, _, cx| {
                        if let Some(&width) = state.read(cx).sizes().first() {
                            this.sidebar_width = Some(width);
                            this.save_width(width, cx);
                        }
                    }))
                    .child(resizable_panel().size(width).size_range(px(180.)..px(600.)).child(self.sidebar(cx)))
                    // Opaque so code stays readable over the blurred window.
                    .child(resizable_panel().child(div().size_full().bg(cx.theme().colors.list).child(self.diff.clone())))
                    .into_any_element(),
                _ => div().flex_1().flex().items_center().justify_center().child(Spinner::new()).into_any_element(),
            })
    }
}
