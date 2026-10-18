//! Changed-file suggestions for the diff header menu.

use super::state::PrModel;
use revisor_core::files::{related_files, Relation};

pub fn results(model: &PrModel, file: usize) -> Vec<(String, Relation)> {
    related_files(&model.files, file)
        .into_iter()
        .map(|(index, relation)| (model.files[index].filename.clone(), relation))
        .collect()
}

pub fn reason(relation: Relation) -> &'static str {
    match relation {
        Relation::Test => "Matching test name",
        Relation::Source => "Matching source name",
        Relation::SameName => "Same filename stem",
        Relation::Mentions => "Name mentioned in changed lines",
        Relation::SameFolder => "Same folder",
    }
}
