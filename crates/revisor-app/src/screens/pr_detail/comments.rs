//! Inline comment thread widgets, shared by the diff and the conversations panel.

use super::markdown_editor::MarkdownEditor;
use super::state::PrModel;
use crate::{app, backend, theme};
use gpui_kit::component::{
    button::{Button, ButtonVariant, ButtonVariants as _},
    input::{Escape, InputEvent},
    notification::Notification,
    text::TextView,
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::provider::types::ReviewComment;
use revisor_core::{time::relative_time, AppState};
use std::{future::Future, sync::Arc};

pub enum CommentFormEvent {
    Submit(String),
    /// The optional second button (`alt_label`).
    SubmitAlt(String),
    Cancel,
}

/// Markdown editor with Cancel / submit buttons. Cmd+Enter submits, Escape cancels.
pub struct CommentForm {
    editor: Entity<MarkdownEditor>,
    submit_label: &'static str,
    /// Label of a second, non-primary submit button; `None` hides it.
    pub alt_label: Option<&'static str>,
    busy: bool,
    _subscription: Subscription,
}

impl EventEmitter<CommentFormEvent> for CommentForm {}

impl CommentForm {
    pub fn new(
        initial: &str,
        placeholder: &'static str,
        submit_label: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| MarkdownEditor::new(initial, placeholder, (3, 12), window, cx));
        let subscription = cx.subscribe(&editor.read(cx).input.clone(), |this, _, event: &InputEvent, cx| match event {
            InputEvent::PressEnter { secondary: true, .. } => this.submit(false, cx),
            InputEvent::Change => cx.notify(),
            _ => {}
        });
        Self { editor, submit_label, alt_label: None, busy: false, _subscription: subscription }
    }

    /// Create a form owned by `V`; keep the returned subscription for as long as the form is shown.
    pub fn open<V: 'static>(
        initial: &str,
        placeholder: &'static str,
        submit_label: &'static str,
        window: &mut Window,
        cx: &mut Context<V>,
        on_event: impl FnMut(&mut V, &Entity<CommentForm>, &CommentFormEvent, &mut Window, &mut Context<V>) + 'static,
    ) -> (Entity<CommentForm>, Subscription) {
        let form = cx.new(|cx| CommentForm::new(initial, placeholder, submit_label, window, cx));
        let subscription = cx.subscribe_in(&form, window, on_event);
        (form, subscription)
    }

    pub fn set_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        self.busy = busy;
        self.editor.update(cx, |editor, cx| editor.set_disabled(busy, cx));
        cx.notify();
    }

    fn body(&self, cx: &App) -> String {
        self.editor.read(cx).value(cx)
    }

    fn submit(&mut self, alt: bool, cx: &mut Context<Self>) {
        let body = self.body(cx);
        if !self.busy && !body.is_empty() {
            cx.emit(if alt { CommentFormEvent::SubmitAlt(body) } else { CommentFormEvent::Submit(body) });
        }
    }
}

impl Render for CommentForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let empty = self.body(cx).is_empty();
        v_flex()
            .gap_2()
            .on_action(cx.listener(|_, _: &Escape, _, cx| cx.emit(CommentFormEvent::Cancel)))
            .child(self.editor.clone())
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("⌘Enter to submit"),
                    )
                    .child(
                        Button::new("cancel")
                            .ghost()
                            .small()
                            .label("Cancel")
                            .disabled(self.busy)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(CommentFormEvent::Cancel))),
                    )
                    .children(self.alt_label.map(|label| {
                        Button::new("submit-alt")
                            .outline()
                            .small()
                            .label(label)
                            .disabled(empty || self.busy)
                            .on_click(cx.listener(|this, _, _, cx| this.submit(true, cx)))
                    }))
                    .child(
                        Button::new("submit")
                            .primary()
                            .small()
                            .label(self.submit_label)
                            .loading(self.busy)
                            .disabled(empty || self.busy)
                            .on_click(cx.listener(|this, _, _, cx| this.submit(false, cx))),
                    ),
            )
    }
}

