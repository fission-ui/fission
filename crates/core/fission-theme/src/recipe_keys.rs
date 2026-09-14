//! Typed recipe, part and scalar keys, and the theme accessors that read recipes through them.

use crate::{ComponentRecipe, ResolvedComponentStyle, Theme};

/// Typed names for the component recipes, parts and scalars Fission's design
/// systems declare, generated from the default design system.
///
/// Read a recipe with its key, such as `theme.recipe(recipes::Accordion)`, and
/// its parts with that component's part enum, such as
/// `recipes::AccordionPart::Indicator`. A misspelt or undeclared name is then a
/// compile error instead of a silently empty style. Every supplied design
/// system declares all of [`REQUIRED`](recipes::REQUIRED).
#[allow(clippy::all)]
pub mod recipes {
    include!(concat!(env!("OUT_DIR"), "/generated_recipe_keys.rs"));
}

/// A component recipe named by a generated key in [`recipes`].
pub trait RecipeKey: Copy {
    /// The name the design system declares the recipe under.
    const NAME: &'static str;
    /// The parts this component's recipe declares.
    type Part: RecipePartKey;
    /// The scalars this component's recipe declares.
    type Scalar: RecipeScalarKey;
}

/// A named part of one component's recipe.
pub trait RecipePartKey: Copy {
    /// The name the design system declares the part under.
    fn name(self) -> &'static str;
}

/// A named scalar of one component's recipe.
pub trait RecipeScalarKey: Copy {
    /// The name the design system declares the scalar under.
    fn name(self) -> &'static str;
}

/// The part key of a recipe that declares no parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoParts {}

impl RecipePartKey for NoParts {
    fn name(self) -> &'static str {
        match self {}
    }
}

/// The scalar key of a recipe that declares no scalars.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoScalars {}

impl RecipeScalarKey for NoScalars {
    fn name(self) -> &'static str {
        match self {}
    }
}

/// A component recipe read through its key, so its parts and scalars can only
/// be named with that component's own keys.
///
/// Dereferences to the [`ComponentRecipe`] for its base style, sizes and states.
pub struct Recipe<'a, K: RecipeKey> {
    recipe: &'a ComponentRecipe,
    key: std::marker::PhantomData<K>,
}

impl<K: RecipeKey> Clone for Recipe<'_, K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: RecipeKey> Copy for Recipe<'_, K> {}

impl<K: RecipeKey> std::fmt::Debug for Recipe<'_, K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(K::NAME).field(self.recipe).finish()
    }
}

impl<'a, K: RecipeKey> Recipe<'a, K> {
    /// Returns a part, or an empty style when the design system does not declare it.
    pub fn part(&self, part: K::Part) -> &'a ResolvedComponentStyle {
        self.recipe.part_named(part.name())
    }

    /// Returns a part only when the design system declares it.
    pub fn try_part(&self, part: K::Part) -> Option<&'a ResolvedComponentStyle> {
        self.recipe.try_part_named(part.name())
    }

    /// Returns a scalar the recipe declares.
    pub fn scalar(&self, scalar: K::Scalar) -> Option<f32> {
        self.recipe.scalar_named(scalar.name())
    }

    /// The untyped recipe, for code that works across components.
    pub fn component_recipe(&self) -> &'a ComponentRecipe {
        self.recipe
    }
}

impl<K: RecipeKey> std::ops::Deref for Recipe<'_, K> {
    type Target = ComponentRecipe;

    fn deref(&self) -> &ComponentRecipe {
        self.recipe
    }
}

impl Theme {
    /// Returns the design system's recipe for `name`.
    ///
    /// Returns an empty recipe when the design system does not declare one, so
    /// a widget can read parts and fall back per field rather than branching on
    /// whether a recipe exists.
    ///
    /// A build-time check requires every supplied design system to declare the
    /// recipes widgets read, so an empty result here means either an
    /// application design system that chose not to override this component, or
    /// a widget reading a name nobody declares.
    pub fn recipe<K: RecipeKey>(&self, _key: K) -> Recipe<'_, K> {
        Recipe {
            recipe: self.recipe_named(K::NAME),
            key: std::marker::PhantomData,
        }
    }

    /// Returns the recipe for `key` only when the design system declares it.
    pub fn try_recipe<K: RecipeKey>(&self, _key: K) -> Option<Recipe<'_, K>> {
        self.try_recipe_named(K::NAME).map(|recipe| Recipe {
            recipe,
            key: std::marker::PhantomData,
        })
    }

    /// Returns the recipe declared under `name`, or an empty recipe.
    ///
    /// Prefer [`recipe`](Self::recipe); this is for names known only at run
    /// time, such as an application design system's own components.
    pub fn recipe_named(&self, name: &str) -> &ComponentRecipe {
        static EMPTY: std::sync::OnceLock<ComponentRecipe> = std::sync::OnceLock::new();
        self.components
            .recipes
            .get(name)
            .unwrap_or_else(|| EMPTY.get_or_init(ComponentRecipe::default))
    }

    /// Returns the recipe declared under `name` only when the design system declares it.
    pub fn try_recipe_named(&self, name: &str) -> Option<&ComponentRecipe> {
        self.components.recipes.get(name)
    }
}
