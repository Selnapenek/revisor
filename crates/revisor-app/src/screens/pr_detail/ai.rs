//! Overview tab: the PR description, then a one-click review by the installed Claude Code or Codex CLI.
//! The review shows a recap of the change and suggested comments; each one opens a prefilled comment form
//! on its line. Nothing is posted by itself.

use super::state::{PrEvent, PrModel, ScrollTarget};
use crate::ai_runs::{self, Finished};
use crate::{backend, theme, video};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    menu::{DropdownMenu as _, PopupMenuItem},
    scroll::ScrollableElement as _,
    text::TextView,
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::agent::{self, Backend, Model};
use revisor_core::ai_review::{self, Choice, Comment, Review};
use revisor_core::cache::queries;
use revisor_core::comments::Side;

/// An installed CLI with its models and the chosen model and effort.
struct Agent {
    backend: Backend,
    models: Vec<Model>,
    model: String,
    effort: String,
}

impl Agent {
    fn current(&self) -> Option<&Model> {
        self.models.iter().find(|m| m.id == self.model)
    }

    fn normalize(&mut self) {
        (self.model, self.effort) = agent::pick(&self.models, &self.model, &self.effort);
    }
}

pub struct AiPanel {
    model: Entity<PrModel>,
    /// `ai_review::review_key` of this PR; the run itself lives in `ai_runs`.
    key: String,
    agents: Vec<Agent>,
    /// CLI detection finished; until then assume one is installed.
    agents_loaded: bool,
    selected: usize,
    review: Option<Review>,
    /// No CLI to run; run failures come from `ai_runs`.
    error: Option<String>,
    description_open: bool,
    report_open: bool,
    details_open: bool,
    _load: Task<()>,
    _agents_load: Task<()>,
    _subscriptions: Vec<Subscription>,
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

impl AiPanel {
    pub fn new(model: Entity<PrModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let m = model.read(cx);
        let key = ai_review::review_key(&m.owner, &m.repo, m.number);
        let saved_key = key.clone();
        let load = backend::spawn(
            window,
            cx,
            move |s| async move { queries::get_setting(s.cache.lock().await.conn(), &saved_key).map_err(backend::err) },
            |this: &mut Self, result, _, cx| {
                if this.review.is_none() {
                    this.review = result.ok().flatten().and_then(|json| serde_json::from_str(&json).ok());
                    this.share_reading_order(cx);
                    cx.notify();
                }
            },
        );
        let agents_load = Self::load_agents(window, cx);
        let runs = ai_runs::runs(cx);
        // A run started here or from the PR list may finish while this panel exists.
        let subscriptions = vec![
            cx.observe(&model, |_, _, cx| cx.notify()),
            cx.observe(&runs, |_, _, cx| cx.notify()),
            cx.subscribe(&runs, |this, _, event: &Finished, cx| {
                if let (true, Ok(review)) = (event.key == this.key, &event.result) {
                    this.review = Some(review.clone());
                    this.share_reading_order(cx);
                }
            }),
        ];
        Self {
            model,
            key,
            agents: Vec::new(),
            agents_loaded: false,
            selected: 0,
            review: None,
            error: None,
            description_open: true,
            report_open: true,
            details_open: false,
            _load: load,
            _agents_load: agents_load,
            _subscriptions: subscriptions,
        }
    }

    /// The file list and the diff follow the review's reading order.
    fn share_reading_order(&self, cx: &mut Context<Self>) {
        let Some(review) = &self.review else { return };
        let (head, order) = (review.head_sha.clone(), review.reading_order.clone());
        self.model.update(cx, |m, cx| m.set_reading_order(head, order, cx));
    }

    /// Read current defaults when this PR screen returns from Settings.
    pub fn reload_agents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self._agents_load = Self::load_agents(window, cx);
    }

