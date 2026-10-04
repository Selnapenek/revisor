use gpui_kit::*;

pub struct PrDetailScreen;

impl PrDetailScreen {
    pub fn new(_owner: SharedString, _repo: SharedString, _number: u64, _: &mut Window, _: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for PrDetailScreen {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child("PR detail")
    }
}
