use super::state::PrModel;
use gpui_kit::*;

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
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div().child(self.model.read(cx).commit_selection_label())
    }
}