    fn load_agents(window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
        backend::spawn(
            window,
            cx,
            |s| async move {
                let models = tokio::task::spawn_blocking(|| {
                    agent::installed().into_iter().map(|b| (b, agent::models(b))).collect::<Vec<_>>()
                });
                let models = models.await.map_err(backend::err)?;
                let db = s.cache.lock().await;
                let get = |key: String| queries::get_setting(db.conn(), &key).map(Option::unwrap_or_default).map_err(backend::err);
                let preferred = get("ai-backend".into())?;
                let mut agents = Vec::new();
                for (backend, models) in models {
                    let (model, effort) = (get(format!("ai-model:{}", backend.executable()))?, get(format!("ai-effort:{}", backend.executable()))?);
                    let mut agent = Agent { backend, models, model, effort };
                    agent.normalize();
                    agents.push(agent);
                }
                Ok((agents, Backend::parse(&preferred)))
            },
            |this: &mut Self, result, _, cx| {
                this.agents_loaded = true;
                if let Ok((agents, preferred)) = result {
                    this.selected = agents.iter().position(|a| Some(a.backend) == preferred).unwrap_or(0);
                    this.agents = agents;
                    cx.notify();
                }
            },
        )
    }

    fn save(key: String, value: String, cx: &App) {
        let task = backend::call(cx, move |s| async move {
            queries::set_setting(s.cache.lock().await.conn(), &key, &value).map_err(backend::err)
        });
        cx.background_spawn(task).detach();
    }

    fn pick_backend(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index;
        Self::save("ai-backend".into(), self.agents[index].backend.executable().into(), cx);
        cx.notify();
    }

    fn pick_model(&mut self, model: String, cx: &mut Context<Self>) {
        let Some(agent) = self.agents.get_mut(self.selected) else { return };
        agent.model = model;
        agent.normalize();
        let name = agent.backend.executable();
        Self::save(format!("ai-model:{name}"), agent.model.clone(), cx);
        Self::save(format!("ai-effort:{name}"), agent.effort.clone(), cx);
        cx.notify();
    }

    fn pick_effort(&mut self, effort: String, cx: &mut Context<Self>) {
        let Some(agent) = self.agents.get_mut(self.selected) else { return };
        agent.effort = effort;
        Self::save(format!("ai-effort:{}", agent.backend.executable()), agent.effort.clone(), cx);
        cx.notify();
    }

    /// Before the CLIs are detected this uses the first installed one with its defaults.
    fn choice(&self) -> Option<Choice> {
        let some = |s: &String| (!s.is_empty()).then(|| s.clone());
        match self.agents.get(self.selected) {
            Some(a) => Some(Choice { backend: a.backend, model: some(&a.model), effort: some(&a.effort) }),
            None => agent::installed().first().map(|&backend| Choice { backend, model: None, effort: None }),
        }
    }

    pub fn busy(&self, cx: &App) -> bool {
        ai_runs::runs(cx).read(cx).is_running(&self.key)
    }

    /// The saved review is for the commit on screen.
    pub fn is_current(&self, cx: &App) -> bool {
        self.review.as_ref().is_some_and(|r| self.model.read(cx).head_sha() == Some(r.head_sha.as_str()))
    }

    /// Claude Code or Codex is installed, or detection is still running.
    pub fn available(&self) -> bool {
        !self.agents_loaded || !self.agents.is_empty()
    }

    /// Findings of the current review not yet dismissed or drafted, most severe first.
    pub fn open_findings(&self, cx: &App) -> Vec<(usize, &Comment)> {
        let Some(review) = self.review.as_ref().filter(|_| self.is_current(cx)) else { return Vec::new() };
        let mut open: Vec<_> = review.comments.iter().enumerate().filter(|(_, c)| !c.dismissed).collect();
        open.sort_by_key(|(_, c)| ai_review::rank(c));
        open
    }

    /// Scroll the diff to the most severe open finding, when its line is in the diff.
    pub fn reveal_top_finding(&mut self, cx: &mut Context<Self>) {
        let files = &self.model.read(cx).files;
        let Some((_, c)) = self.open_findings(cx).into_iter().find(|(_, c)| ai_review::in_diff(files, c)) else { return };
        let (path, line) = (c.file.clone(), c.line);
        let side = if c.side == "old" { Side::Left } else { Side::Right };
        self.model.update(cx, |m, cx| m.reveal(&path, ScrollTarget::Line { path: path.clone(), side, line }, cx));
    }

