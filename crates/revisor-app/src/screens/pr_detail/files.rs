//! Files tab: review focus and a filterable file list.

use super::state::{CommitSelection, PrEvent, PrModel, ScrollTarget};
use crate::{backend, theme};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    input::{Escape, Input, InputEvent, InputState},
    progress::Progress,
    scroll::ScrollableElement as _,
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::cache::queries;
use revisor_core::files::{build_file_tree, related_files, review_progress, visible_rows, Relation, Row, RowKind};
use revisor_core::focus::{self, FocusResult};
use revisor_core::keyring;
use std::collections::HashSet;
use std::ops::Range;

const VIEW_KEY: &str = "file-list-view";
/// Row height at the default 16px rem; rows scale with the appearance setting.
const ROW_HEIGHT: f32 = 26.;
const ROW_GROUP: &str = "file-row";

/// The selected file's related test, or the code a selected test tests.
pub fn counterpart(model: &PrModel) -> Option<String> {
    let path = model.selected.as_ref()?;
    let i = model.files.iter().position(|f| &f.filename == path)?;
    related_files(&model.files, i)
        .into_iter()
        .find(|(_, r)| matches!(r, Relation::Test | Relation::Source))
        .map(|(j, _)| model.files[j].filename.clone())
}

pub struct FilesPanel {
    model: Entity<PrModel>,
    focus: Entity<FocusPanel>,
    filter: Entity<InputState>,
    include: Entity<InputState>,
    exclude: Entity<InputState>,
    show_globs: bool,
    tree: bool,
    /// Collapsed dirs of the tree view (separate from the diff's collapsed files).
    collapsed_dirs: HashSet<String>,
    rows: Vec<Row>,
    scroll: UniformListScrollHandle,
    /// Selection the list last scrolled to.
    scrolled_to: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl FilesPanel {
    pub fn new(model: Entity<PrModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = |placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let filter = input("Filter files...", window, cx);
        let include = input("e.g. *.ts, src/**", window, cx);
        let exclude = input("e.g. *.test.ts, dist/**", window, cx);
        let mut subscriptions: Vec<Subscription> = [&filter, &include, &exclude]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                    if let InputEvent::Change = event {
                        this.apply_filter(cx);
                    }
                })
            })
            .collect();
        // `reveal` clears filters in the model; mirror that into the inputs.
        subscriptions.push(cx.observe_in(&model, window, |this, model, window, cx| {
            let m = model.read(cx);
            let values = [m.filter_query.clone(), m.include.clone(), m.exclude.clone()];
            for (input, value) in [&this.filter, &this.include, &this.exclude].into_iter().zip(values) {
                if input.read(cx).value() != value {
                    input.update(cx, |s, cx| s.set_value(value, window, cx));
                }
            }
            this.rebuild_rows(cx);
            cx.notify();
        }));
        subscriptions.push(cx.subscribe(&model, |this, _, event: &PrEvent, cx| {
            if let PrEvent::SelectedFromScroll = event {
                this.reveal_selected(cx);
                cx.notify();
            }
        }));
        backend::spawn(
            window,
            cx,
            |s| async move { queries::get_setting(s.cache.lock().await.conn(), VIEW_KEY).map_err(backend::err) },
            |this: &mut Self, result, _, cx| {
                this.tree = result.ok().flatten().as_deref() == Some("tree");
                this.rebuild_rows(cx);
            },
        )
        .detach();
        let focus = cx.new(|cx| FocusPanel::new(model.clone(), window, cx));
        Self {
            model,
            focus,
            filter,
            include,
            exclude,
            show_globs: false,
            tree: false,
            collapsed_dirs: HashSet::new(),
            rows: vec![],
            scroll: UniformListScrollHandle::new(),
            scrolled_to: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn focus_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filter.update(cx, |s, cx| s.focus(window, cx));
    }

    fn apply_filter(&mut self, cx: &mut Context<Self>) {
        let [query, include, exclude] =
            [&self.filter, &self.include, &self.exclude].map(|i| i.read(cx).value().to_string());
        self.model.update(cx, |m, cx| m.set_filter(query, include, exclude, cx));
    }

    fn clear_globs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for input in [&self.include, &self.exclude] {
            input.update(cx, |s, cx| s.set_value("", window, cx));
        }
        self.apply_filter(cx);
    }

    fn toggle_view(&mut self, cx: &mut Context<Self>) {
        self.tree = !self.tree;
        self.scrolled_to = None;
        self.rebuild_rows(cx);
        let value = if self.tree { "tree" } else { "list" };
        let save = backend::call(cx, move |s| async move {
            queries::set_setting(s.cache.lock().await.conn(), VIEW_KEY, value).map_err(backend::err)
        });
        cx.background_spawn(save).detach();
        cx.notify();
    }

    fn rebuild_rows(&mut self, cx: &App) {
        let m = self.model.read(cx);
        let paths = m.visible.iter().map(|&i| m.files[i].filename.as_str());
        self.rows = if self.tree {
            // ponytail: in reading order a dir can show more than once; its rows collapse together.
            visible_rows(&build_file_tree(paths, m.reading_order().is_some()), &self.collapsed_dirs)
        } else {
            paths
                .enumerate()
                .map(|(index, path)| Row {
                    depth: 0,
                    name: path.rsplit('/').next().unwrap_or(path).to_string(),
                    path: path.to_string(),
                    kind: RowKind::File { index },
                })
                .collect()
        };
        self.reveal_selected(cx);
    }

    /// Scroll the sidebar to the selected file once per selection change.
    fn reveal_selected(&mut self, cx: &App) {
        let m = self.model.read(cx);
        if m.selected != self.scrolled_to {
            self.scrolled_to = m.selected.clone();
            if let Some(ix) = self.rows.iter().position(|r| m.selected.as_ref() == Some(&r.path)) {
                self.scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
            }
        }
    }

    fn render_rows(&mut self, range: Range<usize>, _: &mut Window, cx: &mut Context<Self>) -> Vec<Stateful<Div>> {
        let m = self.model.read(cx);
        let theme = cx.theme();
        let start = range.start;
        self.rows[range]
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let path = row.path.clone();
                // By position: in reading order the same dir can have more than one row.
                let el = h_flex()
                    .id(("file-row", start + i))
                    .group(ROW_GROUP)
                    .relative()
                    .w_full()
                    .h(rems(ROW_HEIGHT / 16.))
                    .gap_1p5()
                    .pl(px(12. + row.depth as f32 * 12.))
                    .pr_3()
                    .text_xs()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.list_hover));
                let index = match row.kind {
                    RowKind::File { index } => index,
                    RowKind::Dir { files, collapsed } => {
                        return el
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.collapsed_dirs.remove(&path) {
                                this.collapsed_dirs.insert(path.clone());
                            }
                            this.rebuild_rows(cx);
                            cx.notify();
                        }))
                        .child(
                            Icon::new(if collapsed { IconName::ChevronRight } else { IconName::ChevronDown })
                                .xsmall()
                                .text_color(theme.muted_foreground),
                        )
                        .child(div().flex_1().min_w_0().truncate().font_medium().child(row.name.clone()))
                        .child(div().text_color(theme.muted_foreground).child(files.to_string()))
                    }
                };
                let file = m.visible_file(index);
                let reviewed = m.reviewed.contains(&file.filename);
                let (letter, color) = theme::file_status(&file.status, cx);
                let dir = (!self.tree).then(|| row.path.rsplit_once('/').map(|(d, _)| format!("{d}/"))).flatten();
                let (model, viewed_path) = (self.model.clone(), path.clone());
                let selected = m.selected.as_deref() == Some(&file.filename);
                el.when(selected, |el| el.bg(theme.list_active))
                    .child(
                        theme::margin_mark(theme.primary)
                            .when(!selected, |el| el.invisible().group_hover(ROW_GROUP, |s| s.visible())),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.model.update(cx, |m, cx| m.select_file(&path, cx));
                    }))
                    .child(
                        // Ticking "viewed" must not also select the row.
                        div()
                            .id(SharedString::from(format!("viewed:{}", row.path)))
                            .on_click(|_, _, cx| cx.stop_propagation())
                            .child(
                                Checkbox::new(SharedString::from(format!("viewed-box:{}", row.path)))
                                    .checked(reviewed)
                                    .tooltip("Viewed (V)")
                                    .on_click(move |_, _, cx| {
                                        model.update(cx, |m, cx| m.toggle_reviewed(&viewed_path, cx))
                                    }),
                            ),
                    )
                    .children(m.reading_step(&file.filename).map(|(step, _)| {
                        div().flex_shrink_0().font_family(theme.mono_font_family.clone()).text_color(theme.muted_foreground).child(format!("{step}."))
                    }))
                    .child(div().w_3().flex_shrink_0().font_family(theme.mono_font_family.clone()).font_semibold().text_color(color).child(letter))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .children(dir.map(|d| div().min_w_0().truncate().text_color(theme.muted_foreground).child(d)))
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .font_medium()
                                    .when(reviewed, |el| el.line_through().text_color(theme.muted_foreground))
                                    .child(row.name.clone()),
                            )
                            .when(reviewed, |el| {
                                el.child(Icon::new(IconName::Check).xsmall().ml_1().flex_shrink_0().text_color(theme.info))
                            }),
                    )
                    .when(file.additions > 0, |el| {
                        el.child(div().font_family(theme.mono_font_family.clone()).text_color(theme.success).child(format!("+{}", file.additions)))
                    })
                    .when(file.deletions > 0, |el| {
                        el.child(div().font_family(theme.mono_font_family.clone()).text_color(theme.danger).child(format!("-{}", file.deletions)))
                    })
            })
            .collect()
    }
}