/// Run a comment API call for `form`: busy while pending, then `upsert_comment` and `done`, or show the error.
pub fn save_comment<V: 'static, Fut>(
    model: &Entity<PrModel>,
    form: &Entity<CommentForm>,
    what: &'static str,
    window: &Window,
    cx: &mut Context<V>,
    call: impl FnOnce(Arc<AppState>) -> Fut,
    done: impl FnOnce(&mut V, &mut Context<V>) + 'static,
) where
    Fut: Future<Output = Result<ReviewComment, String>> + Send + 'static,
{
    save_with(model, form, what, window, cx, call, |m, comment, cx| m.upsert_comment(comment, cx), done);
}

/// `save_comment` with a custom model update for the call's result.
pub fn save_with<V: 'static, T: Send + 'static, Fut>(
    model: &Entity<PrModel>,
    form: &Entity<CommentForm>,
    what: &'static str,
    window: &Window,
    cx: &mut Context<V>,
    call: impl FnOnce(Arc<AppState>) -> Fut,
    apply: impl FnOnce(&mut PrModel, T, &mut Context<PrModel>) + 'static,
    done: impl FnOnce(&mut V, &mut Context<V>) + 'static,
) where
    Fut: Future<Output = Result<T, String>> + Send + 'static,
{
    form.update(cx, |form, cx| form.set_busy(true, cx));
    let (model, form) = (model.clone(), form.clone());
    backend::spawn(window, cx, call, move |this, result, window, cx| match result {
        Ok(value) => {
            model.update(cx, |m, cx| apply(m, value, cx));
            done(this, cx);
        }
        Err(e) => {
            form.update(cx, |form, cx| form.set_busy(false, cx));
            window.push_notification(Notification::error(format!("Failed to {what}: {e}")), cx);
        }
    })
    .detach();
}

/// One review thread (root + replies), read from the model by root comment id.
pub struct ThreadView {
    model: Entity<PrModel>,
    root_id: u64,
    interactive: bool,
    /// User toggle; `None` follows the thread: resolved threads start collapsed.
    expanded: Option<bool>,
    reply: Option<(Entity<CommentForm>, Subscription)>,
    editing: Option<(u64, Entity<CommentForm>, Subscription)>,
    _subscription: Subscription,
}

impl ThreadView {
    /// `interactive: false` renders the thread read-only (no reply, resolve, edit or delete).
    pub fn new(model: Entity<PrModel>, root_id: u64, interactive: bool, cx: &mut Context<Self>) -> Self {
        Self {
            _subscription: cx.observe(&model, |_, _, cx| cx.notify()),
            model,
            root_id,
            interactive,
            expanded: None,
            reply: None,
            editing: None,
        }
    }

    fn repo(&self, cx: &App) -> (String, String, u64) {
        let m = self.model.read(cx);
        (m.owner.to_string(), m.repo.to_string(), m.number)
    }

