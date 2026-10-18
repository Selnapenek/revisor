//! Settings screen: appearance and AI review defaults.

use crate::{app, backend, theme};
use crate::screens::pr_detail;
use crate::appearance::{self, Field};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::agent::{self, Backend, Model};
use revisor_core::cache::queries;

// ponytail: curated lists filtered to installed fonts; a searchable picker over all fonts if people ask for more.
const UI_FONTS: &[&str] = &["JetBrains Mono", "Inter", "IBM Plex Sans", "Helvetica Neue", "Avenir Next", "Futura", "Georgia"];
const CODE_FONTS: &[&str] = &[
    "JetBrains Mono",
    "SF Mono",
    "Menlo",
    "Monaco",
    "Fira Code",
    "Cascadia Code",
    "Source Code Pro",
    "IBM Plex Mono",
    "Iosevka",
    "Hack",
    "Courier New",
];
const SCALES: &[&str] = &["80", "90", "100", "110", "125", "150"];

/// Dropdown options as `(value, label)`.
type Options = Vec<(SharedString, SharedString)>;

pub struct SettingsScreen {
    ui_fonts: Options,
    code_fonts: Options,
    ai_agents: Vec<AiAgent>,
    ai_error: Option<String>,
    _ai_load: Task<()>,
}

struct AiAgent {
    backend: Backend,
    source: Option<String>,
    version: String,
    models: Vec<Model>,
    model: String,
    effort: String,
}

impl AiAgent {
    fn normalize(&mut self) {
        if !self.models.iter().any(|model| model.id == self.model) {
            self.model = self.models.first().map(|model| model.id.clone()).unwrap_or_default();
        }
        if let Some(model) = self.models.iter().find(|model| model.id == self.model) {
            if !model.efforts.iter().any(|effort| effort.id == self.effort) {
                self.effort = model.default_effort.clone();
            }
        }
    }
}

#[derive(Clone, Copy)]
enum AiField { Model, Effort }

impl SettingsScreen {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let installed = cx.text_system().all_font_names();
        let fonts = |list: &[&str]| -> Options {
            list.iter()
                .filter(|f| installed.iter().any(|i| i == *f))
                .map(|f| (SharedString::from(*f), SharedString::from(*f)))
                .collect()
        };
        let mut ui_fonts: Options = vec![(appearance::SYSTEM_FONT.into(), "System".into())];
        ui_fonts.extend(fonts(UI_FONTS));
        let ai_load = backend::spawn(
            window,
            cx,
            |state| async move {
                let agents = tokio::task::spawn_blocking(|| {
                    [Backend::Claude, Backend::Codex].into_iter().map(|backend| {
                        let info = agent::cli_info(backend);
                        (backend, info, agent::models(backend))
                    }).collect::<Vec<_>>()
                }).await.map_err(backend::err)?;
                let db = state.cache.lock().await;
                agents.into_iter().map(|(backend, info, models)| {
                    let name = backend.executable();
                    let get = |kind| queries::get_setting(db.conn(), &format!("ai-{kind}:{name}"))
                        .map(Option::unwrap_or_default).map_err(backend::err);
                    let mut agent = AiAgent {
                        backend,
                        source: info.as_ref().map(|(path, _)| path.display().to_string()),
                        version: info.map(|(_, version)| version).unwrap_or_else(|| "Not installed".into()),
                        models,
                        model: get("model")?,
                        effort: get("effort")?,
                    };
                    agent.normalize();
                    Ok(agent)
                }).collect::<Result<Vec<_>, String>>()
            },
            |this: &mut Self, result, _, cx| {
                match result {
                    Ok(agents) => this.ai_agents = agents,
                    Err(error) => this.ai_error = Some(error),
                }
                cx.notify();
            },
        );
        Self { ui_fonts, code_fonts: fonts(CODE_FONTS), ai_agents: Vec::new(), ai_error: None, _ai_load: ai_load }
    }

    fn set_ai(&mut self, backend: Backend, field: AiField, value: SharedString, cx: &mut Context<Self>) {
        let Some(agent) = self.ai_agents.iter_mut().find(|agent| agent.backend == backend) else { return };
        let previous = (agent.model.clone(), agent.effort.clone());
        match field {
            AiField::Model => { agent.model = value.to_string(); agent.normalize(); }
            AiField::Effort => agent.effort = value.to_string(),
        }
        let name = backend.executable();
        let (model, effort) = (agent.model.clone(), agent.effort.clone());
        // Finish the local write before Back can restore a cached PR screen.
        self.ai_error = backend::block_on(cx, move |state| async move {
            let db = state.cache.lock().await;
            queries::set_setting(db.conn(), &format!("ai-model:{name}"), &model).map_err(backend::err)?;
            queries::set_setting(db.conn(), &format!("ai-effort:{name}"), &effort).map_err(backend::err)
        }).err();
        if self.ai_error.is_some() {
            (agent.model, agent.effort) = previous;
        }
        cx.notify();
    }

    fn ai_page(&self, backend: Backend, cx: &Context<Self>) -> SettingPage {
        let name = backend.label();
        let Some(agent) = self.ai_agents.iter().find(|agent| agent.backend == backend) else {
            let status = self.ai_error.clone().unwrap_or_else(|| "Checking CLI…".into());
            return SettingPage::new(name).resettable(false).group(SettingGroup::new().title("CLI source").item(
                SettingItem::new("Status", SettingField::render(move |_, _, _| div().child(status.clone()))),
            ));
        };
        let version = agent.version.clone();
        let source = agent.source.clone().unwrap_or_else(|| "Install the CLI to enable AI review.".into());
        let mut page = SettingPage::new(name).resettable(false).groups([
            SettingGroup::new().title("CLI source")
                .item(SettingItem::new("Version", SettingField::render(move |_, _, _| div().child(version.clone()))))
                .item(SettingItem::new("Source", SettingField::render(move |_, _, _| div().child(source.clone())))),
        ]);
        if let Some(error) = &self.ai_error { page = page.description(format!("Could not save AI settings: {error}")); }
        let Some(model) = agent.models.iter().find(|model| model.id == agent.model) else {
            return page.group(SettingGroup::new().title("Default model").item(
                SettingItem::new("Status", SettingField::render(|_, _, _| div().child("No models available"))),
            ));
        };
        let selected_model = agent.model.clone();
        let selected_effort = agent.effort.clone();
        let models = agent.models.iter().map(|model| (model.id.clone().into(), model.label.clone().into())).collect();
        let efforts = model.efforts.iter().map(|effort| (effort.id.clone().into(), effort.label.clone().into())).collect();
        let getter = cx.entity().downgrade();
        let setter = getter.clone();
        let model_field = SettingField::scrollable_dropdown(models,
            move |cx| getter.upgrade().and_then(|entity| entity.read(cx).ai_agents.iter().find(|a| a.backend == backend)
                .map(|a| SharedString::from(a.model.clone()))).unwrap_or_else(|| selected_model.clone().into()),
            move |value, cx| { setter.update(cx, |this, cx| this.set_ai(backend, AiField::Model, value, cx)).ok(); });
        let getter = cx.entity().downgrade();
        let setter = getter.clone();
        let effort_field = SettingField::dropdown(efforts,
            move |cx| getter.upgrade().and_then(|entity| entity.read(cx).ai_agents.iter().find(|a| a.backend == backend)
                .map(|a| SharedString::from(a.effort.clone()))).unwrap_or_else(|| selected_effort.clone().into()),
            move |value, cx| { setter.update(cx, |this, cx| this.set_ai(backend, AiField::Effort, value, cx)).ok(); });
        page = page.group(SettingGroup::new().title("Default model")
            .item(SettingItem::new("Model", model_field).description("Used for AI reviews of pull requests."))
            .item(SettingItem::new("Effort", effort_field)));
        page
    }
}

