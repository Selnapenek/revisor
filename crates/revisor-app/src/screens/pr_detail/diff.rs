//! The diff: toolbar, code search bar and one virtualized list over every visible file.

use super::code_text::CodeText;
use super::comments::{save_comment, save_with, CommentForm, CommentFormEvent, ThreadView};
use super::state::{CommitSelection, DiffMode, PrEvent, PrModel, ScrollTarget};
use super::syntax::FileSyntax;
use crate::{backend, theme};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    input::{Escape, Input, InputEvent, InputState},
    scroll::ScrollableElement as _,
    spinner::Spinner,
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::comments::Side;
use revisor_core::diff::expand::{
    compute_gaps, expand_all, expand_down, expand_up, ExpandableGap, GapPosition, EXPAND_LINES,
};
use revisor_core::diff::parser::{DiffHunk, DiffLine, DiffLineKind};
use revisor_core::diff::search::segments;
use revisor_core::diff::side_by_side::{to_side_by_side, SideBySideRow};
use revisor_core::diff::word_diff::{WordDiffKind, WordDiffSpan};
use revisor_core::diff::ProcessedDiff;
use revisor_core::provider::types::NewComment;
use std::collections::HashMap;
use std::ops::Range;

/// Height hint for rows the list has not measured yet (keeps the scrollbar sane).
const ROW_HEIGHT: Pixels = px(20.);
const GUTTER: Rems = rems(3.125);
/// Code line height; in rems so it follows the appearance scale.
const LINE_HEIGHT: Rems = rems(1.25);
/// Rows kept above a scrolled-to line or thread.
const SCROLL_CONTEXT: usize = 3;
const LINE_GROUP: &str = "diff-line";

/// One list row. File indexes point into `PrModel::files`.
#[derive(Clone, Copy, PartialEq)]
enum Row {
    File(usize),
    Notice(usize, &'static str),
    Expand(usize, ExpandableGap),
    Hunk(usize, usize),
    /// Index into the file's side-by-side rows.
    Split(usize, usize),
    /// Hunk and line index.
    Unified(usize, usize, usize),
    Thread(usize, u64),
    NewComment(usize),
}

impl Row {
    fn file(self) -> usize {
        match self {
            Row::File(f)
            | Row::Notice(f, _)
            | Row::Expand(f, _)
            | Row::Hunk(f, _)
            | Row::Split(f, _)
            | Row::Unified(f, ..)
            | Row::Thread(f, _)
            | Row::NewComment(f) => f,
        }
    }
}

#[derive(Clone, Copy)]
enum Expand {
    Up(usize),
    Down(usize),
    All(ExpandableGap),
}

struct Expanded {
    hunks: Vec<DiffHunk>,
    side_by_side: Vec<SideBySideRow>,
}

struct NewTarget {
    file: usize,
    side: Side,
    /// Pressed line; dragging or a modifier click moves `end` to make a multi-line range.
    anchor: u64,
    end: u64,
    /// Opened with Cmd+click: the next click on the same side sets `end`.
    pick_end: bool,
    form: Entity<CommentForm>,
    _subscription: Subscription,
}

impl NewTarget {
    /// First and last line. The form shows under the last.
    fn range(&self) -> (u64, u64) {
        (self.anchor.min(self.end), self.anchor.max(self.end))
    }

    fn covers(&self, fi: usize, side: Side, line: u64) -> bool {
        let (first, last) = self.range();
        self.file == fi && self.side == side && (first..=last).contains(&line)
    }
}

pub struct DiffView {
    model: Entity<PrModel>,
    rows: Vec<Row>,
    list: ListState,
    /// Context-expanded hunks by path, for the current commit selection.
    expanded: HashMap<String, Expanded>,
    /// Syntax colors by path, filled when a file's lines first render.
    syntax: HashMap<String, FileSyntax>,
    /// Expand click waiting for the file content.
    pending_expand: Option<(usize, Expand)>,
    new_comment: Option<NewTarget>,
    /// Mouse is held after pressing a gutter `+`; moving over lines extends the range.
    dragging: bool,
    threads: HashMap<u64, Entity<ThreadView>>,
    /// Search match index ranges by file and (old, new) line number.
    matches: HashMap<(usize, Option<u64>, Option<u64>), Range<usize>>,
    /// Changes when the file set is replaced; resets expansions and scroll.
    files_key: (CommitSelection, bool),
    search_open: bool,
    search: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl DiffView {
    pub fn new(model: Entity<PrModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search in diff..."));
        let list = ListState::new(0, ListAlignment::Top, px(400.));
        let view = cx.entity().downgrade();
        list.set_scroll_handler(move |event, _, cx| {
            view.update(cx, |this, cx| this.select_from_scroll(event.visible_range.start, cx)).ok();
        });
        let subscriptions = vec![
            cx.observe_in(&model, window, |this, _, window, cx| this.sync(window, cx)),
            cx.subscribe_in(&model, window, |this, _, event: &PrEvent, window, cx| {
                let PrEvent::ScrollTo(target) = event else { return };
                this.sync(window, cx);
                this.scroll_to(target, cx);
            }),
            cx.subscribe(&search, |this, input, event: &InputEvent, cx| match event {
                InputEvent::Change => {
                    let query = input.read(cx).value().to_string();
                    this.model.update(cx, |m, cx| m.set_search_query(query, cx));
                }
                &InputEvent::PressEnter { shift, .. } => this.model.update(cx, |m, cx| m.step_match(!shift, cx)),
                _ => {}
            }),
        ];
        let m = model.read(cx);
        let mut this = Self {
            files_key: (m.selection, m.files_loading),
            model,
            rows: vec![],
            list,
            expanded: HashMap::new(),
            syntax: HashMap::new(),
            pending_expand: None,
            new_comment: None,
            dragging: false,
            threads: HashMap::new(),
            matches: HashMap::new(),
            search_open: false,
            search,
            _subscriptions: subscriptions,
        };
        this.sync(window, cx);
        this
    }

    /// Bring local state in line with the model and rebuild the rows.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let model = self.model.read(cx);
        let key = (model.selection, model.files_loading);
        let reset = key != self.files_key;
        if reset {
            self.files_key = key;
            self.expanded.clear();
            self.syntax.clear();
            self.pending_expand = None;
            self.new_comment = None;
        }
        let search_opened = model.search.is_some() && !self.search_open;
        self.search_open = model.search.is_some();
        self.threads.retain(|id, _| model.threads.iter().any(|t| t.root.id == *id));
        if let Some((fi, action)) = self.pending_expand {
            let path = &model.files[fi].filename;
            let git_ref = model.head_sha().unwrap_or_default();
            if !model.file_lines_loading(path, git_ref) {
                // Loaded, or the fetch failed (already reported by the model).
                self.pending_expand = None;
                if let Some(lines) = model.cached_file_lines(path, git_ref) {
                    self.apply_expand(fi, action, &lines, cx);
                }
            }
        }
        if search_opened {
            // Also focuses the input.
            self.search.update(cx, |s, cx| s.set_value("", window, cx));
        }
        self.rebuild(reset, cx);
        cx.notify();
    }

