//! Semantic tab: TypeSafe search over the PR diff; hits jump to their hunk.

use super::conversations::NO_KEY;
use super::state::{CommitSelection, PrModel, ScrollTarget};
use crate::{backend, theme};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement as _,
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::comments::Side;
use revisor_core::session_secrets;
use revisor_core::semantic::{self, AnchorSide, SemanticResult};
use std::hash::{DefaultHasher, Hash, Hasher};

/// Files a result was computed for; a change drops it.
fn files_context(m: &PrModel) -> u64 {
    let mut h = DefaultHasher::new();
    format!("{:?}", m.selection).hash(&mut h);
    m.files_loading.hash(&mut h);
    m.pr.as_ref().map(|p| (&p.base_sha, &p.head_sha)).hash(&mut h);
    for f in &m.files {
        (&f.filename, &f.sha).hash(&mut h);
    }
    h.finish()
}

pub struct SemanticPanel {
    model: Entity<PrModel>,
    query: Entity<InputState>,
    busy: bool,
    error: Option<SharedString>,
    result: Option<SemanticResult>,
    /// Context of the current search or result.
    context: Option<u64>,
    _search: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl SemanticPanel {
    pub fn new(model: Entity<PrModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Where did retry behavior change?"));
        let subscriptions = vec![
            cx.observe(&model, |this, model, cx| {
                if this.context.is_some_and(|k| k != files_context(model.read(cx))) {
                    this.context = None;
                    this.busy = false;
                    this.error = None;
                    this.result = None;
                    this._search = Task::ready(());
                }
                cx.notify();
            }),
            cx.subscribe_in(&query, window, |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => this.search(window, cx),
                InputEvent::Change => cx.notify(),
                _ => {}
            }),
        ];
        Self {
            model,
            query,
            busy: false,
            error: None,
            result: None,
            context: None,
            _search: Task::ready(()),
            _subscriptions: subscriptions,
        }
    }

