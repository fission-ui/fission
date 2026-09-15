//! Control density: how much room interactive controls take.
//!
//! A design system declares its control sizes once, at comfortable density.
//! [`Theme::with_density`] derives the compact and spacious variants by moving
//! every control height one [`SizingTokens::density_step`] down or up, so any
//! DSP package gets all three without declaring them. Heights never drop below
//! [`SizingTokens::min_pointer_target`], and only control-sized heights move:
//! dividers, progress tracks and large surfaces keep their declared size.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::{
    ComponentSize, ComponentStateStyles, ComponentTheme, ResolvedComponentStyle, Theme, Tokens,
};

/// How much room controls take.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Density {
    /// One step smaller: data-dense tools, tables, pro desktop apps.
    Compact,
    /// The design system's declared sizes.
    #[default]
    Comfortable,
    /// One step larger: touch-first and accessibility-focused apps.
    Spacious,
}

impl Density {
    fn level(self) -> i32 {
        match self {
            Self::Compact => -1,
            Self::Comfortable => 0,
            Self::Spacious => 1,
        }
    }
}

/// Recipes for controls whose heights follow density. Display components such
/// as badges, avatars and progress bars keep their declared size.
const CONTROL_RECIPES: &[&str] = &[
    "button",
    "date_picker",
    "dropdown",
    "input",
    "menu",
    "number_input",
    "pagination",
    "select",
    "tabs",
    "tag",
    "time_picker",
];

/// Heights at or above this are surfaces (alerts, empty states), not controls.
const LARGEST_CONTROL_HEIGHT: f32 = 64.0;

impl Theme {
    /// This theme with controls sized for `density`.
    ///
    /// Calling it again with another density moves from the current density,
    /// not from the original, and the same theme at the same density is built
    /// once and then shared.
    pub fn with_density(&self, density: Density) -> Theme {
        if self.tokens.density == density {
            return self.clone();
        }
        let key = (
            Arc::as_ptr(&self.components) as usize,
            self.tokens.density,
            density,
        );
        let cache = CACHE.get_or_init(Mutex::default);
        if let Ok(cache) = cache.lock() {
            if let Some(entry) = cache.get(&key) {
                // The cached entry keeps its source components alive, so the
                // pointer cannot have been reused by another theme; the tokens
                // check catches an app that edited tokens in place.
                if entry.source_tokens == self.tokens {
                    return entry.theme.clone();
                }
            }
        }

        let theme = self.densified(density);
        if let Ok(mut cache) = cache.lock() {
            if cache.len() >= MAX_CACHED_THEMES {
                cache.clear();
            }
            cache.insert(
                key,
                CachedTheme {
                    _source: self.components.clone(),
                    source_tokens: self.tokens.clone(),
                    theme: theme.clone(),
                },
            );
        }
        theme
    }

    fn densified(&self, density: Density) -> Theme {
        let sizing = &self.tokens.sizing;
        let step = (density.level() - self.tokens.density.level()) as f32;
        let adjust = Adjust {
            delta: sizing.density_step * step,
            floor: sizing.min_pointer_target,
        };

        let mut theme = self.clone();
        theme.tokens.density = density;
        let sizing = &mut theme.tokens.sizing;
        for height in [
            &mut sizing.control_sm,
            &mut sizing.control_md,
            &mut sizing.control_lg,
            &mut sizing.control_xl,
        ] {
            *height = adjust.height(*height);
        }
        adjust.components(theme.components_mut());
        theme
    }
}

struct CachedTheme {
    _source: Arc<ComponentTheme>,
    source_tokens: Tokens,
    theme: Theme,
}

type CacheKey = (usize, Density, Density);

const MAX_CACHED_THEMES: usize = 32;

static CACHE: OnceLock<Mutex<HashMap<CacheKey, CachedTheme>>> = OnceLock::new();

#[derive(Clone, Copy)]
struct Adjust {
    delta: f32,
    floor: f32,
}

impl Adjust {
    fn height(self, height: f32) -> f32 {
        if height < self.floor || height > LARGEST_CONTROL_HEIGHT {
            return height;
        }
        (height + self.delta).max(self.floor)
    }

    fn style(self, style: &mut ResolvedComponentStyle) {
        // Square controls (pagination items, close buttons) stay square.
        let square = style.width.is_some() && style.width == style.height;
        if let Some(height) = style.height.as_mut() {
            *height = self.height(*height);
        }
        if square {
            style.width = style.height;
        }
        if let Some(min_height) = style.min_height.as_mut() {
            *min_height = self.height(*min_height);
        }
    }

    fn states(self, states: &mut ComponentStateStyles) {
        self.style(&mut states.default);
        for state in [
            &mut states.hover,
            &mut states.active,
            &mut states.focus,
            &mut states.disabled,
            &mut states.error,
            &mut states.selected,
            &mut states.loading,
        ]
        .into_iter()
        .flatten()
        {
            self.style(state);
        }
    }

    fn sizes(self, sizes: &mut [(ComponentSize, ResolvedComponentStyle)]) {
        for (_, style) in sizes {
            self.style(style);
        }
    }

    fn components(self, components: &mut ComponentTheme) {
        components.button.height = self.height(components.button.height);
        self.sizes(&mut components.button.sizes);
        for (_, states) in &mut components.button.hierarchies {
            self.states(states);
        }

        components.text_input.height = self.height(components.text_input.height);
        self.sizes(&mut components.text_input.sizes);

        self.sizes(&mut components.select.sizes);
        self.states(&mut components.select.trigger_states);

        self.sizes(&mut components.menu.trigger_sizes);
        self.states(&mut components.menu.trigger_states);
        self.states(&mut components.menu.item_states);
        self.states(&mut components.menu.destructive_item_states);

        self.style(&mut components.pagination.item_style);
        self.style(&mut components.pagination.selected_style);
        self.style(&mut components.pagination.ellipsis_style);

        let tabs = &mut components.tabs;
        self.sizes(&mut tabs.sizes);
        self.states(&mut tabs.states);
        self.style(&mut tabs.track_style);
        for (_, presentation) in &mut tabs.presentations {
            self.sizes(&mut presentation.sizes);
            self.states(&mut presentation.states);
            self.style(&mut presentation.track_style);
        }

        let recipes = Arc::make_mut(&mut components.recipes);
        for name in CONTROL_RECIPES {
            let Some(recipe) = recipes.get_mut(*name) else {
                continue;
            };
            self.style(&mut recipe.base);
            for part in recipe.parts.values_mut() {
                self.style(part);
            }
            for size in recipe.sizes.values_mut() {
                self.style(size);
            }
            self.states(&mut recipe.states);
        }
        // A modal's close button is a control even though the modal is not.
        if let Some(close) = recipes
            .get_mut("modal")
            .and_then(|modal| modal.parts.get_mut("close"))
        {
            self.style(close);
        }
    }
}