    pub fn start(&mut self, cx: &mut Context<Self>) {
        let m = self.model.read(cx);
        let (Some(pr), Some(head)) = (m.pr.as_ref(), m.head_sha()) else { return };
        if self.busy(cx) || m.files_loading {
            return;
        }
        let (title, body, head) = (pr.title.clone(), pr.body.clone().unwrap_or_default(), head.to_string());
        let (owner, repo, base) = (m.owner.to_string(), m.repo.to_string(), pr.base_sha.clone());
        let (files, key) = (m.files.clone(), self.key.clone());
        let Some(choice) = self.choice() else {
            self.error = Some("Install Claude Code or Codex and sign in to use AI review.".into());
            cx.notify();
            return;
        };
        self.error = None;
        ai_runs::runs(cx).update(cx, |runs, cx| {
            runs.start(key.clone(), cx, move |s| async move {
                let standards = ai_review::standards(&*s.provider().await?, &owner, &repo, &base).await;
                let review = ai_review::run(choice, head, &title, &body, standards, &files).await?;
                ai_review::save(&s, &key, &review).await?;
                Ok(review)
            })
        });
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        let key = self.key.clone();
        ai_runs::runs(cx).update(cx, |runs, cx| runs.cancel(&key, cx));
    }

    fn draft(&mut self, index: usize, comment: &Comment, cx: &mut Context<Self>) {
        let (path, line, body) = (comment.file.clone(), comment.line, comment.body.clone());
        let side = if comment.side == "old" { Side::Left } else { Side::Right };
        self.set_dismissed(Some(index), true, cx);
        self.model.update(cx, |m, cx| {
            m.reveal(&path, ScrollTarget::Line { path: path.clone(), side, line }, cx);
            cx.emit(PrEvent::DraftComment { path, side, line, body });
        });
        cx.notify();
    }

    /// Mark one finding (`Some`) or all of them, and save the review so it survives a restart.
    fn set_dismissed(&mut self, index: Option<usize>, dismissed: bool, cx: &mut Context<Self>) {
        let Some(review) = self.review.as_mut() else { return };
        for (i, c) in review.comments.iter_mut().enumerate() {
            if index.is_none_or(|index| index == i) {
                c.dismissed = dismissed;
            }
        }
        if let Ok(json) = serde_json::to_string(review) {
            Self::save(self.key.clone(), json, cx);
        }
        cx.notify();
    }

    /// A dropdown button. Options are (value, label, detail); `stacked` puts the detail under the
    /// label (model IDs), else at the right (effort descriptions). `pick` gets the chosen value.
    #[allow(clippy::too_many_arguments)]
    fn picker(
        &self,
        id: &'static str,
        label: String,
        options: Vec<(String, String, String)>,
        selected: String,
        stacked: bool,
        pick: fn(&mut Self, String, &mut Context<Self>),
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        Button::new(id).xsmall().outline().label(label).dropdown_caret(true).disabled(self.busy(cx)).dropdown_menu(
            move |menu, _, _| {
                options.iter().fold(menu, |menu, (value, label, detail)| {
                    let (entity, value) = (entity.clone(), value.clone());
                    let (label, detail) = (label.clone(), detail.clone());
                    let item = PopupMenuItem::element(move |_, cx| {
                        let detail = div().text_xs().text_color(cx.theme().muted_foreground).child(detail.clone());
                        if stacked {
                            v_flex().child(label.clone()).child(detail).into_any_element()
                        } else {
                            h_flex().w_full().gap_4().child(div().flex_1().child(label.clone())).child(detail).into_any_element()
                        }
                    });
                    menu.item(item.checked(value == selected).on_click(move |_, _, cx| {
                        entity.update(cx, |this, cx| pick(this, value.clone(), cx)).ok();
                    }))
                })
            },
        )
    }