impl Render for FilesPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let m = self.model.read(cx);
        let (reviewed, total) = review_progress(&m.files, &m.reviewed);
        let has_globs = !(m.include.is_empty() && m.exclude.is_empty());
        let filtered = has_globs || !m.filter_query.is_empty();
        let (has_order, ai_order) = (m.has_reading_order(), m.ai_order);
        let theme = cx.theme();
        let glob_field = |label: &'static str, input: &Entity<InputState>| {
            v_flex()
                .gap_0p5()
                .child(div().text_xs().text_color(theme.muted_foreground).child(label))
                .child(Input::new(input).small())
        };
        let filters = v_flex()
            .flex_shrink_0()
            .gap_2()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .gap_1()
                    // Escape leaves the filter; focus goes back to the diff shortcuts.
                    .on_action(|_: &Escape, window, cx| window.blur(cx))
                    .child(Input::new(&self.filter).small().cleanable(true).flex_1())
                    .when(has_order, |el| {
                        el.child(
                            // The label names the order in use, so the state is clear without a pressed look.
                            Button::new("ai-order")
                                .xsmall()
                                .map(|b| if ai_order { b.outline().icon(IconName::Check).label("AI order") } else { b.ghost().label("Diff order") })
                                .tooltip(if ai_order {
                                    "Files follow the AI reading order. Click for diff order."
                                } else {
                                    "Files follow the diff order. Click for the AI reading order."
                                })
                                .on_click(cx.listener(|this, _, _, cx| this.model.update(cx, |m, cx| m.toggle_ai_order(cx)))),
                        )
                    })
                    .child(
                        Button::new("file-view")
                            .ghost()
                            .xsmall()
                            .icon(if self.tree { IconName::Menu } else { IconName::FolderClosed })
                            .tooltip(if self.tree { "Switch to list view" } else { "Switch to tree view" })
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_view(cx))),
                    )
                    .child(
                        Button::new("file-globs")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Settings2)
                            .selected(has_globs)
                            .tooltip("Toggle file glob filters")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_globs = !this.show_globs;
                                cx.notify();
                            })),
                    ),
            )
            .when(self.show_globs, |el| {
                el.child(glob_field("Files to include", &self.include))
                    .child(glob_field("Files to exclude", &self.exclude))
            });
        let progress = v_flex()
            .flex_shrink_0()
            .gap_1p5()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .child(theme::eyebrow(format!("{reviewed} / {total} viewed"), cx))
            .child(
                Progress::new("review-progress")
                    .with_size(px(2.))
                    .color(theme.primary)
                    .value(if total == 0 { 0. } else { reviewed as f32 * 100. / total as f32 }),
            );
        let list = if self.rows.is_empty() {
            v_flex()
                .items_center()
                .gap_2()
                .py_4()
                .text_xs()
                .text_color(theme.muted_foreground)
                .when(filtered, |el| {
                    el.child(div().font_family(theme.mono_font_family.clone()).italic().text_base().child("No files match current filters"))
                })
                .when(has_globs, |el| {
                    el.child(
                        Button::new("clear-globs")
                            .link()
                            .xsmall()
                            .label("Clear glob filters")
                            .on_click(cx.listener(|this, _, window, cx| this.clear_globs(window, cx))),
                    )
                })
                .into_any_element()
        } else {
            div()
                .size_full()
                .child(
                    uniform_list("files", self.rows.len(), cx.processor(Self::render_rows))
                        .track_scroll(&self.scroll)
                        .size_full(),
                )
                .vertical_scrollbar(&self.scroll)
                .into_any_element()
        };
        v_flex()
            .size_full()
            .when(crate::experimental::enabled(cx), |el| el.child(self.focus.clone()))
            .child(filters)
            .child(progress)
            .child(div().flex_1().min_h_0().child(list))
    }
}

