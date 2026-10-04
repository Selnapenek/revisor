use super::state::PrModel;
use gpui_kit::*;

pub struct ConversationsPanel {
    model: Entity<PrModel>,
}

impl ConversationsPanel {
    pub fn new(model: Entity<PrModel>, _: &mut Window, _: &mut Context<Self>) -> Self {
        Self { model }
    }
}

impl Render for ConversationsPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let _ = &self.model;
        div().child("ConversationsPanel")
    }
}
