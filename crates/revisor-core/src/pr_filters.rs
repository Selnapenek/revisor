//! PR list filters: per-repo persistence, presets, known users, client-side search.

use crate::cache::queries::{get_setting, set_setting};
use crate::provider::types::{PrSearchFilters, PrUser};
use rusqlite::Connection;

pub const REVIEW_REQUESTED_ME: &str = "review-requested:@me";

/// `(value, label)` pairs for the review status filter.
pub const REVIEW_STATUS_OPTIONS: [(&str, &str); 7] = [
    ("none", "No reviews"),
    ("required", "Review required"),
    ("approved", "Approved"),
    ("changes_requested", "Changes requested"),
    ("reviewed-by:@me", "Reviewed by you"),
    ("-reviewed-by:@me", "Not reviewed by you"),
    (REVIEW_REQUESTED_ME, "Awaiting your review"),
];

fn key(repo_key: &str) -> String {
    format!("pr-filters:{repo_key}")
}

/// Saved filters for `owner/repo`; empty when missing, malformed or unreadable.
pub fn load(conn: &Connection, repo_key: &str) -> PrSearchFilters {
    get_setting(conn, &key(repo_key))
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

/// Best effort: a failed write keeps the filters usable in memory.
pub fn save(conn: &Connection, repo_key: &str, filters: &PrSearchFilters) {
    if let Ok(json) = serde_json::to_string(filters) {
        let _ = set_setting(conn, &key(repo_key), &json);
    }
}

pub fn with_author_preset(filters: &PrSearchFilters, login: &str) -> PrSearchFilters {
    PrSearchFilters { authors: vec![login.to_string()], ..filters.clone() }
}

pub fn with_review_requested_preset(filters: &PrSearchFilters) -> PrSearchFilters {
    PrSearchFilters { review_status: Some(REVIEW_REQUESTED_ME.into()), ..filters.clone() }
}

/// Add `value` if absent, remove it if present.
pub fn toggle(values: &mut Vec<String>, value: &str) {
    match values.iter().position(|v| v == value) {
        Some(i) => {
            values.remove(i);
        }
        None => values.push(value.to_string()),
    }
}

/// Add users not yet known (by login), keeping the list sorted by login.
pub fn merge_users(known: &mut Vec<PrUser>, incoming: impl IntoIterator<Item = PrUser>) {
    for user in incoming {
        if !known.iter().any(|u| u.login == user.login) {
            known.push(user);
        }
    }
    known.sort_by(|a, b| a.login.cmp(&b.login));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::schema::run_migrations;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        conn
    }

    fn json(f: &PrSearchFilters) -> serde_json::Value {
        serde_json::to_value(f).unwrap()
    }

    fn sample() -> PrSearchFilters {
        PrSearchFilters {
            authors: vec!["someone".into()],
            labels: vec!["bug".into()],
            assignees: vec!["teammate".into()],
            review_status: Some("approved".into()),
        }
    }

    #[test]
    fn filters_restore_independently_for_each_repository() {
        let conn = db();
        let first = sample();
        let second = PrSearchFilters { review_status: Some("required".into()), ..Default::default() };
        save(&conn, "org/one", &first);
        save(&conn, "org/two", &second);
        assert_eq!(json(&load(&conn, "org/one")), json(&first));
        assert_eq!(json(&load(&conn, "org/two")), json(&second));
    }

    #[test]
    fn missing_or_malformed_saved_filters_return_an_empty_filter_set() {
        let conn = db();
        set_setting(&conn, "pr-filters:org/broken", "not json").unwrap();
        set_setting(
            &conn,
            "pr-filters:org/invalid",
            r#"{"authors":[1],"labels":[],"assignees":[],"review_status":null}"#,
        )
        .unwrap();
        for repo in ["org/missing", "org/broken", "org/invalid"] {
            assert!(load(&conn, repo).is_empty(), "{repo}");
        }
    }

    #[test]
    fn storage_errors_do_not_stop_filters_from_loading() {
        let conn = Connection::open_in_memory().unwrap(); // no settings table
        assert!(load(&conn, "org/repo").is_empty());
        save(&conn, "org/repo", &PrSearchFilters::default());
    }

    #[test]
    fn author_preset_replaces_authors_and_keeps_other_filters() {
        let expected = PrSearchFilters { authors: vec!["me".into()], ..sample() };
        assert_eq!(json(&with_author_preset(&sample(), "me")), json(&expected));
    }

    #[test]
    fn review_preset_sets_review_requested_and_keeps_other_filters() {
        let expected = PrSearchFilters { review_status: Some("review-requested:@me".into()), ..sample() };
        assert_eq!(json(&with_review_requested_preset(&sample())), json(&expected));
    }

    #[test]
    fn toggle_adds_and_removes() {
        let mut v = vec!["a".to_string()];
        toggle(&mut v, "b");
        toggle(&mut v, "a");
        assert_eq!(v, ["b"]);
    }

    #[test]
    fn merge_users_dedupes_and_sorts() {
        let user = |login: &str| PrUser { login: login.into(), avatar_url: String::new() };
        let mut known = vec![user("carol")];
        merge_users(&mut known, [user("bob"), user("carol"), user("alice"), user("bob")]);
        let logins: Vec<_> = known.iter().map(|u| u.login.as_str()).collect();
        assert_eq!(logins, ["alice", "bob", "carol"]);
    }

}
