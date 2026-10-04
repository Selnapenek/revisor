use super::state::PrModel;
use gpui_kit::*;

pub struct FilesPanel {
    model: Entity<PrModel>,
}

impl FilesPanel {
    pub fn new(model: Entity<PrModel>, _: &mut Window, _: &mut Context<Self>) -> Self {
        Self { model }
    }

    pub fn focus_filter(&mut self, _: &mut Window, _: &mut Context<Self>) {}
}

impl Render for FilesPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let _ = &self.model;
        div().child("FilesPanel")
    }
}