    /// CLI, model and effort for the next run; saved for later PRs.
    fn choices(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        let agent = self.agents.get(self.selected)?;
        let model = agent.current()?;
        let effort = model.efforts.iter().find(|e| e.id == agent.effort)?;
        let backends = self.agents.iter().enumerate().map(|(i, a)| (i.to_string(), a.backend.label().to_string(), String::new())).collect();
        let models = agent.models.iter().map(|m| (m.id.clone(), m.label.clone(), m.id.clone())).collect();
        let efforts = model.efforts.iter().map(|e| (e.id.clone(), e.label.clone(), e.description.clone())).collect();
        Some(
            v_flex()
                .gap_1()
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_1()
                        .when(self.agents.len() > 1, |el| {
                            el.child(self.picker(
                                "ai-backend",
                                agent.backend.label().into(),
                                backends,
                                self.selected.to_string(),
                                false,
                                |this, i, cx| this.pick_backend(i.parse().unwrap_or(0), cx),
                                cx,
                            ))
                        })
                        .child(self.picker("ai-model", model.label.clone(), models, model.id.clone(), true, Self::pick_model, cx))
                        .child(self.picker(
                            "ai-effort",
                            format!("{} effort", effort.label),
                            efforts,
                            effort.id.clone(),
                            false,
                            Self::pick_effort,
                            cx,
                        )),
                )
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("Runs {} · {} effort", model.id, effort.label.to_lowercase())),
                ),
        )
    }

    fn comment_row(&self, index: usize, comment: &Comment, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let color = match comment.severity.as_str() {
            "high" => theme.danger,
            "medium" => theme.warning,
            _ => theme.muted_foreground,
        };
        let located = ai_review::in_diff(&self.model.read(cx).files, comment);
        let location = format!("{}:{}", comment.file, comment.line);
        let (path, line) = (comment.file.clone(), comment.line);
        let side = if comment.side == "old" { Side::Left } else { Side::Right };
        let link = if located {
            Button::new(("ai-link", index))
                .link()
                .xsmall()
                .label(location)
                .on_click(cx.listener(move |this, _, _, cx| {
                    let target = ScrollTarget::Line { path: path.clone(), side, line };
                    this.model.update(cx, |m, cx| m.reveal(&path, target, cx));
                }))
                .into_any_element()
        } else {
            div().text_color(theme.muted_foreground).child(format!("{location} · not in diff")).into_any_element()
        };
        let comment = comment.clone();
        v_flex()
            .gap_1()
            .p_3()
            .border_b_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .gap_2()
                    .min_w_0()
                    .child(theme::stamp(comment.severity.clone(), color, theme))
                    .children(ai_review::AXES.iter().find(|(a, _)| *a == comment.axis).map(|(_, title)| div().text_color(theme.muted_foreground).child(*title)))
                    .child(link),
            )
            .when(!comment.rule.is_empty(), |el| el.child(div().text_color(theme.muted_foreground).child(comment.rule.clone())))
            .child(TextView::markdown(("ai-comment", index), comment.body.clone()))
            .child(
                h_flex()
                    .gap_1()
                    .when(located, |el| {
                        el.child(
                            Button::new(("ai-draft", index))
                                .xsmall()
                                .outline()
                                .label("Add comment")
                                .tooltip("Open the comment form on this line with this text")
                                .on_click(cx.listener(move |this, _, _, cx| this.draft(index, &comment, cx))),
                        )
                    })
                    .child(
                        Button::new(("ai-dismiss", index))
                            .xsmall()
                            .ghost()
                            .label("Dismiss")
                            .on_click(cx.listener(move |this, _, _, cx| this.set_dismissed(Some(index), true, cx))),
                    ),
            )
    }

    fn section(title: impl Into<SharedString>, body: impl IntoElement, cx: &Context<Self>) -> Div {
        v_flex().gap_1().p_3().border_b_1().border_color(cx.theme().border).child(theme::eyebrow(title, cx)).child(body)
    }

    /// A section that opens and closes from its header; `flag` picks its open state.
    fn fold(
        &self,
        id: &'static str,
        title: &'static str,
        open: bool,
        flag: fn(&mut Self) -> &mut bool,
        body: impl IntoElement,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new(id)
                    .ghost()
                    .small()
                    .w_full()
                    .accessibility_label(format!("{} {title}", if open { "Close" } else { "Open" }))
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .child(Icon::new(if open { IconName::ChevronDown } else { IconName::ChevronRight }).xsmall())
                            .child(theme::eyebrow(title, cx)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let open = flag(this);
                        *open = !*open;
                        cx.notify();
                    })),
            )
            .when(open, |el| el.child(body))
    }

    fn results(&self, review: &Review, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let danger = &review.danger;
        let door_color = if danger.door == "one-way" { theme.danger } else { theme.success };
        let (spec_label, spec_color) = match review.spec.status.as_str() {
            "matches" => ("matches description", theme.success),
            "partial" => ("partly matches", theme.warning),
            "differs" => ("differs from description", theme.danger),
            _ => ("no spec in description", muted),
        };
        let standards = match review.standards.is_empty() {
            true if review.standards_status == "limited" => "No standards files loaded".to_string(),
            true => "No standards files found on the base branch".to_string(),
            false => format!("Rules from {}", review.standards.join(", ")),
        };
        let (standards_label, standards_color) = match review.standards_status.as_str() {
            "checked" => ("Checked in supplied diff", theme.success),
            "limited" => ("Limited: rule files skipped", theme.warning),
            _ => ("No rules loaded", muted),
        };
        // Sorted here too: reviews saved before severity-first ordering are still sorted by axis.
        let mut open: Vec<_> = review.comments.iter().enumerate().filter(|(_, c)| !c.dismissed).collect();
        open.sort_by_key(|(_, c)| ai_review::rank(c));
        let (total, dismissed) = (review.comments.len(), review.comments.len() - open.len());
        let findings = v_flex()
            .child(
                h_flex()
                    .gap_2()
                    .px_3()
                    .pt_3()
                    .child(theme::eyebrow("Findings", cx))
                    .when(total > 0, |el| el.child(div().text_color(muted).child(format!("{} of {total} left", open.len()))))
                    .when(dismissed > 0, |el| {
                        el.child(
                            Button::new("ai-restore")
                                .xsmall()
                                .ghost()
                                .label(format!("Restore {dismissed}"))
                                .tooltip("Show dismissed findings again")
                                .on_click(cx.listener(|this, _, _, cx| this.set_dismissed(None, false, cx))),
                        )
                    }),
            )
            .when(total == 0, |el| el.child(div().px_3().py_2().text_color(muted).child("None found. The AI can miss problems.")))
            .children(open.into_iter().map(|(i, c)| self.comment_row(i, c, cx)));
        let details = v_flex()
            .child(Self::section("Evidence", TextView::markdown("ai-evidence", review.evidence.clone()), cx))
            .child(Self::section("Spec", TextView::markdown("ai-spec", review.spec.note.clone()), cx))
            .child(Self::section(
                "Standards check",
                v_flex()
                    .gap_1()
                    .child(theme::stamp(standards_label, standards_color, theme))
                    .child(div().text_color(muted).child(standards.clone()))
                    .child(theme::eyebrow("Applicable rules", cx))
                    .child(TextView::markdown("ai-standards-applicable", review.standards_check.applicable.clone()))
                    .child(theme::eyebrow("Supported by diff", cx))
                    .child(TextView::markdown("ai-standards-passing", review.standards_check.passing.clone()))
                    .child(theme::eyebrow("Cannot check from diff", cx))
                    .child(TextView::markdown("ai-standards-unchecked", review.standards_check.unchecked.clone())),
                cx,
            ))
            .when(!review.skipped.is_empty(), |el| {
                el.child(
                    v_flex()
                        .gap_0p5()
                        .p_3()
                        .text_color(muted)
                        .child(theme::eyebrow(format!("Not reviewed {}", review.skipped.len()), cx))
                        .children(review.skipped.iter().map(|(file, reason)| div().child(format!("{file} · {reason}")))),
                )
            });
        v_flex()
            .when(!self.is_current(cx), |el| {
                el.child(div().px_3().pt_3().text_color(theme.warning).child(format!(
                    "This review is for commit {}. Run it again for the current changes.",
                    short(&review.head_sha)
                )))
            })
            .child(Self::section("Summary", TextView::markdown("ai-summary", review.summary.clone()), cx))
            .child(
                v_flex()
                    .gap_1()
                    .p_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .child(theme::stamp(format!("{} door", danger.door), door_color, theme))
                            .child(div().text_color(muted).child(format!("Blast radius: {}", danger.blast_radius)))
                            .child(theme::stamp(spec_label, spec_color, theme)),
                    )
                    .child(div().child(danger.note.clone())),
            )
            .child(findings)
            .child(self.fold("ai-details-toggle", "Details", self.details_open, |this| &mut this.details_open, details, cx))
    }
}

