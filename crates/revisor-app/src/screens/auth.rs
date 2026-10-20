//! GitHub device-flow sign-in, with a personal access token fallback.

use crate::{app, backend, theme};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState},
    *,
};
use gpui_kit::{prelude::*, *};
use revisor_core::{auth, auth::DeviceLoginInfo, provider::types::ProviderUser};

const BROWSER_HINT: &str = "Copy the address and code to sign in on GitHub.";

pub struct AuthScreen {
    login: Option<DeviceLoginInfo>,
    starting: bool,
    error: Option<SharedString>,
    show_pat: bool,
    pat: Entity<InputState>,
    pat_submitting: bool,
    gh_submitting: bool,
    _login_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl AuthScreen {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let pat = cx.new(|cx| {
            InputState::new(window, cx).placeholder("ghp_xxxxxxxxxxxxxxxxxxxx").masked(true)
        });
        let subscriptions = vec![
            cx.subscribe_in(&pat, window, |this, _, event, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.connect_with_pat(window, cx);
                }
            }),
            // Leaving the screen cancels a pending device login.
            cx.on_release(|this, cx| {
                if let Some(login) = this.login.take() {
                    let id = login.attempt_id;
                    let task = backend::call(cx, |s| async move { auth::cancel_github_login(&s, id).await });
                    cx.background_spawn(task).detach();
                }
            }),
        ];
        Self {
            login: None,
            starting: false,
            error: None,
            show_pat: false,
            pat,
            pat_submitting: false,
            gh_submitting: false,
            _login_task: None,
            _subscriptions: subscriptions,
        }
    }

    fn begin_login(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.starting = true;
        self.error = None;
        let start = backend::call(cx, |s| async move { auth::start_github_login(&s).await });
        self._login_task = Some(cx.spawn_in(window, async move |this, cx| {
            let info = match start.await {
                Ok(info) => info,
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.starting = false;
                        this.error = Some(e.into());
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };
            let id = info.attempt_id.clone();
            let wait = this.update(cx, |this, cx| {
                cx.open_url(&info.verification_uri);
                this.login = Some(info);
                cx.notify();
                backend::call(cx, |s| async move { auth::wait_for_github_login(&s, id).await })
            });
            let Ok(wait) = wait else { return };
            let result = wait.await;
            this.update_in(cx, |this, window, cx| {
                this.starting = false;
                let attempt = this.login.take();
                match result {
                    Ok(user) => this.signed_in(user, window, cx),
                    // A cancelled attempt already cleared `login`; stay quiet then.
                    Err(e) if attempt.is_some() => this.error = Some(e.into()),
                    Err(_) => {}
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn cancel_login(&mut self, cx: &mut Context<Self>) {
        self.starting = false;
        self.error = None;
        if let Some(login) = self.login.take() {
            let id = login.attempt_id;
            let task = backend::call(cx, |s| async move { auth::cancel_github_login(&s, id).await });
            cx.background_spawn(task).detach();
        }
        cx.notify();
    }

    fn connect_with_pat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pat_submitting {
            return;
        }
        self.pat_submitting = true;
        self.error = None;
        let token = self.pat.read(cx).value().to_string();
        backend::spawn(
            window,
            cx,
            |s| async move { auth::sign_in_with_pat(&s, token).await },
            |this, result, window, cx| {
                this.pat_submitting = false;
                match result {
                    Ok(user) => this.signed_in(user, window, cx),
                    Err(e) => this.error = Some(e.into()),
                }
            },
        )
        .detach();
        cx.notify();
    }

    fn connect_with_gh_cli(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.gh_submitting {
            return;
        }
        self.gh_submitting = true;
        self.error = None;
        backend::spawn(
            window,
            cx,
            |s| async move { auth::sign_in_with_gh_cli(&s).await },
            |this, result, window, cx| {
                this.gh_submitting = false;
                match result {
                    Ok(user) => this.signed_in(user, window, cx),
                    Err(e) => this.error = Some(e.into()),
                }
            },
        )
        .detach();
        cx.notify();
    }

    fn signed_in(&mut self, user: ProviderUser, window: &mut Window, cx: &mut Context<Self>) {
        window.activate_window();
        app::set_user(Some(user), window, cx);
    }
}

impl Render for AuthScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let error = self
            .error
            .clone()
            .map(|e| div().text_sm().text_color(theme.danger).child(e));

        let body = if self.show_pat {
            v_flex()
                .gap_3()
                .child(div().text_sm().font_medium().child("GitHub Personal Access Token"))
                .child(Input::new(&self.pat).mask_toggle())
                .children(error)
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("Needs the `repo` scope for private repositories and review actions."),
                )
                .child(
                    Button::new("pat-connect")
                        .primary()
                        .large()
                        .w_full()
                        .label("Connect to GitHub")
                        .loading(self.pat_submitting)
                        .on_click(cx.listener(|this, _, window, cx| this.connect_with_pat(window, cx))),
                )
                .child(
                    Button::new("pat-back")
                        .ghost()
                        .w_full()
                        .label("Back to OAuth sign-in")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.show_pat = false;
                            this.error = None;
                            this.pat.update(cx, |pat, cx| pat.set_value("", window, cx));
                            cx.notify();
                        })),
                )
        } else if let Some(login) = &self.login {
            let uri = login.verification_uri.clone();
            v_flex()
                .gap_4()
                .child(
                    v_flex()
                        .items_center()
                        .py_4()
                        .gap_3()
                        .border_y_1()
                        .border_color(theme.border)
                        .child(theme::eyebrow("Enter this code on GitHub", cx))
                        .child(h_flex().gap_1().font_family(theme.mono_font_family.clone()).text_xl().children(
                            login.user_code.chars().map(|c| {
                                if c == '-' {
                                    div().px_1().text_color(theme.muted_foreground).child("-")
                                } else {
                                    div()
                                        .w(px(32.))
                                        .h(px(40.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .border_1()
                                        .border_color(theme.border)
                                        .child(c.to_string())
                                }
                            }),
                        ))
                        .child(
                            div()
                                .font_family(theme.mono_font_family.clone())
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(uri.clone()),
                        ),
                )
                .children(error)
                .child(
                    Button::new("copy-code")
                        .large()
                        .w_full()
                        .label("Copy code")
                        .on_click({
                            let code = login.user_code.clone();
                            move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(code.clone()))
                        }),
                )
                .child(
                    Button::new("open-github")
                        .large()
                        .w_full()
                        .label("Open GitHub again")
                        .on_click(move |_, _, cx| cx.open_url(&uri)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_center()
                        .text_color(theme.muted_foreground)
                        .child(format!("Complete sign-in in your browser. Revisor will continue when you approve access. {BROWSER_HINT}")),
                )
                .child(
                    Button::new("cancel-login")
                        .ghost()
                        .w_full()
                        .label("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_login(cx))),
                )
        } else {
            v_flex()
                .gap_4()
                .child(
                    div()
                        .text_sm()
                        .text_center()
                        .text_color(theme.muted_foreground)
                        .child("Sign in with GitHub to review your repositories. Revisor needs access to your repositories, including private repositories."),
                )
                .children(error)
                .child(
                    Button::new("sign-in")
                        .primary()
                        .large()
                        .w_full()
                        .label("Sign in with GitHub for this session")
                        .loading(self.starting)
                        .on_click(cx.listener(|this, _, window, cx| this.begin_login(window, cx))),
                )
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            Button::new("try-sample")
                                .outline()
                                .large()
                                .w_full()
                                .label("Try a sample pull request")
                                .on_click(|_, window, cx| app::start_sample(window, cx)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_center()
                                .text_color(theme.muted_foreground)
                                .child("Every feature, no sign-in or AI setup. Nothing leaves this computer."),
                        ),
                )
                .child(
                    Button::new("use-gh")
                        .ghost()
                        .w_full()
                        .label("Use GitHub CLI (stay signed in)")
                        .loading(self.gh_submitting)
                        .on_click(cx.listener(|this, _, window, cx| this.connect_with_gh_cli(window, cx))),
                )
                .child(
                    Button::new("use-pat")
                        .ghost()
                        .w_full()
                        .label("Use a personal access token for this session")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.show_pat = true;
                            this.error = None;
                            this.pat.update(cx, |pat, cx| pat.focus(window, cx));
                            cx.notify();
                        })),
                )
        };

        div().size_full().flex().items_center().justify_center().child(
            theme::island(cx)
                .w(px(520.))
                .h_auto()
                .p_6()
                .child(v_flex()
                .w_full()
                .gap_8()
                .child(
                    v_flex()
                        .items_center()
                        .gap_1()
                        .child(
                            h_flex()
                                .gap_3()
                                .child(div().w(px(2.)).h(px(32.)).bg(theme.primary))
                                .child(div().font_family(theme.mono_font_family.clone()).text_size(rems(2.75)).line_height(rems(3.)).child("Revisor")),
                        )
                        .child(
                            div()
                                .font_family(theme.mono_font_family.clone())
                                .italic()
                                .text_lg()
                                .text_color(theme.muted_foreground)
                                .child("Every change, proofed."),
                        ),
                )
                .child(body)),
        )
    }
}