    fn rebuild(&mut self, reset: bool, cx: &App) {
        let rows = self.build_rows(cx);
        if reset {
            self.list.reset_with_uniform_height(rows.len(), ROW_HEIGHT);
        } else if let Some((old, count)) = changed_span(&self.rows, &rows) {
            // Splice only what changed so measured heights and the scroll position survive.
            self.list.splice(old, count);
        }
        self.rows = rows;

        self.matches.clear();
        let model = self.model.read(cx);
        let Some(search) = &model.search else { return };
        for (i, m) in search.matches.iter().enumerate() {
            let fi = model.visible[m.file];
            let diff = model.files[fi].diff.as_ref();
            if let Some(l) = diff.and_then(|d| d.hunks.get(m.hunk)).and_then(|h| h.lines.get(m.line)) {
                self.matches.entry((fi, l.old_lineno, l.new_lineno)).or_insert(i..i).end = i + 1;
            }
        }
    }

    fn build_rows(&self, cx: &App) -> Vec<Row> {
        let model = self.model.read(cx);
        let git_ref = model.head_sha();
        // Hidden while dragging, so the form does not push the next lines away from the cursor.
        let new_target = self.new_comment.as_ref().filter(|_| !self.dragging).map(|n| (n.file, n.side, n.range().1));
        let mut rows = Vec::new();
        for &fi in &model.visible {
            let file = &model.files[fi];
            rows.push(Row::File(fi));
            if model.collapsed.contains(&file.filename) {
                continue;
            }
            let diff = match &file.diff {
                None => {
                    rows.push(Row::Notice(fi, "No diff available"));
                    continue;
                }
                Some(d) if d.is_binary => {
                    rows.push(Row::Notice(fi, "Binary file — not shown"));
                    continue;
                }
                Some(d) if d.is_too_large => {
                    rows.push(Row::Notice(fi, "File too large to display"));
                    continue;
                }
                Some(d) => d,
            };
            let expanded = self.expanded.get(&file.filename);
            let hunks = expanded.map_or(&diff.hunks, |e| &e.hunks);
            let total = git_ref.and_then(|g| model.cached_file_lines(&file.filename, g)).map(|l| l.len());
            let gaps = compute_gaps(hunks, total);
            let gap = |position, hunk| {
                gaps.iter().find(|g| g.position == position && g.hunk_index == hunk).map(|&g| Row::Expand(fi, g))
            };
            let mut threads: HashMap<(Side, u64), Vec<u64>> = HashMap::new();
            for t in model.threads_for(&file.filename) {
                threads.entry((t.side, t.line)).or_default().push(t.root.id);
            }
            // A diff line's threads, then the new comment form.
            let anchored = |rows: &mut Vec<Row>, side: Side, line: Option<u64>| {
                let Some(line) = line else { return };
                if let Some(ids) = threads.get(&(side, line)) {
                    rows.extend(ids.iter().map(|&id| Row::Thread(fi, id)));
                }
                if new_target == Some((fi, side, line)) {
                    rows.push(Row::NewComment(fi));
                }
            };
            // An expand row between hunks replaces the hunk header.
            let header = |rows: &mut Vec<Row>, hunk: usize| {
                if hunk == 0 {
                    rows.extend(gap(GapPosition::Before, 0));
                }
                let between = if hunk > 0 { gap(GapPosition::Between, hunk) } else { None };
                rows.push(between.unwrap_or(Row::Hunk(fi, hunk)));
            };
            match model.diff_mode {
                DiffMode::Split => {
                    let mut hunk = 0;
                    for (i, r) in expanded.map_or(&diff.side_by_side, |e| &e.side_by_side).iter().enumerate() {
                        if r.is_hunk_header {
                            header(&mut rows, hunk);
                            hunk += 1;
                            continue;
                        }
                        rows.push(Row::Split(fi, i));
                        anchored(&mut rows, Side::Left, r.left.as_ref().and_then(|l| l.old_lineno));
                        anchored(&mut rows, Side::Right, r.right.as_ref().and_then(|l| l.new_lineno));
                    }
                }
                DiffMode::Unified => {
                    for (h, hunk) in hunks.iter().enumerate() {
                        header(&mut rows, h);
                        for (i, l) in hunk.lines.iter().enumerate() {
                            rows.push(Row::Unified(fi, h, i));
                            let (side, line) = unified_anchor(l);
                            anchored(&mut rows, side, line);
                        }
                    }
                }
            }
            rows.extend(hunks.len().checked_sub(1).and_then(|last| gap(GapPosition::After, last)));
        }
        rows
    }