    fn open_reply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let root = self.root_id;
        self.reply = Some(CommentForm::open("", "Write a reply...", "Reply", window, cx, move |this, form, event, window, cx| {
            match event {
                CommentFormEvent::Cancel | CommentFormEvent::SubmitAlt(_) => this.reply = None,
                CommentFormEvent::Submit(body) => {
                    let ((o, r, n), body) = (this.repo(cx), body.clone());
                    let model = this.model.clone();
                    save_comment(
                        &model,
                        form,
                        "reply",
                        window,
                        cx,
                        move |s| async move {
                            s.provider().await?.reply_to_comment(&o, &r, n, root, &body).await.map_err(backend::err)
                        },
                        |this: &mut Self, _| this.reply = None,
                    );
                }
            }
            cx.notify();
        }));
        cx.notify();
    }

    fn open_edit(&mut self, comment: &ReviewComment, window: &mut Window, cx: &mut Context<Self>) {
        let (id, original) = (comment.id, comment.body.clone());
        let (form, subscription) = CommentForm::open(&comment.body, "", "Save", window, cx, move |this, form, event, window, cx| {
            match event {
                CommentFormEvent::Submit(body) if *body != original => {
                    let ((o, r, _), body) = (this.repo(cx), body.clone());
                    let model = this.model.clone();
                    save_comment(
                        &model,
                        form,
                        "update comment",
                        window,
                        cx,
                        move |s| async move { s.provider().await?.update_comment(&o, &r, id, &body).await.map_err(backend::err) },
                        |this: &mut Self, _| this.editing = None,
                    );
                }
                _ => this.editing = None,
            }
            cx.notify();
        });
        self.editing = Some((id, form, subscription));
        cx.notify();
    }

    fn confirm_delete(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let this = this.clone();
            alert
                .confirm()
                .title("Delete comment?")
                .description("This cannot be undone.")
                .ok_text("Delete")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    this.update(cx, |this, cx| this.delete(id, window, cx)).ok();
                    true
                })
        });
    }

    fn delete(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let (o, r, _) = self.repo(cx);
        let model = self.model.clone();
        backend::spawn(
            window,
            cx,
            move |s| async move { s.provider().await?.delete_comment(&o, &r, id).await.map_err(backend::err) },
            move |_, result, window, cx| match result {
                Ok(()) => model.update(cx, |m, cx| m.remove_comment(id, cx)),
                Err(e) => window.push_notification(Notification::error(format!("Failed to delete comment: {e}")), cx),
            },
        )
        .detach();
    }

    /// Optimistic resolve / unresolve; rolls back if the API call fails.
    fn set_resolved(&mut self, node_id: String, resolved: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.model.update(cx, |m, cx| m.set_resolved(&node_id, resolved, cx));
        self.expanded = None;
        let id = node_id.clone();
        backend::spawn(
            window,
            cx,
            move |s| async move {
                let p = s.provider().await?;
                if resolved { p.resolve_thread(&id).await } else { p.unresolve_thread(&id).await }.map_err(backend::err)
            },
            move |this, result, window, cx| {
                if let Err(e) = result {
                    this.model.update(cx, |m, cx| m.set_resolved(&node_id, !resolved, cx));
                    let action = if resolved { "resolve" } else { "unresolve" };
                    window.push_notification(Notification::error(format!("Failed to {action} thread: {e}")), cx);
                }
            },
        )
        .detach();
    }

    fn comment(&self, c: &ReviewComment, first: bool, own: bool, pending: bool, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let editing = self.editing.as_ref().filter(|(id, ..)| *id == c.id).map(|(_, form, _)| form.clone());
        let url = c.html_url.clone();
        let comment = c.clone();
        v_flex()
            .gap_1()
            .px_3()
            .py_2()
            .when(!first, |el| el.border_t_1().border_color(theme.border))
            .child(
                h_flex()
                    .gap_2()
                    .text_xs()
                    .child(theme::avatar(&c.author_login, &c.author_avatar))
                    .child(div().font_semibold().child(c.author_login.clone()))
                    .child(div().text_color(theme.muted_foreground).child(relative_time(&c.created_at)))
                    .when(pending, |el| el.child(theme::stamp("Pending", theme.warning, theme)))
                    .child(div().flex_1())
                    .when(own && self.interactive && editing.is_none(), |el| {
                        let id = c.id;
                        el.child(
                            Button::new(("comment-edit", id))
                                .ghost()
                                .xsmall()
                                .label("Edit")
                                .on_click(cx.listener(move |this, _, window, cx| this.open_edit(&comment, window, cx))),
                        )
                        .child(
                            Button::new(("comment-delete", id))
                                .ghost()
                                .xsmall()
                                .label("Delete")
                                .on_click(cx.listener(move |this, _, window, cx| this.confirm_delete(id, window, cx))),
                        )
                    })
                    .child(
                        Button::new(("comment-open", c.id))
                            .ghost()
                            .xsmall()
                            .icon(IconName::ExternalLink)
                            .tooltip("Open on GitHub")
                            .on_click(move |_, _, cx| cx.open_url(&url)),
                    ),
            )
            .child(match editing {
                Some(form) => form.into_any_element(),
                None => div()
                    .text_sm()
                    .child(TextView::markdown(("comment-body", c.id), c.body.clone()))
                    .into_any_element(),
            })
    }
}

