mod commands;

pub use revisor_core::{
    auth, cache, diff, focus, keyring, provider, rank_findings, semantic, source_snapshot, AppState,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::open().expect("Failed to initialize database");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::auth::start_github_login,
            commands::auth::wait_for_github_login,
            commands::auth::cancel_github_login,
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
            commands::focus::focus_has_key,
            commands::focus::focus_save_key,
            commands::focus::focus_remove_key,
            commands::focus::find_focus_files,
            commands::semantic::search_semantic_diff,
            commands::rank_findings::rank_ai_findings,
            commands::source_snapshot::get_pr_head_source_snapshot,
            commands::comments::get_pr_comments,
            commands::comments::get_issue_comments,
            commands::comments::get_pr_reviews,
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