/// "Review focus": ranks files worth a closer look via TypeSafe.
struct FocusPanel {
    model: Entity<PrModel>,
    expanded: bool,
    has_key: bool,
    show_key_form: bool,
    key: Entity<InputState>,
    saving: bool,
    busy: bool,
    result: Option<FocusResult>,
    error: Option<SharedString>,
    show_skipped: bool,
    /// Commit selection and `(filename, sha)` of the files the result belongs to.
    context: (CommitSelection, Vec<(String, String)>),
    _key_op: Task<()>,
    _analyze: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl FocusPanel {
    fn new(model: Entity<PrModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let key = cx.new(|cx| InputState::new(window, cx).placeholder("TypeSafe API key").masked(true));
        let load = backend::spawn(
            window,
            cx,
            |_| async move { keyring::get_focus_key().map(|k| k.is_some()).map_err(backend::err) },
            |this: &mut Self, result, _, _| match result {
                Ok(has) => this.has_key = has,
                Err(_) => this.error = Some("Could not read the TypeSafe key status.".into()),
            },
        );
        let subscriptions = vec![
            cx.observe(&model, |this, model, cx| {
                let m = model.read(cx);
                let (selection, files) = &this.context;
                let same = *selection == m.selection
                    && files.len() == m.files.len()
                    && files.iter().zip(&m.files).all(|((n, s), f)| *n == f.filename && *s == f.sha);
                if !same {
                    this.context = (m.selection, m.files.iter().map(|f| (f.filename.clone(), f.sha.clone())).collect());
                    this.reset_result();
                }
                cx.notify();
            }),
            cx.subscribe_in(&key, window, |this, _, event, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.save_key(window, cx);
                }
            }),
        ];
        Self {
            model,
            // Experimental and needs a TypeSafe key: keep the file list in view until asked for.
            expanded: false,
            has_key: false,
            show_key_form: false,
            key,
            saving: false,
            busy: false,
            result: None,
            error: None,
            show_skipped: false,
            context: (CommitSelection::All, vec![]),
            _key_op: load,
            _analyze: Task::ready(()),
            _subscriptions: subscriptions,
        }
    }

    /// Drops the result and any in-flight request.
    fn reset_result(&mut self) {
        self.result = None;
        self.error = None;
        self.busy = false;
        self._analyze = Task::ready(());
    }

    fn save_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.key.read(cx).value().to_string();
        if self.busy || self.saving || key.trim().is_empty() {
            return;
        }
        self.set_key(Some(key), window, cx);
    }

    /// Store `key`, or delete the stored key for `None`.
    fn set_key(&mut self, key: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.saving = true;
        self.error = None;
        let removing = key.is_none();
        self._key_op = backend::spawn(
            window,
            cx,
            move |_| async move {
                match key {
                    Some(key) => keyring::store_focus_key(&key),
                    None => keyring::delete_focus_key(),
                }
                .map_err(backend::err)
            },
            move |this, result, window, cx| {
                this.saving = false;
                match result {
                    Ok(()) => {
                        this.reset_result();
                        this.has_key = !removing;
                        this.show_key_form = false;
                        this.key.update(cx, |s, cx| s.set_value("", window, cx));
                    }
                    Err(e) if removing => this.error = Some(format!("Could not remove the TypeSafe key: {e}").into()),
                    Err(e) => this.error = Some(format!("Could not save key: {e}").into()),
                }
            },
        );
        cx.notify();
    }

    fn analyze(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !crate::experimental::enabled(cx) { return; }
        if !self.has_key {
            self.show_key_form = true;
            cx.notify();
            return;
        }
        let m = self.model.read(cx);
        let Some(pr) = m.pr.as_ref().filter(|_| m.selection == CommitSelection::All && !m.files_loading) else {
            return;
        };
        let snapshot = focus::snapshot_from(&m.owner, &m.repo, pr, &m.files);
        self.reset_result();
        self.busy = true;
        // Replaced (and so cancelled) by `reset_result` when files or the commit selection change.
        self._analyze = backend::spawn(
            window,
            cx,
            move |_| async move {
                let key = keyring::get_focus_key()
                    .map_err(backend::err)?
                    .ok_or_else(|| "TypeSafe key not found".to_string())?;
                Ok(focus::analyze(snapshot, key).await)
            },
            |this, result, _, _| {
                this.busy = false;
                match result {
                    Ok(result) => this.result = Some(result),
                    Err(e) => this.error = Some(format!("Could not find focus files: {e}").into()),
                }
            },
        );
        cx.notify();
    }

    fn render_result(&self, result: &FocusResult, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let suggestions = result.suggestions.iter().map(|s| {
            let path = s.filename.clone();
            v_flex()
                .id(SharedString::from(format!("focus:{}", s.filename)))
                .px_1()
                .py_1()
                .cursor_pointer()
                .hover(|st| st.bg(theme.list_hover))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.model.update(cx, |m, cx| m.reveal(&path, ScrollTarget::File(path.clone()), cx))
                }))
                .child(div().truncate().text_color(theme.link).child(s.filename.clone()))
                .child(div().text_color(theme.muted_foreground).child(format!(
                    "{}{}",
                    s.topic,
                    if s.confidence < 0.6 { " · Uncertain" } else { "" }
                )))
        });
        let empty = if result.error.is_some() {
            "No suggestions from completed analyses."
        } else {
            "No strong focus signals found. Review all files as usual."
        };
        v_flex()
            .gap_1()
            .child(div().text_color(theme.muted_foreground).child(format!(
                "{} analyzed · {} not analyzed",
                result.analyzed_count,
                result.skipped.len()
            )))
            .children(result.error.clone().map(|e| div().text_color(theme.danger).child(e)))
            .child(if result.suggestions.is_empty() {
                div().child(empty).into_any_element()
            } else {
                v_flex().id("focus-suggestions").max_h(px(200.)).overflow_y_scrollbar().children(suggestions).into_any_element()
            })
            .when(!result.skipped.is_empty(), |el| {
                el.child(
                    Button::new("focus-skipped")
                        .link()
                        .xsmall()
                        .label(if self.show_skipped { "Hide not analyzed" } else { "Not analyzed" })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_skipped = !this.show_skipped;
                            cx.notify();
                        })),
                )
                .when(self.show_skipped, |el| {
                    el.child(
                        v_flex()
                            .id("focus-skipped-list")
                            .max_h(px(112.))
                            .overflow_y_scrollbar()
                            .gap_1()
                            .text_color(theme.muted_foreground)
                            .children(result.skipped.iter().map(|s| div().child(format!("{}: {}", s.filename, s.reason)))),
                    )
                })
            })
    }
}