impl Render for AiPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy(cx);
        let error = self.error.clone().or_else(|| ai_runs::runs(cx).read(cx).error(&self.key).map(String::from));
        let theme = cx.theme();
        let m = self.model.read(cx);
        let files_loading = m.files_loading;
        let body = m.pr.as_ref().and_then(|pr| pr.body.clone()).filter(|b| !b.trim().is_empty());
        let description = match body {
            Some(body) => div().px_3().pb_3().text_sm().child(TextView::markdown("pr-description", body).plugin(video::Videos)).into_any_element(),
            None => div().px_3().pb_3().text_color(theme.muted_foreground).child("No description.").into_any_element(),
        };
        let label = match (&self.review, self.is_current(cx)) {
            (None, _) => "Run AI review",
            (Some(_), true) => "Run again",
            (Some(_), false) => "Review new changes",
        };
        let recap = self.review.as_ref().map(|r| {
            let cost = r.cost_usd.map_or(String::new(), |c| format!(" · ${c:.2}"));
            div().flex_1().min_w_0().text_color(theme.muted_foreground).child(format!(
                "AI recap · {} · {} · {}s{cost}",
                r.backend,
                short(&r.head_sha),
                r.seconds
            ))
        });
        let actions = v_flex()
            .gap_2()
            .p_3()
            .border_b_1()
            .border_color(theme.border)
            .when(self.review.is_none(), |el| {
                el.child(div().text_color(theme.muted_foreground).child(
                    "Runs Claude Code or Codex with your account. The PR diff, title and description \
                     go to Anthropic or OpenAI. You get a recap and suggested comments; nothing is posted.",
                ))
            })
            .children(self.choices(cx))
            .child(h_flex().gap_2().items_center().children(recap).child(
                Button::new("ai-run")
                    .map(|b| if self.review.is_some() { b.xsmall().ghost() } else { b.small().primary() })
                    .label(if busy { "Reviewing…" } else { label })
                    .loading(busy)
                    .disabled(busy || files_loading)
                    .on_click(cx.listener(|this, _, _, cx| this.start(cx))),
            )
            .when(busy, |el| {
                el.child(
                    Button::new("ai-cancel")
                        .small()
                        .ghost()
                        .label("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                )
            }));
        let report = v_flex()
            .child(actions)
            .children(error.map(|e| div().px_3().pt_3().text_color(theme.danger).child(e)))
            .children(self.review.as_ref().map(|review| self.results(review, cx)));
        v_flex()
            .id("overview")
            .size_full()
            .text_xs()
            .overflow_y_scrollbar()
            .child(self.fold("description-toggle", "Description", self.description_open, |this| &mut this.description_open, description, cx))
            .child(self.fold("ai-report-toggle", "AI review", self.report_open, |this| &mut this.report_open, report, cx))
    }
}