fn dropdown(field: Field, options: Options) -> SettingField<SharedString> {
    SettingField::dropdown(options, move |cx| appearance::get(field, cx), move |value, cx| appearance::set(field, value, cx))
        .default_value(field.default())
}

impl Render for SettingsScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let mut themes: Options = ThemeRegistry::global(cx)
            .themes()
            .keys()
            .filter_map(|name| Some((name.clone(), SharedString::from(name.strip_prefix("Revisor ")?.to_string()))))
            .collect();
        themes.sort_by(|a, b| a.1.cmp(&b.1));
        let scales = SCALES.iter().map(|s| (SharedString::from(*s), SharedString::from(format!("{s}%")))).collect();

        let appearance = SettingPage::new("Appearance").resettable(true).groups([
            SettingGroup::new().title("Theme").item(
                SettingItem::new("Theme", dropdown(Field::Theme, themes))
                    .description("Colors for the whole app, including code highlighting."),
            ),
            SettingGroup::new()
                .title("Fonts")
                .item(
                    SettingItem::new("Interface font", dropdown(Field::UiFont, self.ui_fonts.clone()))
                        .description("Body text, buttons and lists."),
                )
                .item(
                    SettingItem::new("Code font", dropdown(Field::CodeFont, self.code_fonts.clone()))
                        .description("Diffs, paths, numbers, labels and titles."),
                ),
            SettingGroup::new().title("Scaling").item(
                SettingItem::new("Interface scale", dropdown(Field::Scale, scales)).description("Size of all text and rows."),
            ),
            SettingGroup::new()
                .title("Help")
                .item(
                    SettingItem::new(
                        "Quick tips",
                        SettingField::switch(appearance::tips, appearance::set_tips).default_value(true),
                    )
                    .description("Shortcuts for the current screen, in the status bar (⌘/)."),
                )
                .item(
                    SettingItem::new(
                        "Keyboard shortcuts",
                        SettingField::render(|_, _, _| {
                            Button::new("shortcuts")
                                .small()
                                .outline()
                                .label("Show")
                                .on_click(|_, window, cx| pr_detail::open_shortcuts(window, cx))
                        }),
                    )
                    .description("Every key for reviewing a pull request. Press ? on a pull request to open it there."),
                ),
        ]);

        let header = h_flex()
            .flex_shrink_0()
            .gap_3()
            .px_4()
            .py_3()
            .child(
                Button::new("back")
                    .ghost()
                    .small()
                    .label("← Back")
                    .tooltip_with_action("Back", &app::GoBack, None)
                    .on_click(|_, window, cx| app::go_back(window, cx)),
            )
            .child(div().font_family(theme.mono_font_family.clone()).text_2xl().child("Settings"));
        v_flex()
            .size_full()
            .child(header)
            .child(div().flex_1().overflow_hidden().mt_2().child(theme::island(cx).child(
                Settings::new("settings").page(appearance).page(self.ai_page(Backend::Claude, cx)).page(self.ai_page(Backend::Codex, cx))
            )))
    }
}
