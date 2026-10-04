use gpui_kit::*;

pub struct PrListScreen;

impl PrListScreen {
    pub fn new(_owner: SharedString, _repo: SharedString, _: &mut Window, _: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for PrListScreen {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child("PR list")
    }
}