    fn hunks<'a>(&'a self, model: &'a PrModel, fi: usize) -> &'a [DiffHunk] {
        let file = &model.files[fi];
        match self.expanded.get(&file.filename) {
            Some(e) => &e.hunks,
            None => file.diff.as_ref().map_or(&[], |d| &d.hunks),
        }
    }

    /// Diff lines shown by a row: `[left, right]` in split mode, `[line, None]` in unified mode.
    fn row_lines<'a>(&'a self, model: &'a PrModel, row: Row) -> [Option<&'a DiffLine>; 2] {
        match row {
            Row::Split(fi, i) => {
                let file = &model.files[fi];
                let sbs = match self.expanded.get(&file.filename) {
                    Some(e) => &e.side_by_side,
                    None => file.diff.as_ref().map_or(&[][..], |d| &d.side_by_side),
                };
                sbs.get(i).map_or([None, None], |r| [r.left.as_ref(), r.right.as_ref()])
            }
            Row::Unified(fi, h, i) => [self.hunks(model, fi).get(h).and_then(|h| h.lines.get(i)), None],
            _ => [None, None],
        }
    }

    fn select_from_scroll(&mut self, top: usize, cx: &mut Context<Self>) {
        let Some(row) = self.rows.get(top) else { return };
        let Some(path) = self.model.read(cx).files.get(row.file()).map(|f| f.filename.clone()) else { return };
        self.model.update(cx, |m, cx| m.set_selected_from_scroll(&path, cx));
    }

    fn scroll_to(&mut self, target: &ScrollTarget, cx: &mut Context<Self>) {
        if let Some((ix, context)) = self.target_row(target, cx) {
            self.list.scroll_to(ListOffset { item_ix: ix.saturating_sub(context), offset_in_item: px(0.) });
            cx.notify();
        }
    }

    /// Row index of `target` and how many rows of context to keep above it.
    fn target_row(&self, target: &ScrollTarget, cx: &App) -> Option<(usize, usize)> {
        let model = self.model.read(cx);
        let file_row = |path: &str| {
            let fi = model.files.iter().position(|f| f.filename == path)?;
            self.rows.iter().position(|r| *r == Row::File(fi)).map(|ix| (ix, 0))
        };
        let line_row = |fi: usize, hit: &dyn Fn(Row, &DiffLine) -> bool| {
            self.rows.iter().position(|&r| {
                r.file() == fi && self.row_lines(model, r).iter().flatten().any(|l| hit(r, l))
            })
        };
        match target {
            ScrollTarget::File(path) => file_row(path),
            ScrollTarget::Thread(id) => {
                let found = self.rows.iter().position(|r| matches!(r, Row::Thread(_, t) if t == id));
                found.map(|ix| (ix, SCROLL_CONTEXT)).or_else(|| {
                    let thread = model.threads.iter().find(|t| t.root.id == *id)?;
                    file_row(&thread.root.path)
                })
            }
            ScrollTarget::Line { path, side, line } => {
                let fi = model.files.iter().position(|f| &f.filename == path)?;
                let hit = |r: Row, l: &DiffLine| {
                    let anchor = match r {
                        Row::Unified(..) => unified_anchor(l),
                        _ if *side == Side::Left => (Side::Left, l.old_lineno),
                        _ => (Side::Right, l.new_lineno),
                    };
                    anchor == (*side, Some(*line))
                };
                line_row(fi, &hit).map(|ix| (ix, SCROLL_CONTEXT)).or_else(|| file_row(path))
            }
            ScrollTarget::Match => {
                let m = model.active_match()?;
                let fi = model.visible[m.file];
                let l = model.files[fi].diff.as_ref()?.hunks.get(m.hunk)?.lines.get(m.line)?;
                let key = (l.old_lineno, l.new_lineno);
                line_row(fi, &|_, x| (x.old_lineno, x.new_lineno) == key).map(|ix| (ix, SCROLL_CONTEXT))
            }
        }
    }

    fn expand(&mut self, fi: usize, action: Expand, cx: &mut Context<Self>) {
        let model = self.model.read(cx);
        let Some(git_ref) = model.head_sha().map(str::to_string) else { return };
        let path = model.files[fi].filename.clone();
        match self.model.update(cx, |m, cx| m.file_lines(&path, &git_ref, cx)) {
            Some(lines) => {
                self.apply_expand(fi, action, &lines, cx);
                self.rebuild(false, cx);
            }
            None => self.pending_expand = Some((fi, action)),
        }
        cx.notify();
    }

