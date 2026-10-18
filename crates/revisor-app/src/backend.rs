//! Bridge between GPUI's executor and the tokio runtime that `revisor-core` needs.

use gpui_kit::*;
use revisor_core::AppState;
use std::{future::Future, sync::Arc};

pub struct Backend {
    runtime: tokio::runtime::Runtime,
    state: Arc<AppState>,
}

impl Global for Backend {}

pub fn init(cx: &mut App) {
    let runtime = tokio::runtime::Runtime::new().expect("Failed to start tokio runtime");
    let state = Arc::new(AppState::open().expect("Failed to initialize database"));
    cx.set_global(Backend { runtime, state });
}

/// Run `f` on tokio. The returned future can be awaited on the GPUI side.
pub fn call<T, Fut>(
    cx: &App,
    f: impl FnOnce(Arc<AppState>) -> Fut,
) -> impl Future<Output = Result<T, String>> + 'static
where
    T: Send + 'static,
    Fut: Future<Output = Result<T, String>> + Send + 'static,
{
    let backend = cx.global::<Backend>();
    let handle = backend.runtime.spawn(f(backend.state.clone()));
    async move { handle.await.map_err(|e| e.to_string())? }
}

/// Like `call`, but stop the Tokio task when the GPUI caller drops its future.
pub fn call_cancellable<T, Fut>(
    cx: &App,
    f: impl FnOnce(Arc<AppState>) -> Fut,
) -> impl Future<Output = Result<T, String>> + 'static
where
    T: Send + 'static,
    Fut: Future<Output = Result<T, String>> + Send + 'static,
{
    struct AbortOnDrop(tokio::task::AbortHandle);
    impl Drop for AbortOnDrop {
        fn drop(&mut self) { self.0.abort(); }
    }
    let backend = cx.global::<Backend>();
    let task = backend.runtime.spawn(f(backend.state.clone()));
    let guard = AbortOnDrop(task.abort_handle());
    async move {
        let result = task.await.map_err(|e| e.to_string())?;
        drop(guard);
        result
    }
}

/// Run `f` on tokio and wait for it. Startup only: it blocks the UI thread.
pub fn block_on<T, Fut>(cx: &App, f: impl FnOnce(Arc<AppState>) -> Fut) -> Result<T, String>
where
    Fut: Future<Output = Result<T, String>>,
{
    let backend = cx.global::<Backend>();
    backend.runtime.block_on(f(backend.state.clone()))
}

/// Run `f` on tokio, then hand the result to `done` on the view. Drop the task to ignore the result.
pub fn spawn<V: 'static, T, Fut>(
    window: &Window,
    cx: &mut Context<V>,
    f: impl FnOnce(Arc<AppState>) -> Fut,
    done: impl FnOnce(&mut V, Result<T, String>, &mut Window, &mut Context<V>) + 'static,
) -> Task<()>
where
    T: Send + 'static,
    Fut: Future<Output = Result<T, String>> + Send + 'static,
{
    let fut = call(cx, f);
    cx.spawn_in(window, async move |this, cx| {
        let result = fut.await;
        this.update_in(cx, |view, window, cx| {
            done(view, result, window, cx);
            cx.notify();
        })
        .ok();
    })
}

/// Error to string, for `map_err`.
pub fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
