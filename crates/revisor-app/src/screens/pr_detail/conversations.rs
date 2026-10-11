//! Conversations tab: timeline of threads, issue comments and reviews; AI ranking of review findings.

use super::state::{CommitSelection, PrModel, ScrollTarget};
use crate::{backend, theme};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement as _,
    tab::{Tab, TabBar},
    text::TextView,
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::comments::CommentThread;
use revisor_core::provider::types::{IssueComment, PrReview};
use revisor_core::rank_findings::{self as rf, RankingResult};
use revisor_core::{keyring, time::relative_time};
use std::hash::{DefaultHasher, Hash, Hasher};

pub(super) const NO_KEY: &str = "Save a TypeSafe API key in the Files tab";

#[derive(Clone, Copy, PartialEq)]
enum Resolution {
    All,
    Unresolved,
    Resolved,
}

enum Entry<'a> {
    Thread(&'a CommentThread),
    Issue(&'a IssueComment),
    Review(&'a PrReview),
}

impl Entry<'_> {
    fn timestamp(&self) -> &str {
        match self {
            Entry::Thread(t) => &t.replies.last().unwrap_or(&t.root).created_at,
            Entry::Issue(c) => &c.created_at,
            Entry::Review(r) => r.submitted_at.as_deref().unwrap_or(""),
        }
    }

    /// `query` is lowercase.
    fn matches(&self, query: &str) -> bool {
        let hit = |author: &str, body: &str| {
            author.to_lowercase().contains(query) || body.to_lowercase().contains(query)
        };
        match self {
            Entry::Thread(t) => std::iter::once(&t.root).chain(&t.replies).any(|c| hit(&c.author_login, &c.body)),
            Entry::Issue(c) => hit(&c.author_login, &c.body),
            Entry::Review(r) => hit(&r.author_login, &r.body),
        }
    }
}

/// One-line plain preview of a markdown body.
fn preview(body: &str) -> String {
    let text: String = body.chars().filter(|c| !"#*`~[]()>".contains(*c)).collect();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match text.char_indices().nth(100) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text,
    }
}

/// Everything a ranking result depends on; a change makes it stale.
fn ranking_context(m: &PrModel, authors: &[String]) -> u64 {
    let mut h = DefaultHasher::new();
    format!("{:?}", m.selection).hash(&mut h);
    (m.files_loading, authors).hash(&mut h);
    m.pr.as_ref().map(|p| (&p.base_sha, &p.head_sha)).hash(&mut h);
    for f in &m.files {
        (&f.filename, &f.sha, &f.status).hash(&mut h);
    }
    for c in &m.comments {
        (c.id, &c.body, &c.updated_at, c.outdated).hash(&mut h);
    }
    for t in &m.threads {
        (t.root.id, t.is_resolved).hash(&mut h);
    }
    h.finish()
}

