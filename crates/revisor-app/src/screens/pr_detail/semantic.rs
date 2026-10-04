use super::state::PrModel;
use gpui_kit::*;

pub struct SemanticPanel {
    model: Entity<PrModel>,
}

impl SemanticPanel {
    pub fn new(model: Entity<PrModel>, _: &mut Window, _: &mut Context<Self>) -> Self {
        Self { model }
    }
}

impl Render for SemanticPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let _ = &self.model;
        div().child("SemanticPanel")
    }
}