    fn apply_expand(&mut self, fi: usize, action: Expand, lines: &[String], cx: &App) {
        let model = self.model.read(cx);
        let mut hunks = self.hunks(model, fi).to_vec();
        if hunks.is_empty() {
            return;
        }
        match action {
            Expand::Up(h) => expand_up(&mut hunks, h, lines, EXPAND_LINES),
            Expand::Down(h) => expand_down(&mut hunks, h, lines, EXPAND_LINES),
            Expand::All(gap) => expand_all(&mut hunks, &gap, lines),
        }
        let side_by_side = to_side_by_side(&hunks);
        let path = model.files[fi].filename.clone();
        self.syntax.remove(&path);
        self.expanded.insert(path, Expanded { hunks, side_by_side });
    }

    fn ensure_syntax(&mut self, fi: usize, cx: &App) {
        let model = self.model.read(cx);
        let path = &model.files[fi].filename;
        if !self.syntax.contains_key(path) {
            let syntax = FileSyntax::new(path, self.hunks(model, fi), &cx.theme().highlight_theme);
            self.syntax.insert(path.clone(), syntax);
        }
    }

    /// Gutter `+` press. Shift/Cmd+press (or any press after a Cmd+press) on the same file and side
    /// sets the range end of the open form; otherwise opens a form on `line`. Then drag to extend.
    fn comment_press(&mut self, fi: usize, side: Side, line: u64, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let extend = e.modifiers.shift || e.modifiers.secondary();
        match self.new_comment.as_mut().filter(|n| n.file == fi && n.side == side && (extend || n.pick_end)) {
            Some(n) => {
                n.end = line;
                n.pick_end = false;
            }
            None => {
                self.open_new_comment(fi, side, line, window, cx);
                if let Some(n) = &mut self.new_comment {
                    n.pick_end = e.modifiers.secondary();
                }
            }
        }
        self.dragging = true;
        self.rebuild(false, cx);
        cx.notify();
    }

    /// Mouse moved over a line while dragging from a gutter `+`.
    fn drag_to(&mut self, fi: usize, side: Side, line: u64, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !self.dragging {
            return;
        }
        if e.pressed_button != Some(MouseButton::Left) {
            // Released outside the diff.
            return self.end_drag(cx);
        }
        if let Some(n) = self.new_comment.as_mut().filter(|n| n.file == fi && n.side == side && n.end != line) {
            n.end = line;
            n.pick_end = false;
            cx.notify();
        }
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.dragging {
            self.dragging = false;
            self.rebuild(false, cx);
            cx.notify();
        }
    }

    /// Primary button adds to the viewer's pending review (starting one if needed); the second posts at once.
    fn open_new_comment(&mut self, fi: usize, side: Side, line: u64, window: &mut Window, cx: &mut Context<Self>) {
        let reviewing = self.model.read(cx).pending_review.is_some();
        let (primary, single) =
            if reviewing { ("Add review comment", "Add single comment") } else { ("Start a review", "Comment") };
        let (form, subscription) =
            CommentForm::open("", "Write a comment...", primary, window, cx, move |this, form, event, window, cx| {
                let (body, to_review) = match event {
                    CommentFormEvent::Submit(body) => (body, true),
                    CommentFormEvent::SubmitAlt(body) => (body, false),
                    CommentFormEvent::Cancel => {
                        this.new_comment = None;
                        this.rebuild(false, cx);
                        cx.notify();
                        return;
                    }
                };
                let Some((first, line)) = this.new_comment.as_ref().map(NewTarget::range) else { return };
                let m = this.model.read(cx);
                let Some(commit_id) = m.pr.as_ref().map(|pr| pr.head_sha.clone()) else { return };
                let (o, r, n) = (m.owner.to_string(), m.repo.to_string(), m.number);
                let pending = m.pending_review.clone();
                let params = NewComment {
                    path: m.files[fi].filename.clone(),
                    line,
                    side: if side == Side::Left { "LEFT" } else { "RIGHT" }.into(),
                    body: body.clone(),
                    commit_id,
                    start_line: (first < line).then_some(first),
                };
                let model = this.model.clone();
                // The model notification that follows rebuilds the rows.
                let done = |this: &mut Self, _: &mut Context<Self>| this.new_comment = None;
                if !to_review {
                    save_comment(
                        &model,
                        form,
                        "add comment",
                        window,
                        cx,
                        move |s| async move { s.provider().await?.add_comment(&o, &r, n, params).await.map_err(backend::err) },
                        done,
                    );
                    return;
                }
                save_with(
                    &model,
                    form,
                    "add review comment",
                    window,
                    cx,
                    move |s| async move {
                        let p = s.provider().await?;
                        let created = pending.is_none();
                        let review = match pending {
                            Some(review) => review,
                            None => p.create_pending_review(&o, &r, n, &params.commit_id).await.map_err(backend::err)?,
                        };
                        match p.add_review_comment(&review.node_id, params).await {
                            Ok(comment) => Ok((review, comment)),
                            Err(e) => {
                                // GitHub allows one pending review: drop an empty one so a retry can start again.
                                if created {
                                    p.delete_pending_review(&o, &r, n, review.id).await.ok();
                                }
                                Err(backend::err(e))
                            }
                        }
                    },
                    |m, (review, comment), cx| {
                        m.set_pending_review(Some(review), cx);
                        m.upsert_comment(comment, cx);
                    },
                    done,
                );
            });
        form.update(cx, |form, _| form.alt_label = Some(single));
        self.new_comment = Some(NewTarget { file: fi, side, anchor: line, end: line, pick_end: false, form, _subscription: subscription });
        self.rebuild(false, cx);
        cx.notify();
    }

