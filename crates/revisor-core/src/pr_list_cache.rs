use crate::provider::types::{PrSearchFilters, PrState, PullRequest};
use std::collections::VecDeque;

const MAX_QUERIES: usize = 12;

#[derive(Clone, PartialEq, Eq)]
pub struct PrListQuery {
    repo: String,
    state: String,
    search: String,
    authors: Vec<String>,
    labels: Vec<String>,
    assignees: Vec<String>,
    review_status: Option<String>,
}

impl PrListQuery {
    pub fn new(owner: &str, repo: &str, state: PrState, filters: &PrSearchFilters, search: &str) -> Self {
        let sorted = |values: &[String]| {
            let mut values = values.to_vec();
            values.sort();
            values
        };
        Self {
            repo: format!("{owner}/{repo}"),
            state: state.to_string(),
            search: search.trim().to_string(),
            authors: sorted(&filters.authors),
            labels: sorted(&filters.labels),
            assignees: sorted(&filters.assignees),
            review_status: filters.review_status.clone(),
        }
    }
}

#[derive(Clone)]
pub struct PrListSnapshot {
    pub items: Vec<PullRequest>,
    pub page: u32,
    pub has_next_page: bool,
    pub total_count: Option<u64>,
}

#[derive(Default)]
pub struct PrListCache(VecDeque<(PrListQuery, PrListSnapshot)>);

impl PrListCache {
    pub fn get(&mut self, query: &PrListQuery) -> Option<PrListSnapshot> {
        let index = self.0.iter().position(|(key, _)| key == query)?;
        let entry = self.0.remove(index)?;
        let snapshot = entry.1.clone();
        self.0.push_back(entry);
        Some(snapshot)
    }

    pub fn insert(&mut self, query: PrListQuery, snapshot: PrListSnapshot) {
        if let Some(index) = self.0.iter().position(|(key, _)| key == &query) {
            self.0.remove(index);
        }
        self.0.push_back((query, snapshot));
        if self.0.len() > MAX_QUERIES {
            self.0.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::types::{PrSearchFilters, PrState};

    #[test]
    fn restores_the_full_query_snapshot_and_ignores_filter_order() {
        let mut cache = PrListCache::default();
        let mut filters = PrSearchFilters { labels: vec!["bug".into(), "urgent".into()], ..Default::default() };
        let key = PrListQuery::new("o", "r", PrState::Closed, &filters, " fix ");
        let snapshot = PrListSnapshot { items: Vec::new(), page: 3, has_next_page: true, total_count: Some(83) };
        cache.insert(key, snapshot);
        filters.labels.reverse();
        let same = PrListQuery::new("o", "r", PrState::Closed, &filters, "fix");
        let restored = cache.get(&same).unwrap();
        assert_eq!(restored.page, 3);
        assert_eq!(restored.total_count, Some(83));
        assert!(restored.has_next_page);
        assert!(cache.get(&PrListQuery::new("o", "r", PrState::Open, &filters, "fix")).is_none());
    }

    #[test]
    fn evicts_the_oldest_query_after_the_limit() {
        let mut cache = PrListCache::default();
        for index in 0..13 {
            cache.insert(
                PrListQuery::new("o", "r", PrState::Open, &PrSearchFilters::default(), &index.to_string()),
                PrListSnapshot { items: Vec::new(), page: 1, has_next_page: false, total_count: Some(index) },
            );
        }
        assert!(cache.get(&PrListQuery::new("o", "r", PrState::Open, &PrSearchFilters::default(), "0")).is_none());
        assert!(cache.get(&PrListQuery::new("o", "r", PrState::Open, &PrSearchFilters::default(), "12")).is_some());
    }
}