impl Render for ThreadView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let Some(thread) = model.threads.iter().find(|t| t.root.id == self.root_id) else {
            return div();
        };
        let theme = cx.theme();
        let (root, resolved) = (self.root_id, thread.is_resolved);
        // Margin note: an `info` rule (`success` once resolved) beside indented text, no boxed card.
        let frame = v_flex()
            .w_full()
            .overflow_hidden()
            .border_l_2()
            .border_color(if resolved { theme.success.opacity(0.7) } else { theme.info.opacity(0.7) });

        if !self.expanded.unwrap_or(!resolved) {
            let hover = theme.list_hover;
            return frame.child(
                h_flex()
                    .id(("thread-expand", root))
                    .gap_2()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.expanded = Some(true);
                        cx.notify();
                    }))
                    .child(theme::stamp("Resolved", theme.success, theme))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(theme.muted_foreground)
                            .child(format!("{} — {}", thread.root.author_login, preview(&thread.root.body))),
                    )
                    .when(!thread.replies.is_empty(), |el| {
                        el.child(div().text_color(theme.muted_foreground).child(format!("+{}", thread.replies.len())))
                    })
                    .child(Icon::new(IconName::ChevronDown).xsmall().text_color(theme.muted_foreground)),
            );
        }

        let me = app::current_user(cx).map(|u| u.login);
        // Pending threads are not on GitHub yet: no reply or resolve until the review is submitted.
        let pending_root = model.is_pending(&thread.root);
        let comments = std::iter::once(&thread.root).chain(&thread.replies).enumerate().map(|(i, c)| {
            let own = me.as_deref() == Some(c.author_login.as_str());
            self.comment(c, i == 0, own, model.is_pending(c), cx)
        });
        let footer = match &self.reply {
            Some((form, _)) => div().p_3().border_t_1().border_color(theme.border).child(form.clone()),
            None => {
                let hover = theme.list_hover;
                h_flex()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id(("thread-reply", root))
                            .flex_1()
                            .px_3()
                            .py_2()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .cursor_pointer()
                            .hover(move |s| s.bg(hover))
                            .child("Reply...")
                            .on_click(cx.listener(|this, _, window, cx| this.open_reply(window, cx))),
                    )
                    .children(thread.thread_node_id.clone().map(|node| {
                        Button::new(("thread-resolve", root))
                            .ghost()
                            .small()
                            .icon(if resolved { IconName::Close } else { IconName::Check })
                            .label(if resolved { "Unresolve" } else { "Resolve" })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_resolved(node.clone(), !resolved, window, cx)
                            }))
                    }))
            }
        };

        frame
            .when(resolved, |el| {
                el.child(
                    h_flex()
                        .gap_1p5()
                        .px_3()
                        .py_1()
                        .text_xs()
                        .child(theme::stamp("Resolved", theme.success, theme))
                        .child(div().flex_1())
                        .child(Button::new(("thread-collapse", root)).ghost().xsmall().label("Collapse").on_click(
                            cx.listener(|this, _, _, cx| {
                                this.expanded = Some(false);
                                cx.notify();
                            }),
                        )),
                )
            })
            .child(v_flex().when(resolved, |el| el.opacity(0.7)).children(comments))
            .when(self.interactive && !pending_root, |el| el.child(footer))
    }
}

/// One-line plain-text preview of a markdown body.
fn preview(body: &str) -> String {
    let text: String = body
        .chars()
        .filter(|c| !"#*`~[]()>".contains(*c))
        .map(|c| if c == '\n' { ' ' } else { c })
        .collect();
    let text = text.trim();
    match text.char_indices().nth(80) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text.to_string(),
    }
}