    fn thread_view(&mut self, id: u64, cx: &mut Context<Self>) -> Entity<ThreadView> {
        let model = self.model.clone();
        self.threads.entry(id).or_insert_with(|| cx.new(|cx| ThreadView::new(model, id, true, cx))).clone()
    }

    fn render_row(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // List items size to their content; a stretching column makes every row full width.
        v_flex().w_full().child(self.row_element(ix, window, cx)).into_any_element()
    }

    fn row_element(&mut self, ix: usize, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(&row) = self.rows.get(ix) else { return div().into_any_element() };
        if let Row::Split(fi, _) | Row::Unified(fi, ..) = row {
            self.ensure_syntax(fi, cx);
        }
        let thread = match row {
            Row::Thread(_, id) => Some(self.thread_view(id, cx)),
            _ => None,
        };
        let model = self.model.read(cx);
        let theme = cx.theme();
        match row {
            Row::File(fi) => self.file_header(fi, model, cx).into_any_element(),
            Row::Notice(_, text) => div()
                .py_8()
                .text_center()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(text)
                .into_any_element(),
            Row::Expand(fi, gap) => self.expand_row(fi, gap, model, cx).into_any_element(),
            Row::Hunk(fi, h) => div()
                .px_3()
                .py_1()
                .text_xs()
                .font_family(theme.mono_font_family.clone())
                .text_color(theme.muted_foreground)
                .bg(theme::diff_hunk_bg(cx))
                .border_y_1()
                .border_color(theme.border)
                .children(self.hunks(model, fi).get(h).map(|h| format!("§ {}", h.header)))
                .into_any_element(),
            Row::Split(..) | Row::Unified(..) => self.line_row(ix, row, model, cx).into_any_element(),
            Row::Thread(..) => div().px_4().py_1().children(thread).into_any_element(),
            Row::NewComment(_) => div()
                .px_4()
                .py_1()
                .children(self.new_comment.as_ref().map(|n| {
                    let (first, last) = n.range();
                    v_flex()
                        .gap_2()
                        .pl_3()
                        .py_1()
                        .border_l_2()
                        .border_color(theme.info)
                        .child(div().text_xs().text_color(theme.muted_foreground).child(if n.pick_end {
                            format!("Comment from line {first} · click + on the last line")
                        } else if first == last {
                            format!("Comment on line {last} · drag or Shift+click + for a range")
                        } else {
                            format!("Comment on lines {first}–{last}")
                        }))
                        .child(n.form.clone())
                }))
                .into_any_element(),
        }
    }

