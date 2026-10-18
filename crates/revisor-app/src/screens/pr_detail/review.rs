//! "Finish your review" dialog: body, Comment / Approve / Request changes, submit or discard.

use super::markdown_editor::MarkdownEditor;
use super::state::PrModel;
use crate::backend;
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    input::InputEvent,
    notification::Notification,
    radio::{Radio, RadioGroup},
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::provider::types::ReviewEvent;
use revisor_core::AppState;
use std::{future::Future, sync::Arc};

const EVENTS: [(ReviewEvent, &str, &str); 3] = [
    (ReviewEvent::Comment, "Comment", "Submit general feedback without explicit approval."),
    (ReviewEvent::Approve, "Approve", "Submit feedback and approve merging these changes."),
    (ReviewEvent::RequestChanges, "Request changes", "Submit feedback suggesting changes."),
];

pub struct ReviewPanel {
    model: Entity<PrModel>,
    editor: Entity<MarkdownEditor>,
    event: ReviewEvent,
    busy: bool,
    /// "Discard" was clicked once; the next click deletes the pending review.
    confirm_discard: bool,
    _subscriptions: [Subscription; 2],
    _task: Task<()>,
}

impl ReviewPanel {
    pub fn new(model: Entity<PrModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let repo = model.read(cx).repo_name();
        let editor = cx.new(|cx| MarkdownEditor::new("", "Leave a comment", (4, 12), repo, window, cx));
        let subscription = cx.subscribe(&editor.read(cx).input.clone(), |_, _, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                cx.notify();
            }
        });
        let observer = cx.observe(&editor, |_, _, cx| cx.notify());
        Self {
            model,
            editor,
            event: ReviewEvent::Comment,
            busy: false,
            confirm_discard: false,
            _subscriptions: [subscription, observer],
            _task: Task::ready(()),
        }
    }

    /// GitHub rejects Comment / Request changes with no body and no review comments.
    fn can_submit(&self, cx: &App) -> bool {
        !self.busy
            && !self.editor.read(cx).uploading()
            && (self.event == ReviewEvent::Approve
                || !self.editor.read(cx).value(cx).is_empty()
                || self.model.read(cx).pending_count() > 0)
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let m = self.model.read(cx);
        let (o, r, n) = (m.owner.to_string(), m.repo.to_string(), m.number);
        let (pending, event) = (m.pending_review.as_ref().map(|p| p.id), self.event);
        let body = self.editor.read(cx).value(cx);
        self.run(window, cx, "submit review", move |s| async move {
            s.provider().await?.submit_review(&o, &r, n, pending, event, &body).await.map_err(backend::err)
        });
    }

    fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.confirm_discard {
            self.confirm_discard = true;
            cx.notify();
            return;
        }
        let m = self.model.read(cx);
        let (o, r, n) = (m.owner.to_string(), m.repo.to_string(), m.number);
        let Some(id) = m.pending_review.as_ref().map(|p| p.id) else { return };
        self.run(window, cx, "discard review", move |s| async move {
            s.provider().await?.delete_pending_review(&o, &r, n, id).await.map_err(backend::err)
        });
    }

    fn set_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        self.busy = busy;
        self.editor.update(cx, |editor, cx| editor.set_disabled(busy, cx));
        cx.notify();
    }

    /// Run a provider call; on success close the dialog and reload the PR, else show the error.
    fn run<Fut>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        what: &'static str,
        call: impl FnOnce(Arc<AppState>) -> Fut,
    ) where
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        self.set_busy(true, cx);
        self._task = backend::spawn(
            window,
            cx,
            call,
            move |this: &mut Self, result, window, cx| {
                this.set_busy(false, cx);
                this.confirm_discard = false;
                match result {
                    Ok(()) => {
                        window.close_dialog(cx);
                        this.model.update(cx, |m, cx| m.reload(cx));
                    }
                    Err(e) => window.push_notification(Notification::error(format!("Failed to {what}: {e}")), cx),
                }
            },
        );
    }
}

impl Render for ReviewPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let m = self.model.read(cx);
        // GitHub does not let authors approve or request changes on their own PR.
        let own = m.pr.as_ref().is_some_and(|pr| m.viewer.as_deref() == Some(pr.author_login.as_str()));
        let (pending, count) = (m.pending_review.is_some(), m.pending_count());
        let theme = cx.theme();
        let selected = EVENTS.iter().position(|(e, ..)| *e == self.event);
        let entity = cx.entity().downgrade();

        v_flex()
            .gap_3()
            .child(self.editor.clone())
            .child(
                RadioGroup::new("review-event")
                    .selected_index(selected)
                    .disabled(self.busy)
                    .children(EVENTS.iter().enumerate().map(|(i, (event, label, help))| {
                        Radio::new(("review-event", i))
                            .label(*label)
                            .tooltip(*help)
                            .disabled(own && *event != ReviewEvent::Comment)
                    }))
                    .on_click(move |i, _, cx| {
                        entity
                            .update(cx, |this, cx| {
                                this.event = EVENTS[*i].0;
                                cx.notify();
                            })
                            .ok();
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(match (own, count) {
                        (true, _) => "You can only comment on your own pull request.".to_string(),
                        (_, 0) => EVENTS.iter().find(|(e, ..)| *e == self.event).map_or("", |(.., h)| *h).to_string(),
                        (_, n) => format!("{n} pending comment{} will be submitted.", if n == 1 { "" } else { "s" }),
                    }),
            )
            .child(
                h_flex()
                    .gap_2()
                    .when(pending, |el| {
                        el.child(
                            Button::new("review-discard")
                                .danger()
                                .ghost()
                                .small()
                                .label(if self.confirm_discard { "Click again to discard" } else { "Discard review" })
                                .disabled(self.busy)
                                .on_click(cx.listener(|this, _, window, cx| this.discard(window, cx))),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        Button::new("review-cancel")
                            .ghost()
                            .small()
                            .label("Cancel")
                            .disabled(self.busy)
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("review-submit")
                            .primary()
                            .small()
                            .label("Submit review")
                            .loading(self.busy)
                            .disabled(!self.can_submit(cx))
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                    ),
            )
    }
}
