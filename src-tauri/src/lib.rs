mod cache;
mod commands;
mod diff;
mod highlight;
mod keyring;
mod provider;

use cache::Cache;
use provider::GitProvider;
use tokio::sync::{Mutex, RwLock};

pub struct AppState {
    pub provider: RwLock<Option<Box<dyn GitProvider>>>,
    pub cache: Mutex<Cache>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cache = Cache::open().expect("Failed to initialize database");

    let state = AppState {
        provider: RwLock::new(None),
        cache: Mutex::new(cache),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::auth::get_stored_token,
            commands::auth::validate_and_store_token,
            commands::auth::logout,
            commands::auth::restore_session,
            commands::repos::get_saved_repos,
            commands::repos::save_repo,
            commands::repos::remove_repo,
            commands::repos::list_user_repos,
            commands::repos::search_repos,
            commands::pull_requests::list_pull_requests,
            commands::pull_requests::get_pull_request,
            commands::pull_requests::search_pull_requests,
            commands::pull_requests::list_repo_labels,
            commands::pull_requests::get_rate_limit,
            commands::commits::get_pr_commits,
            commands::commits::get_compare_files,
            commands::files::get_pr_files,
            commands::files::get_file_content,
            commands::comments::get_pr_comments,
            commands::comments::add_comment,
            commands::comments::reply_to_comment,
            commands::comments::update_comment,
            commands::comments::delete_comment,
            commands::comments::get_thread_resolutions,
            commands::comments::resolve_thread,
            commands::comments::unresolve_thread,
            commands::review::get_reviewed_files,
            commands::review::mark_file_reviewed,
            commands::review::unmark_file_reviewed,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running Revisor");
}