    fn file_header(&self, fi: usize, model: &PrModel, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let file = &model.files[fi];
        let path = SharedString::from(file.filename.clone());
        let collapsed = model.collapsed.contains(&file.filename);
        let (status, color) = theme::file_status(&file.status, cx);
        let toggle = path.clone();
        let reviewed = path.clone();
        h_flex()
            .gap_3()
            .px_4()
            .py_2()
            .border_y_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .child(
                h_flex()
                    .id(ElementId::Name(path.clone()))
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.model.update(cx, |m, cx| m.toggle_collapsed(&toggle, cx))))
                    .child(
                        Icon::new(if collapsed { IconName::ChevronRight } else { IconName::ChevronDown })
                            .xsmall()
                            .text_color(theme.muted_foreground),
                    )
                    .child(div().w_3().flex_shrink_0().text_xs().font_family(theme.mono_font_family.clone()).font_semibold().text_color(color).child(status))
                    .child(div().flex_1().truncate().text_sm().font_family(theme.mono_font_family.clone()).child(path.clone())),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .gap_1p5()
                    .text_xs()
                    .font_family(theme.mono_font_family.clone())
                    .when(file.additions > 0, |el| el.child(div().text_color(theme.success).child(format!("+{}", file.additions))))
                    .when(file.deletions > 0, |el| el.child(div().text_color(theme.danger).child(format!("−{}", file.deletions)))),
            )
            .child(
                Checkbox::new(ElementId::Name(format!("viewed:{path}").into()))
                    .small()
                    .label("Viewed")
                    .checked(model.reviewed.contains(&file.filename))
                    .on_click(cx.listener(move |this, _: &bool, _, cx| {
                        this.model.update(cx, |m, cx| {
                            m.toggle_reviewed(&reviewed, cx);
                            // The file collapsed: pin its header to the top so the next file follows right below.
                            if m.reviewed.contains(reviewed.as_ref()) {
                                cx.emit(PrEvent::ScrollTo(ScrollTarget::File(reviewed.to_string())));
                            }
                        })
                    })),
            )
    }

    fn expand_row(&self, fi: usize, gap: ExpandableGap, model: &PrModel, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let path = &model.files[fi].filename;
        let loading = self.pending_expand.is_some_and(|(f, _)| f == fi);
        let incremental = gap.line_count.is_some_and(|n| n > EXPAND_LINES as u64 * 2);
        // Up / down arrow buttons, as in the old UI.
        let (up, down) = match gap.position {
            GapPosition::Before => (None, Some(Expand::Up(0))),
            GapPosition::Between => (Some(Expand::Up(gap.hunk_index)), Some(Expand::Down(gap.hunk_index - 1))),
            GapPosition::After => (Some(Expand::Down(gap.hunk_index)), None),
        };
        let button = |name: &str, icon: IconName, label: Option<String>, tooltip: String, action: Expand| {
            Button::new(ElementId::NamedInteger(format!("{path}:{:?}:{name}", gap.position).into(), gap.hunk_index as u64))
                .ghost()
                .xsmall()
                .icon(icon)
                .when_some(label, |b, label| b.label(label))
                .tooltip(tooltip)
                .on_click(cx.listener(move |this, _, _, cx| this.expand(fi, action, cx)))
        };
        let step = |name, icon, direction, action| {
            button(name, icon, Some(EXPAND_LINES.to_string()), format!("Show {EXPAND_LINES} more lines {direction}"), action)
        };
        let all = match gap.line_count {
            Some(n) => format!("Show all {n} hidden lines"),
            None => "Show all hidden lines".into(),
        };
        h_flex()
            .justify_center()
            .gap_1()
            .py_0p5()
            .bg(theme::diff_hunk_bg(cx))
            .border_y_1()
            .border_color(theme.border)
            .when(loading, |el| el.child(div().text_xs().text_color(theme.muted_foreground).child("Loading...")))
            .when(!loading, |el| {
                el.children(up.filter(|_| incremental).map(|a| step("up", IconName::ChevronUp, "above", a)))
                    .child(button("all", IconName::ChevronsUpDown, gap.line_count.map(|n| n.to_string()), all, Expand::All(gap)))
                    .children(down.filter(|_| incremental).map(|a| step("down", IconName::ChevronDown, "below", a)))
            })
    }

    fn line_row(&self, ix: usize, row: Row, model: &PrModel, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let fi = row.file();
        let [first, second] = self.row_lines(model, row);
        // Lines in the new comment's range are tinted.
        let bg = |l: &DiffLine, side: Side, number: Option<u64>| {
            let commented = number.is_some_and(|n| self.new_comment.as_ref().is_some_and(|c| c.covers(fi, side, n)));
            if commented { Some(theme.info.opacity(0.15)) } else { line_bg(l.kind, cx) }
        };
        let el = div().flex().w_full().text_xs().line_height(LINE_HEIGHT).font_family(theme.mono_font_family.clone());
        if let Row::Split(..) = row {
            let half = |line: Option<&DiffLine>, side: Side| {
                let number = line.and_then(|l| if side == Side::Left { l.old_lineno } else { l.new_lineno });
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .group(LINE_GROUP)
                    .border_l_2()
                    .border_color(line.map_or(transparent_black(), |l| marker(l.kind, cx)))
                    .when_some(line.and_then(|l| bg(l, side, number)), |el, bg| el.bg(bg))
                    .when_some(number, |el, n| el.on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| this.drag_to(fi, side, n, e, cx))))
                    .child(self.gutter(fi, number, number.map(|n| (side, n)), model, cx))
                    .child(rule(cx))
                    .children(line.map(|l| self.line_text(fi, l, code_order(ix, side), model, cx)))
            };
            return el.child(half(first, Side::Left)).child(half(second, Side::Right));
        }
        let Some(l) = first else { return el };
        let (side, anchor) = unified_anchor(l);
        let (prefix, color) = match l.kind {
            DiffLineKind::Added => ("+", theme.success),
            DiffLineKind::Removed => ("-", theme.danger),
            DiffLineKind::Context => (" ", theme.muted_foreground),
        };
        el.group(LINE_GROUP)
            .border_l_2()
            .border_color(marker(l.kind, cx))
            .when_some(bg(l, side, anchor), |el, bg| el.bg(bg))
            .when_some(anchor, |el, n| el.on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| this.drag_to(fi, side, n, e, cx))))
            .child(self.gutter(fi, l.old_lineno, anchor.map(|n| (side, n)), model, cx))
            .child(self.gutter(fi, l.new_lineno, None, model, cx))
            .child(rule(cx))
            .child(div().w_3().flex_shrink_0().text_color(color).child(prefix))
            .child(self.line_text(fi, l, code_order(ix, Side::Left), model, cx))
    }

    /// Line number cell; with an anchor, pressing it starts a new comment on that line.
    fn gutter(
        &self,
        fi: usize,
        number: Option<u64>,
        anchor: Option<(Side, u64)>,
        model: &PrModel,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let cell = div()
            .relative()
            .w(GUTTER)
            .flex_shrink_0()
            .px_2()
            .text_right()
            .text_color(theme.muted_foreground)
            .child(number.map(|n| n.to_string()).unwrap_or_default());
        let Some((side, line)) = anchor else { return cell.into_any_element() };
        let id = format!("{}:{side:?}", model.files[fi].filename);
        cell.id(ElementId::NamedInteger(id.into(), line))
            .cursor_pointer()
            .child(
                div()
                    .absolute()
                    .left_0p5()
                    .top(px(2.))
                    .size_4()
                    .rounded(px(2.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme.info)
                    .text_color(theme.info_foreground)
                    .child("+")
                    .invisible()
                    .group_hover(LINE_GROUP, |s| s.visible()),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    // Keep the window text selection from starting a drag here.
                    cx.stop_propagation();
                    this.comment_press(fi, side, line, e, window, cx)
                }),
            )
            .into_any_element()
    }

    /// Line content with syntax colors and word-diff and search highlights.
    fn line_text(&self, fi: usize, l: &DiffLine, order: u64, model: &PrModel, cx: &App) -> impl IntoElement {
        let spans = model.files[fi].diff.as_ref().map_or(&[][..], |d| word_spans(d, l));
        let (ranges, active) = match (self.matches.get(&(fi, l.old_lineno, l.new_lineno)), &model.search) {
            (Some(r), Some(search)) => (
                search.matches[r.clone()].iter().map(|m| m.start..m.end).collect(),
                search.active.checked_sub(r.start).filter(|&a| a < r.len()),
            ),
            _ => (vec![], None),
        };
        let backgrounds = segments(l.content.len(), spans, &ranges, active).into_iter().filter_map(|s| {
            let bg = match s.kind {
                _ if s.active => theme::search_match_active(cx),
                _ if s.matched => theme::search_match(cx),
                WordDiffKind::Insert => theme::diff_add_word(cx),
                WordDiffKind::Delete => theme::diff_del_word(cx),
                WordDiffKind::Equal => return None,
            };
            Some((s.range, HighlightStyle { background_color: Some(bg), ..Default::default() }))
        });
        let syntax = self.syntax.get(&model.files[fi].filename).map_or(&[][..], |s| s.line(l));
        let highlights = combine_highlights(syntax.iter().cloned(), backgrounds);
        let id = ElementId::NamedInteger(format!("code:{fi}:{:?}:{:?}", l.old_lineno, l.new_lineno).into(), order >> 32);
        div().flex_1().min_w_0().px_2().child(code_text(id, order, &l.content, highlights))
    }

    fn toolbar(&self, model: &PrModel, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let mode = model.diff_mode;
        let mode_button = |id: &'static str, label: &'static str, value: DiffMode| {
            Button::new(id)
                .ghost()
                .xsmall()
                .label(label)
                .selected(mode == value)
                .on_click(cx.listener(move |this, _, _, cx| this.model.update(cx, |m, cx| m.set_diff_mode(value, cx))))
        };
        h_flex()
            .flex_shrink_0()
            .gap_3()
            .px_4()
            .py_1p5()
            .items_center()
            .border_b_1()
            .border_color(theme.border)
            .child(theme::eyebrow(format!("{} files", model.visible.len()), cx))
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("collapse-all")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ChevronUp)
                            .tooltip("Collapse all files")
                            .on_click(cx.listener(|this, _, _, cx| this.model.update(cx, |m, cx| m.collapse_all(true, cx)))),
                    )
                    .child(
                        Button::new("expand-all")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ChevronDown)
                            .tooltip("Expand all files")
                            .on_click(cx.listener(|this, _, _, cx| this.model.update(cx, |m, cx| m.collapse_all(false, cx)))),
                    ),
            )
            .when(model.files_loading, |el| el.child(Spinner::new().xsmall()))
            .child(div().flex_1())
            .child(
                Button::new("search-toggle")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Search)
                    .selected(model.search.is_some())
                    .tooltip("Search in diff (⌘F)")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.model.update(cx, |m, cx| if m.search.is_some() { m.close_search(cx) } else { m.open_search(cx) })
                    })),
            )
            .child(
                h_flex()
                    .gap_0p5()
                    .child(mode_button("mode-split", "Split", DiffMode::Split))
                    .child(mode_button("mode-unified", "Unified", DiffMode::Unified)),
            )
    }

    fn search_bar(&self, model: &PrModel, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (query, total, active) =
            model.search.as_ref().map_or(("", 0, 0), |s| (s.query.as_str(), s.matches.len(), s.active));
        let count = match (query.is_empty(), total) {
            (true, _) => String::new(),
            (false, 0) => "No results".into(),
            _ => format!("{} of {total}", active + 1),
        };
        let step = |id: &'static str, icon: IconName, tooltip: &'static str, forward: bool| {
            Button::new(id)
                .ghost()
                .xsmall()
                .icon(icon)
                .tooltip(tooltip)
                .disabled(total == 0)
                .on_click(cx.listener(move |this, _, _, cx| this.model.update(cx, |m, cx| m.step_match(forward, cx))))
        };
        h_flex()
            .flex_shrink_0()
            .gap_2()
            .px_3()
            .py_1p5()
            .border_b_1()
            .border_color(theme.border)
            .on_action(cx.listener(|this, _: &Escape, _, cx| this.model.update(cx, |m, cx| m.close_search(cx))))
            .child(div().flex_1().child(Input::new(&self.search).small().prefix(Icon::new(IconName::Search).xsmall())))
            .child(div().flex_shrink_0().text_xs().font_family(theme.mono_font_family.clone()).text_color(theme.muted_foreground).child(count))
            .child(step("search-prev", IconName::ChevronUp, "Previous match (Shift+Enter)", false))
            .child(step("search-next", IconName::ChevronDown, "Next match (Enter)", true))
            .child(
                Button::new("search-close")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Close)
                    .tooltip("Close (Escape)")
                    .on_click(cx.listener(|this, _, _, cx| this.model.update(cx, |m, cx| m.close_search(cx)))),
            )
    }
}

