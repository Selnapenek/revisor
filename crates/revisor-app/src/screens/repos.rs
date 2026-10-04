use gpui_kit::*;

pub struct ReposScreen;

impl ReposScreen {
    pub fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for ReposScreen {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child("Repos")
    }
}