    fn search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !crate::experimental::enabled(cx) { return; }
        let query = self.query.read(cx).value().trim().to_string();
        let m = self.model.read(cx);
        let Some(pr) = m.pr.as_ref() else { return };
        if query.is_empty() || self.busy || m.files_loading || m.selection != CommitSelection::All {
            return;
        }
        let snapshot = semantic::snapshot_from(&m.owner, &m.repo, pr, &m.files);
        self.context = Some(files_context(m));
        self.busy = true;
        self.error = None;
        self.result = None;
        self._search = backend::spawn(
            window,
            cx,
            move |_| async move {
                let key = session_secrets::get_focus_key().map_err(backend::err)?.ok_or(NO_KEY)?;
                semantic::search(snapshot, query, key).await
            },
            |this, result, _, _| {
                this.busy = false;
                match result {
                    Ok(result) => this.result = Some(result),
                    Err(e) if e == NO_KEY => this.error = Some(e.into()),
                    Err(e) => this.error = Some(format!("Semantic search failed: {e}").into()),
                }
            },
        );
        cx.notify();
    }

    fn select(&mut self, path: &str, hunk_index: usize, cx: &mut Context<Self>) {
        let m = self.model.read(cx);
        let Some(anchor) = m.files.iter().find(|f| f.filename == path).and_then(|f| semantic::anchor(f, hunk_index))
        else {
            return;
        };
        let side = match anchor.side {
            AnchorSide::Old => Side::Left,
            AnchorSide::New => Side::Right,
        };
        let target = ScrollTarget::Line { path: path.to_string(), side, line: anchor.line };
        self.model.update(cx, |m, cx| m.reveal(path, target, cx));
    }

    fn results(&self, result: &SemanticResult, cx: &Context<Self>) -> impl IntoElement {
        let m = self.model.read(cx);
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let hits = result.results.iter().map(|hit| {
            let (path, hunk_index) = (hit.filename.clone(), hit.hunk_index);
            let hunk = m
                .files
                .iter()
                .find(|f| f.filename == hit.filename)
                .and_then(|f| f.diff.as_ref()?.hunks.get(hit.hunk_index));
            let range = match hunk {
                Some(h) => format!(
                    "old {}, new {}",
                    semantic::line_range(h.old_start, h.old_count),
                    semantic::line_range(h.new_start, h.new_count)
                ),
                None => format!("old {}, new {}", hit.old_start, hit.new_start),
            };
            let label = if hit.score >= 1.5 && hit.confidence >= 0.6 { "Relevant" } else { "Possible match" };
            v_flex()
                .id(SharedString::from(format!("hit:{}:{}", hit.filename, hit.hunk_index)))
                .group("semantic-hit")
                .relative()
                .gap_0p5()
                .px_2()
                .py_2()
                .border_b_1()
                .border_color(theme.border)
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .on_click(cx.listener(move |this, _, _, cx| this.select(&path, hunk_index, cx)))
                .child(theme::margin_mark(theme.primary).invisible().group_hover("semantic-hit", |s| s.visible()))
                .child(div().truncate().font_family(theme.mono_font_family.clone()).text_color(theme.link).child(hit.filename.clone()))
                .child(div().text_color(muted).child(format!("{label} · {range}")))
                .child(
                    div()
                        .font_family(theme.mono_font_family.clone())
                        .text_color(muted)
                        .line_clamp(2)
                        .child(hit.preview.clone()),
                )
        });
        let empty = if result.error.is_some() || !result.skipped.is_empty() {
            "No matches in completed analysis. Some files were not searched."
        } else {
            "No relevant diff sections found."
        };
        v_flex()
            .gap_1()
            .child(div().px_1().text_color(muted).child(format!(
                "{} files analyzed · {} not analyzed",
                result.analyzed_count,
                result.skipped.len()
            )))
            .children(result.error.clone().map(|e| div().px_1().text_color(theme.danger).child(e)))
            .when(result.results.is_empty(), |el| {
                el.child(div().px_1().mt_2().font_family(theme.mono_font_family.clone()).italic().text_sm().child(empty))
            })
            .children(hits)
            .when(!result.skipped.is_empty(), |el| {
                el.child(theme::eyebrow("Files not analyzed", cx).px_1().mt_2()).children(
                    result.skipped.iter().map(|s| div().px_1().text_color(muted).child(format!("{}: {}", s.filename, s.reason))),
                )
            })
    }
}

impl Render for SemanticPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let m = self.model.read(cx);
        let theme = cx.theme();
        let all_commits = m.selection == CommitSelection::All;
        let disabled = self.query.read(cx).value().trim().is_empty() || self.busy || m.files_loading || !all_commits;
        v_flex()
            .size_full()
            .text_xs()
            .child(
                v_flex()
                    .gap_2()
                    .p_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(theme::eyebrow("Semantic diff search", cx))
                    .child(div().text_color(theme.muted_foreground).child("Sends your query and PR diff text to TypeSafe."))
                    .child(
                        h_flex()
                            .gap_1()
                            .child(div().flex_1().min_w_0().child(Input::new(&self.query).small()))
                            .child(
                                Button::new("semantic-search")
                                    .primary()
                                    .small()
                                    .label("Search")
                                    .loading(self.busy)
                                    .disabled(disabled)
                                    .on_click(cx.listener(|this, _, window, cx| this.search(window, cx))),
                            ),
                    )
                    .when(!all_commits, |el| {
                        el.child(div().text_color(theme.muted_foreground).child("Switch to All commits to search the full PR."))
                    })
                    .when(m.files_loading, |el| el.child(div().text_color(theme.muted_foreground).child("Loading files…")))
                    .children(self.error.clone().map(|e| div().text_color(theme.danger).child(e))),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .p_2()
                    .overflow_y_scrollbar()
                    .children(self.result.as_ref().map(|r| self.results(r, cx))),
            )
    }
}
