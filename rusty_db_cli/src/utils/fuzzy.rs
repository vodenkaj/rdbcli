use sublime_fuzzy::best_match;

pub fn filter_fuzzy_matches(query: &str, values: &[String]) -> Vec<String> {
    values
        .iter()
        .filter(|value| best_match(query, value).is_some())
        .cloned()
        .collect()
}