pub struct ConversationsPanel {
    model: Entity<PrModel>,
    findings_mode: bool,
    filter: Entity<InputState>,
    resolution: Resolution,
    active: Option<u64>,
    authors: Vec<String>,
    ranking: bool,
    rank_error: Option<SharedString>,
    result: Option<RankingResult>,
    /// Context of the current ranking run or result.
    rank_context: Option<u64>,
    _rank: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl ConversationsPanel {
    pub fn new(model: Entity<PrModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter conversations..."));
        let subscriptions = vec![
            cx.observe(&model, |this, _, cx| {
                let model = this.model.read(cx);
                if this.rank_context.is_some_and(|k| k != ranking_context(model, &this.authors)) {
                    this.reset_ranking();
                }
                cx.notify();
            }),
            cx.subscribe(&filter, |_, _, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    cx.notify();
                }
            }),
        ];
        Self {
            model,
            findings_mode: false,
            filter,
            resolution: Resolution::All,
            active: None,
            authors: vec![],
            ranking: false,
            rank_error: None,
            result: None,
            rank_context: None,
            _rank: Task::ready(()),
            _subscriptions: subscriptions,
        }
    }

    fn reveal_thread(&mut self, path: &str, id: u64, cx: &mut Context<Self>) {
        self.active = Some(id);
        self.model.update(cx, |m, cx| m.reveal(path, ScrollTarget::Thread(id), cx));
    }

    fn reset_ranking(&mut self) {
        self.rank_context = None;
        self.ranking = false;
        self.result = None;
        self.rank_error = None;
        self._rank = Task::ready(());
    }

    fn toggle_author(&mut self, author: &str, cx: &mut Context<Self>) {
        match self.authors.iter().position(|a| a == author) {
            Some(i) => {
                self.authors.remove(i);
            }
            None => self.authors.push(author.to_string()),
        }
        self.reset_ranking();
        cx.notify();
    }

    fn rank(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let m = self.model.read(cx);
        let Some(pr) = m.pr.clone() else { return };
        let findings = rf::select_findings(&m.comments, m.resolutions(), &self.authors);
        if self.ranking || m.files_loading || findings.is_empty() {
            return;
        }
        let error = if m.selection != CommitSelection::All {
            Some("Switch to All commits to rank findings.")
        } else if findings.len() > 50 {
            Some("Ranking supports at most 50 findings. No content was sent.")
        } else {
            rf::files_error(&m.files)
        };
        self.rank_context = Some(ranking_context(m, &self.authors));
        self.result = None;
        self.rank_error = error.map(SharedString::from);
        if error.is_some() {
            cx.notify();
            return;
        }
        let (owner, repo, files) = (m.owner.to_string(), m.repo.to_string(), m.files.clone());
        self.ranking = true;
        self._rank = backend::spawn(
            window,
            cx,
            move |s| async move {
                let key = keyring::get_focus_key().map_err(backend::err)?.ok_or(NO_KEY)?;
                let fetches: Vec<_> = files
                    .into_iter()
                    .map(|file| {
                        let (s, pr, o, r) = (s.clone(), pr.clone(), owner.clone(), repo.clone());
                        tokio::spawn(async move {
                            let git_ref = rf::ranking_revision(&file, &pr);
                            let p = s.provider().await?;
                            let content = p.get_file_content(&o, &r, &file.filename, git_ref).await.map_err(backend::err)?;
                            Ok::<_, String>(rf::ranking_file(&file, &pr, content))
                        })
                    })
                    .collect();
                let mut contexts = Vec::with_capacity(fetches.len());
                for fetch in fetches {
                    contexts.push(fetch.await.map_err(backend::err)??);
                }
                let mut result = rf::rank(rf::snapshot_from(&owner, &repo, &pr, contexts, findings), key).await?;
                rf::rank_findings(&mut result.ranked);
                Ok(result)
            },
            |this, result, _, _| {
                this.ranking = false;
                match result {
                    Ok(result) => this.result = Some(result),
                    Err(e) => this.rank_error = Some(format!("Could not rank findings: {e}").into()),
                }
            },
        );
        cx.notify();
    }

    fn thread_card(&self, t: &CommentThread, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (path, id) = (t.root.path.clone(), t.root.id);
        let hover = theme.list_hover;
        let active = self.active == Some(id);
        let rule = if active {
            theme.primary
        } else if t.is_resolved {
            theme.success.opacity(0.6)
        } else {
            theme.info.opacity(0.6)
        };
        v_flex()
            .id(("thread", id))
            .relative()
            .gap_1()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .when(active, |el| el.bg(theme.list_active))
            .when(t.is_resolved, |el| el.opacity(0.6))
            .child(theme::margin_mark(rule))
            .child(
                h_flex()
                    .gap_1p5()
                    .child(theme::avatar(&t.root.author_login, &t.root.author_avatar))
                    .child(div().text_xs().font_semibold().truncate().child(t.root.author_login.clone()))
                    .child(div().flex_shrink_0().text_xs().text_color(theme.muted_foreground).child(relative_time(&t.root.created_at)))
                    .child(div().flex_1())
                    .when(!t.replies.is_empty(), |el| {
                        let n = t.replies.len();
                        el.child(div().flex_shrink_0().text_xs().text_color(theme.link).child(format!("{n} {}", if n == 1 { "reply" } else { "replies" })))
                    })
                    .when(t.is_resolved, |el| el.child(theme::stamp("Resolved", theme.success, theme))),
            )
            .child(
                div()
                    .text_xs()
                    .font_family(theme.mono_font_family.clone())
                    .text_color(theme.link)
                    .truncate()
                    .child(format!("{}:{}", t.root.path, t.line)),
            )
            .child(div().text_xs().line_clamp(2).child(preview(&t.root.body)))
            .on_click(cx.listener(move |this, _, _, cx| this.reveal_thread(&path, id, cx)))
            .into_any_element()
    }

    /// Issue comment or review: markdown body plus a GitHub link.
    fn body_card(
        &self,
        kind: &'static str,
        id: u64,
        author: &str,
        avatar_url: &str,
        time: Option<&str>,
        badge: Div,
        body: &str,
        url: &str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let url = url.to_string();
        v_flex()
            .relative()
            .gap_1()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .child(theme::margin_mark(theme.info.opacity(0.6)))
            .child(
                h_flex()
                    .gap_1p5()
                    .child(theme::avatar(author, avatar_url))
                    .child(div().text_xs().font_semibold().truncate().child(author.to_string()))
                    .children(time.map(|t| div().flex_shrink_0().text_xs().text_color(theme.muted_foreground).child(relative_time(t))))
                    .child(div().flex_1())
                    .child(badge),
            )
            .child(div().text_xs().child(TextView::markdown((kind, id), body.to_string())))
            .child(
                Button::new(SharedString::from(format!("{kind}-open-{id}")))
                    .link()
                    .xsmall()
                    .label("Open on GitHub")
                    .on_click(move |_, _, cx| cx.open_url(&url)),
            )
            .into_any_element()
    }

    fn timeline(&self, cx: &Context<Self>) -> impl IntoElement {
        let m = self.model.read(cx);
        let theme = cx.theme();
        let mut entries: Vec<Entry> = m
            .threads
            .iter()
            .map(Entry::Thread)
            .chain(m.issue_comments.iter().map(Entry::Issue))
            .chain(m.reviews.iter().filter(|r| !r.body.trim().is_empty()).map(Entry::Review))
            .collect();
        entries.sort_by(|a, b| b.timestamp().cmp(a.timestamp()));
        let query = self.filter.read(cx).value().to_lowercase();
        let resolution = self.resolution;
        entries.retain(|e| {
            let resolved_ok = match e {
                Entry::Thread(t) => resolution == Resolution::All || t.is_resolved == (resolution == Resolution::Resolved),
                _ => true,
            };
            resolved_ok && (query.is_empty() || e.matches(&query))
        });

        let resolved = m.threads.iter().filter(|t| t.is_resolved).count();
        let mut counts = format!("{} open", m.threads.len() - resolved);
        if resolved > 0 {
            counts += &format!(" · {resolved} resolved");
        }
        if !m.issue_comments.is_empty() {
            let n = m.issue_comments.len();
            counts += &format!(" · {n} comment{}", if n == 1 { "" } else { "s" });
        }
        let filtered = !query.is_empty() || resolution != Resolution::All;

        let items: Vec<AnyElement> = entries
            .iter()
            .map(|e| match e {
                Entry::Thread(t) => self.thread_card(t, cx),
                Entry::Issue(c) => self.body_card(
                    "issue",
                    c.id,
                    &c.author_login,
                    &c.author_avatar,
                    Some(&c.created_at),
                    theme::stamp("Conversation", theme.muted_foreground, theme),
                    &c.body,
                    &c.html_url,
                    cx,
                ),
                Entry::Review(r) => {
                    let badge = match r.state.as_str() {
                        "APPROVED" => theme::stamp("Approved", theme.success, theme),
                        "CHANGES_REQUESTED" => theme::stamp("Changes requested", theme.danger, theme),
                        "COMMENTED" => theme::stamp("Commented", theme.info, theme),
                        "DISMISSED" => theme::stamp("Dismissed", theme.muted_foreground, theme),
                        other => theme::stamp(other.to_string(), theme.muted_foreground, theme),
                    };
                    self.body_card("review", r.id, &r.author_login, &r.author_avatar, r.submitted_at.as_deref(), badge, &r.body, &r.html_url, cx)
                }
            })
            .collect();

        v_flex()
            .size_full()
            .child(div().px_3().py_2().child(Input::new(&self.filter).small().cleanable(true)))
            .child(
                div().px_3().pb_2().child(
                    TabBar::new("resolution")
                        .segmented()
                        .small()
                        .selected_index(resolution as usize)
                        .on_click(cx.listener(|this, ix: &usize, _, cx| {
                            this.resolution = [Resolution::All, Resolution::Unresolved, Resolution::Resolved][*ix];
                            cx.notify();
                        }))
                        .children(["All", "Open", "Resolved"].map(|l| Tab::new().label(l))),
                ),
            )
            .child(theme::eyebrow(counts, cx).px_3().py_1().border_b_1().border_color(theme.border))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .when(items.is_empty() && filtered, |el| {
                        el.child(
                            div()
                                .p_4()
                                .font_family(theme.mono_font_family.clone())
                                .italic()
                                .text_base()
                                .text_center()
                                .text_color(theme.muted_foreground)
                                .child("No conversations match your filters"),
                        )
                    })
                    .children(items),
            )
    }

    fn findings(&self, cx: &Context<Self>) -> impl IntoElement {
        let m = self.model.read(cx);
        let theme = cx.theme();
        let mut available: Vec<String> = m.threads.iter().map(|t| t.root.author_login.clone()).collect();
        available.sort();
        available.dedup();
        let findings = rf::select_findings(&m.comments, m.resolutions(), &self.authors);
        let excluded = rf::count_excluded_findings(&m.comments, m.resolutions(), &self.authors);
        let all_commits = m.selection == CommitSelection::All;
        let muted = theme.muted_foreground;
        let danger = theme.danger;

        let author_list = if available.is_empty() {
            div()
                .font_family(theme.mono_font_family.clone())
                .italic()
                .text_sm()
                .child("No inline comment authors are available.")
                .into_any_element()
        } else {
            v_flex()
                .gap_1()
                .max_h(px(96.))
                .overflow_y_scrollbar()
                .children(available.into_iter().map(|author| {
                    Checkbox::new(SharedString::from(format!("author:{author}")))
                        .label(author.clone())
                        .checked(self.authors.contains(&author))
                        .on_click(cx.listener(move |this, _: &bool, _, cx| this.toggle_author(&author, cx)))
                }))
                .into_any_element()
        };

        let results = self.result.as_ref().map(|result| {
            let ranked = result.ranked.iter().enumerate().filter_map(|(i, item)| {
                let f = findings.iter().find(|f| f.id == item.id)?;
                let (path, id, url) = (f.path.clone(), f.id, f.html_url.clone());
                Some(
                    v_flex()
                        .id(("finding", id))
                        .gap_0p5()
                        .p_3()
                        .border_b_1()
                        .border_color(theme.border)
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.list_hover))
                        .on_click(cx.listener(move |this, _, _, cx| this.reveal_thread(&path, id, cx)))
                        .child(
                            div()
                                .font_family(theme.mono_font_family.clone())
                                .text_color(theme.link)
                                .child(format!("{}. {}:{}", i + 1, f.path, f.line)),
                        )
                        .child(div().text_color(muted).child(format!("By {}", f.author)))
                        .child(div().text_color(muted).child(format!(
                            "Impact {:.1}/3 · {} · support confidence {}%",
                            item.impact,
                            rf::effective_support(item).replace('_', " "),
                            (item.confidence * 100.).round()
                        )))
                        .when(item.confidence < 0.6, |el| el.child(div().text_color(danger).child("Low confidence — review manually.")))
                        .child(div().mt_1().line_clamp(3).child(f.body.clone()))
                        .child(
                            Button::new(("finding-link", id))
                                .link()
                                .xsmall()
                                .label("GitHub comment")
                                .on_click(move |_, _, cx| cx.open_url(&url)),
                        ),
                )
            });
            v_flex()
                .children(result.error.clone().map(|e| div().p_2().text_color(danger).child(e)))
                .children(ranked)
                .when(!result.failed.is_empty(), |el| {
                    el.child(
                        v_flex()
                            .p_2()
                            .gap_1()
                            .child(theme::eyebrow(format!("{} failed or unprocessed", result.failed.len()), cx))
                            .children(result.failed.iter().map(|f| div().text_color(muted).child(format!("#{}: {}", f.id, f.reason)))),
                    )
                })
        });

        v_flex()
            .size_full()
            .text_xs()
            .child(
                v_flex()
                    .gap_2()
                    .p_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(div().text_color(muted).child(
                        "Select AI reviewer accounts. Ranking sends their comment text and all changed-file contents to TypeSafe. It does not prove a finding is correct.",
                    ))
                    .child(author_list)
                    .child(theme::eyebrow(format!("{} open findings · {excluded} excluded", findings.len()), cx))
                    .when(!all_commits, |el| el.child("Switch to All commits to rank findings."))
                    .child(
                        Button::new("rank")
                            .primary()
                            .small()
                            .label("Rank findings")
                            .loading(self.ranking)
                            .disabled(self.ranking || findings.is_empty() || m.files_loading || !all_commits)
                            .on_click(cx.listener(|this, _, window, cx| this.rank(window, cx))),
                    )
                    .when(self.ranking, |el| el.child(format!("Ranking {} findings…", findings.len())))
                    .children(self.rank_error.clone().map(|e| div().text_color(danger).child(e))),
            )
            .child(v_flex().flex_1().min_h_0().overflow_y_scrollbar().children(results))
    }
}

impl Render for ConversationsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(
                div().px_3().pt_2().child(
                    TabBar::new("conversation-mode")
                        .segmented()
                        .small()
                        .selected_index(self.findings_mode as usize)
                        .on_click(cx.listener(|this, ix: &usize, _, cx| {
                            this.findings_mode = *ix == 1;
                            cx.notify();
                        }))
                        .children(["Timeline", "Findings"].map(|l| Tab::new().label(l))),
                ),
            )
            .child(div().flex_1().min_h_0().child(if self.findings_mode {
                self.findings(cx).into_any_element()
            } else {
                self.timeline(cx).into_any_element()
            }))
    }
}