impl Render for FocusPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let m = self.model.read(cx);
        let (all, files_loading, has_pr) = (m.selection == CommitSelection::All, m.files_loading, m.pr.is_some());
        let theme = cx.theme();
        let key_empty = self.key.read(cx).value().trim().is_empty();
        let header = h_flex()
            .id("focus-header")
            .gap_1()
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| {
                this.expanded = !this.expanded;
                cx.notify();
            }))
            .child(
                Icon::new(if self.expanded { IconName::ChevronDown } else { IconName::ChevronRight })
                    .xsmall()
                    .text_color(theme.muted_foreground),
            )
            .child(theme::eyebrow("Review focus", cx).text_color(theme.foreground))
            .child(theme::eyebrow("· Experimental", cx).flex_1())
            .when(self.expanded, |el| {
                el.child(Button::new("focus-key").link().xsmall().label("Key").on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.show_key_form = !this.show_key_form;
                    cx.notify();
                })))
            });
        let key_form = v_flex()
            .gap_1p5()
            .child(Input::new(&self.key).small().mask_toggle())
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("focus-key-save")
                            .primary()
                            .xsmall()
                            .label("Save")
                            .loading(self.saving)
                            .disabled(key_empty || self.saving || self.busy)
                            .on_click(cx.listener(|this, _, window, cx| this.save_key(window, cx))),
                    )
                    .child(Button::new("focus-key-cancel").ghost().xsmall().label("Cancel").on_click(cx.listener(
                        |this, _, window, cx| {
                            this.show_key_form = false;
                            this.key.update(cx, |s, cx| s.set_value("", window, cx));
                            cx.notify();
                        },
                    )))
                    .when(self.has_key, |el| {
                        el.child(
                            Button::new("focus-key-remove")
                                .danger()
                                .xsmall()
                                .label("Remove key")
                                .disabled(self.saving || self.busy)
                                .on_click(cx.listener(|this, _, window, cx| this.set_key(None, window, cx))),
                        )
                    }),
            );
        let body = v_flex()
            .gap_2()
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child("Sends file names and diff text to TypeSafe. Suggestions do not replace a full review."),
            )
            .when(self.show_key_form, |el| el.child(key_form))
            .child(if all {
                Button::new("focus-find")
                    .outline()
                    .xsmall()
                    .label("Find focus files")
                    .loading(self.busy)
                    .disabled(!has_pr || self.busy || self.saving || files_loading)
                    .on_click(cx.listener(|this, _, window, cx| this.analyze(window, cx)))
                    .into_any_element()
            } else {
                div()
                    .text_color(theme.muted_foreground)
                    .child("Switch to All commits to find focus files.")
                    .into_any_element()
            })
            .when(files_loading, |el| el.child(div().text_color(theme.muted_foreground).child("Loading files…")))
            .children(self.error.clone().map(|e| div().text_color(theme.danger).child(e)))
            .children(self.result.as_ref().map(|r| self.render_result(r, cx)));
        v_flex()
            .flex_shrink_0()
            .gap_2()
            .px_3()
            .py_2()
            .text_xs()
            .border_b_1()
            .border_color(theme.border)
            .child(header)
            .when(self.expanded, |el| el.child(body))
    }
}