impl Render for DiffView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let body = if model.visible.is_empty() {
            let filtered = !(model.filter_query.is_empty() && model.include.is_empty() && model.exclude.is_empty());
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_2()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(div().font_family(cx.theme().mono_font_family.clone()).italic().text_xl().child(if filtered {
                    "No files match the sidebar filters"
                } else {
                    "No files to review"
                }))
                .when(filtered, |el| {
                    el.child(Button::new("diff-clear-filters").link().small().label("Clear filters").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.model.update(cx, |m, cx| m.set_filter(String::new(), String::new(), String::new(), cx))
                        },
                    )))
                })
        } else {
            // Pin the header of the file at the top of the viewport.
            let top = self.rows.get(self.list.logical_scroll_top().item_ix).map(|r| r.file());
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| this.end_drag(cx)))
                .child(gpui_kit::list(self.list.clone(), cx.processor(Self::render_row)).size_full())
                .children(top.map(|fi| {
                    div().id("sticky-file-header").absolute().top_0().left_0().right_0().child(self.file_header(fi, model, cx))
                }))
                .vertical_scrollbar(&self.list)
        };
        v_flex()
            .size_full()
            .child(self.toolbar(model, cx))
            .when(model.search.is_some(), |el| el.child(self.search_bar(model, cx)))
            .child(body)
    }
}

