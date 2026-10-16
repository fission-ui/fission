use fission::prelude::*;

/// One reflected Material icon: category, name, variant, and its SVG source.
pub type IconEntry = (
    &'static str,
    &'static str,
    &'static str,
    fn() -> &'static str,
);

/// What the gallery shows after the current query and category are applied.
pub struct IconResults {
    /// Every category in the icon set, in reflection order.
    pub categories: Vec<&'static str>,
    /// The icons the filters leave visible.
    pub visible: Vec<IconEntry>,
    /// How many icons the set holds in total.
    pub total: usize,
}

#[derive(Default, Clone, Debug)]
pub struct State {
    /// Case-insensitive substring matched against the icon name.
    pub query: String,
    /// Selected category, or `None` for every category.
    pub category: Option<String>,
}

impl GlobalState for State {}

impl State {
    /// Whether an icon survives the current query and category filter.
    pub fn matches(&self, entry: &IconEntry) -> bool {
        self.matches_query(entry, self.query.trim())
    }

    fn matches_query(&self, entry: &IconEntry, query: &str) -> bool {
        let (category, name, _, _) = entry;
        if self
            .category
            .as_deref()
            .is_some_and(|selected| selected != *category)
        {
            return false;
        }
        query.is_empty() || contains_ignore_ascii_case(name, query)
    }
}

/// Material icon identifiers are ASCII. Compare them without allocating a
/// lowercase `String` for every one of the 10,000+ entries on each keystroke.
fn contains_ignore_ascii_case(value: &str, query: &str) -> bool {
    value
        .as_bytes()
        .windows(query.len())
        .any(|candidate| candidate.eq_ignore_ascii_case(query.as_bytes()))
}

/// Reads the icon set once and applies the filters in a single pass.
///
/// The set holds over ten thousand variants, so the categories, the surviving
/// rows, and the total all come from the same walk rather than three.
pub fn filter_icons(state: &State) -> IconResults {
    let all = fission::icons::material::all_icons();
    let total = all.len();
    let mut categories: Vec<&'static str> = Vec::new();
    let mut visible = Vec::new();
    let query = state.query.trim();

    for entry in all {
        if !categories.contains(&entry.0) {
            categories.push(entry.0);
        }
        if state.matches_query(&entry, query) {
            visible.push(entry);
        }
    }

    IconResults {
        categories,
        visible,
        total,
    }
}

#[fission_reducer(SearchChanged)]
pub fn on_search_changed(state: &mut State, ctx: &mut ReducerContext<State>) {
    let Some(change) = ctx.input.text_change() else {
        return;
    };
    state.query = change.new_text.clone();
}

#[fission_reducer(CategorySelected)]
pub fn on_category_selected(state: &mut State, category: Option<String>) {
    state.category = category;
}
