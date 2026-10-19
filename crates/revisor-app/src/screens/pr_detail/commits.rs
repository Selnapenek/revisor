//! Commit picker: all commits, one commit, or a Shift+click range.

use super::state::{CommitSelection, PrModel};
use crate::theme;
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    popover::Popover,
    scroll::ScrollableElement as _,
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::time::relative_time;

#[derive(IntoElement)]
pub struct CommitSelector {
    model: Entity<PrModel>,
}

impl CommitSelector {
    pub fn new(model: Entity<PrModel>) -> Self {
        Self { model }
    }
}

impl RenderOnce for CommitSelector {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Last plainly clicked commit: the fixed end of a Shift+click range.
        let anchor = window.use_keyed_state("commit-anchor", cx, |_, _| None::<usize>);
        let m = self.model.read(cx);
        let theme = cx.theme();
        let selection = m.selection;
        let all = {
            let (model, anchor) = (self.model.clone(), anchor.clone());
            h_flex()
                .id("commit-all")
                .relative()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(theme.border)
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .when(selection == CommitSelection::All, |el| el.child(theme::margin_mark(theme.primary)))
                .on_click(move |_, _, cx| {
                    anchor.update(cx, |a, _| *a = None);
                    model.update(cx, |m, cx| m.select_commits(CommitSelection::All, cx));
                })
                .child(div().flex_1().font_medium().child("All commits"))
                .child(div().font_family(theme.mono_font_family.clone()).text_color(theme.muted_foreground).child(m.commits.len().to_string()))
        };
        let commits = m.commits.iter().enumerate().map(|(i, c)| {
            let selected = match selection {
                CommitSelection::All => false,
                CommitSelection::Single(s) => s == i,
                CommitSelection::Range(from, to) => (from..=to).contains(&i),
            };
            let (model, anchor) = (self.model.clone(), anchor.clone());
            v_flex()
                .id(SharedString::from(c.sha.clone()))
                .relative()
                .gap_0p5()
                .px_3()
                .py_1p5()
                .border_b_1()
                .border_color(theme.border)
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .when(selected, |el| el.child(theme::margin_mark(theme.primary)))
                .on_click(move |event: &ClickEvent, _, cx| {
                    let last = *anchor.read(cx);
                    let next = match last {
                        Some(last) if event.modifiers().shift && last != i => {
                            CommitSelection::Range(last.min(i), last.max(i))
                        }
                        // Clicking the only selected commit goes back to all.
                        _ if selection == CommitSelection::Single(i) => CommitSelection::All,
                        _ => CommitSelection::Single(i),
                    };
                    anchor.update(cx, |a, _| {
                        *a = match next {
                            CommitSelection::All => None,
                            CommitSelection::Single(i) => Some(i),
                            CommitSelection::Range(..) => last,
                        }
                    });
                    model.update(cx, |m, cx| m.select_commits(next, cx));
                })
                .child(
                    h_flex()
                        .gap_1p5()
                        .text_xs()
                        .child(
                            div()
                                .flex_shrink_0()
                                .font_family(theme.mono_font_family.clone())
                                .text_color(theme.link)
                                .child(c.short_sha.clone()),
                        )
                        .child(div().min_w_0().truncate().child(c.message.lines().next().unwrap_or("").to_string())),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(theme::avatar(&c.author_login, &c.author_avatar))
                        .child(format!("{} · {}", c.author_login, relative_time(&c.authored_date))),
                )
        });
        Popover::new("commit-selector")
            .w(px(360.))
            .trigger(
                Button::new("commit-selector-trigger")
                    .ghost()
                    .xsmall()
                    .label(m.commit_selection_label())
                    .dropdown_caret(true),
            )
            .child(all)
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Click to select · Shift+click for range"),
            )
            .child(v_flex().id("commit-list").max_h(px(400.)).overflow_y_scrollbar().children(commits))
    }
}