fn unified_anchor(l: &DiffLine) -> (Side, Option<u64>) {
    if l.kind == DiffLineKind::Removed {
        (Side::Left, l.old_lineno)
    } else {
        (Side::Right, l.new_lineno)
    }
}

/// 2px left marker: success for additions, danger for deletions, blank so context text aligns.
fn marker(kind: DiffLineKind, cx: &App) -> Hsla {
    match kind {
        DiffLineKind::Added => cx.theme().success,
        DiffLineKind::Removed => cx.theme().danger,
        DiffLineKind::Context => transparent_black(),
    }
}

/// Primary-colored margin rule between the line numbers and the code.
fn rule(cx: &App) -> Div {
    div().w(px(1.)).flex_shrink_0().bg(cx.theme().primary.opacity(0.45))
}

fn line_bg(kind: DiffLineKind, cx: &App) -> Option<Hsla> {
    match kind {
        DiffLineKind::Added => Some(theme::diff_add_bg(cx)),
        DiffLineKind::Removed => Some(theme::diff_del_bg(cx)),
        DiffLineKind::Context => None,
    }
}

/// Word-diff spans of a changed line; `word_diffs` is in line order on both sides.
fn word_spans<'a>(diff: &'a ProcessedDiff, l: &DiffLine) -> &'a [WordDiffSpan] {
    let wd = &diff.word_diffs;
    match l.kind {
        DiffLineKind::Removed => {
            let i = wd.partition_point(|w| w.old_lineno < l.old_lineno);
            wd.get(i).filter(|w| w.old_lineno == l.old_lineno).map_or(&[], |w| &w.old_spans)
        }
        DiffLineKind::Added => {
            let i = wd.partition_point(|w| w.new_lineno < l.new_lineno);
            wd.get(i).filter(|w| w.new_lineno == l.new_lineno).map_or(&[], |w| &w.new_spans)
        }
        DiffLineKind::Context => &[],
    }
}

/// Selection order of a code line: the whole left column before the right one, so a drag
/// down one split column does not take the other. Thread comments keep order 0.
fn code_order(ix: usize, side: Side) -> u64 {
    let column = if side == Side::Left { 0 } else { 1 << 32 };
    column + ix as u64 + 1
}

/// Selectable line text with tabs expanded to four spaces; highlight byte ranges follow the expansion.
// ponytail: copy gives spaces for tabs and only rows the list has rendered; keep raw text per run if that matters.
fn code_text(
    id: ElementId,
    order: u64,
    content: &str,
    highlights: impl Iterator<Item = (Range<usize>, HighlightStyle)>,
) -> CodeText {
    if content.is_empty() {
        return CodeText::new(id, " ".into(), vec![], order);
    }
    if !content.contains('\t') {
        return CodeText::new(id, content.to_string().into(), highlights.collect(), order);
    }
    let at = |i: usize| i + 3 * content[..i].matches('\t').count();
    let highlights = highlights.map(|(r, s)| (at(r.start)..at(r.end), s)).collect();
    CodeText::new(id, content.replace('\t', "    ").into(), highlights, order)
}

/// The range of `old` replaced in `new` (after the common prefix and suffix) and its new length.
fn changed_span<T: PartialEq>(old: &[T], new: &[T]) -> Option<(Range<usize>, usize)> {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    if prefix == old.len() && prefix == new.len() {
        return None;
    }
    let suffix = old[prefix..].iter().rev().zip(new[prefix..].iter().rev()).take_while(|(a, b)| a == b).count();
    Some((prefix..old.len() - suffix, new.len() - prefix - suffix))
}

#[cfg(test)]
mod tests {
    use super::changed_span;

    #[test]
    fn changed_span_keeps_common_ends() {
        assert_eq!(changed_span(&[1, 2, 3], &[1, 2, 3]), None);
        assert_eq!(changed_span(&[1, 2, 3], &[1, 9, 3]), Some((1..2, 1)));
        assert_eq!(changed_span(&[1, 2, 3], &[1, 3]), Some((1..2, 0)));
        assert_eq!(changed_span(&[1, 3], &[1, 2, 2, 3]), Some((1..1, 2)));
        assert_eq!(changed_span(&[1, 1], &[1, 1, 1]), Some((2..2, 1)));
        assert_eq!(changed_span(&[], &[1]), Some((0..0, 1)));
    }
}
