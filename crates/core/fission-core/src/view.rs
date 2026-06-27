//! Read-only view and selector pattern.
//!
//! During widget conversion, the framework provides a scoped [`View`] that gives
//! read-only access to the current [`GlobalState`], theme, i18n registry,
//! layout snapshot, and motion values. Widgets use this to decide what
//! to render without side effects.

use crate::{
    env::VideoState, Env, GlobalState, LayoutRect, LayoutSize, LayoutSnapshot, MotionPropertyId,
    MotionValue, RuntimeState, ViewHandle,
};
use fission_i18n::I18nRegistry;
use fission_ir::WidgetId;
use fission_layout::BoxConstraints;
use fission_theme::Theme;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{BuildHasher, Hash};

/// Read-only access to application state and environment during widget building.
///
/// `View` is the primary way widgets read data. It is parameterised over the
/// concrete [`GlobalState`] type `S`, giving type-safe access to `state` while
/// also exposing the theme, i18n registry, layout snapshot from the previous
/// frame, and motion values.
///
/// # Example
///
/// ```rust,ignore
/// let name = &view.state().user_name;
/// let theme = view.theme();
/// ```
pub struct View<'a, S: GlobalState> {
    /// Reference to the current application state.
    pub state: &'a S,
    /// Runtime interaction, scroll, text-edit, and motion state.
    pub runtime: &'a RuntimeState,
    /// Environment (theme, i18n, viewport size, locale).
    pub env: &'a Env,
    /// Layout snapshot from the previous frame, if available.
    pub layout: Option<&'a LayoutSnapshot>,
}

impl<'a, S: GlobalState> View<'a, S> {
    pub fn new(
        state: &'a S,
        runtime: &'a RuntimeState,
        env: &'a Env,
        layout: Option<&'a LayoutSnapshot>,
    ) -> Self {
        Self {
            state,
            runtime,
            env,
            layout,
        }
    }

    pub fn state(&self) -> &S {
        self.state
    }

    pub fn runtime(&self) -> &RuntimeState {
        self.runtime
    }

    pub fn env(&self) -> &Env {
        self.env
    }

    pub fn layout(&self) -> Option<&LayoutSnapshot> {
        self.layout
    }

    pub fn theme(&self) -> &Theme {
        &self.env.theme
    }
    pub fn i18n(&self) -> &I18nRegistry {
        &self.env.i18n
    }

    pub fn get_rect(&self, id: WidgetId) -> Option<LayoutRect> {
        let node_id: WidgetId = id.into();
        self.layout.and_then(|l| l.get_node_rect(node_id))
    }

    pub fn get_constraints(&self, id: WidgetId) -> Option<BoxConstraints> {
        let node_id: WidgetId = id.into();
        self.layout.and_then(|l| l.get_node_constraints(node_id))
    }

    pub fn viewport_size(&self) -> LayoutSize {
        self.env.viewport_size
    }

    pub fn select<R>(&self, selector: impl FnOnce(&S) -> R) -> R {
        selector(self.state)
    }

    pub fn motion_value(&self, widget_id: WidgetId, property: MotionPropertyId) -> MotionValue {
        self.runtime
            .motion
            .values
            .get(&(widget_id, property.clone()))
            .cloned()
            .unwrap_or_else(|| property.default_value())
    }

    pub fn motion_scalar(&self, widget_id: WidgetId, property: MotionPropertyId) -> f32 {
        self.runtime.motion.scalar_value(widget_id, property)
    }

    pub fn video_state(&self, widget_id: WidgetId) -> Option<&VideoState> {
        self.runtime.video.states.get(&widget_id)
    }
}

/// A read-only generated view over a value.
///
/// `ValueView` is used by generated global-state views for scalar and
/// collection fields. It borrows the current state; [`get`](Self::get) clones
/// only when the caller explicitly asks for an owned value.
#[derive(Clone, Copy, Debug)]
pub struct ValueView<'a, T> {
    value: &'a T,
}

impl<'a, T> ValueView<'a, T> {
    pub fn new(value: &'a T) -> Self {
        Self { value }
    }

    pub fn borrow(&self) -> &'a T {
        self.value
    }

    pub fn map<R>(&self, selector: impl FnOnce(&T) -> R) -> ComputedView<R> {
        ComputedView::new(selector(self.value))
    }
}

impl<T: Clone> ValueView<'_, T> {
    pub fn get(&self) -> T {
        self.value.clone()
    }
}

impl<'a, T> ValueView<'a, Vec<T>> {
    pub fn len(&self) -> usize {
        self.value.len()
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'a, T> {
        self.value.iter()
    }
}

impl<'a, T> IntoIterator for ValueView<'a, Vec<T>> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.value.iter()
    }
}

/// A computed read-only value produced from a generated state view.
#[derive(Clone, Debug)]
pub struct ComputedView<T> {
    value: T,
}

impl<T> ComputedView<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }

    pub fn borrow(&self) -> &T {
        &self.value
    }

    pub fn get(self) -> T {
        self.value
    }

    pub fn map<R>(self, selector: impl FnOnce(&T) -> R) -> ComputedView<R> {
        ComputedView::new(selector(&self.value))
    }
}

/// Maps a state field type to the view returned by generated view accessors.
///
/// `#[derive(FissionStateView)]` implements this trait for nested state
/// structs. Built-in scalar and collection types map to [`ValueView`].
pub trait FissionViewField {
    type View<'a>
    where
        Self: 'a;

    fn view_field<'a>(value: &'a Self) -> Self::View<'a>;
}

macro_rules! scalar_view_field {
    ($($ty:ty),* $(,)?) => {
        $(
            impl FissionViewField for $ty {
                type View<'a> = ValueView<'a, Self> where Self: 'a;

                fn view_field<'a>(value: &'a Self) -> Self::View<'a> {
                    ValueView::new(value)
                }
            }
        )*
    };
}

scalar_view_field!(
    bool, char, String, usize, u8, u16, u32, u64, u128, isize, i8, i16, i32, i64, i128, f32, f64
);

impl<T> FissionViewField for Vec<T> {
    type View<'a>
        = ValueView<'a, Self>
    where
        Self: 'a;

    fn view_field<'a>(value: &'a Self) -> Self::View<'a> {
        ValueView::new(value)
    }
}

impl<T> FissionViewField for Option<T> {
    type View<'a>
        = ValueView<'a, Self>
    where
        Self: 'a;

    fn view_field<'a>(value: &'a Self) -> Self::View<'a> {
        ValueView::new(value)
    }
}

impl<T, const N: usize> FissionViewField for [T; N] {
    type View<'a>
        = ValueView<'a, Self>
    where
        Self: 'a;

    fn view_field<'a>(value: &'a Self) -> Self::View<'a> {
        ValueView::new(value)
    }
}

impl<T: Ord> FissionViewField for BTreeSet<T> {
    type View<'a>
        = ValueView<'a, Self>
    where
        Self: 'a;

    fn view_field<'a>(value: &'a Self) -> Self::View<'a> {
        ValueView::new(value)
    }
}

impl<K: Ord, V> FissionViewField for BTreeMap<K, V> {
    type View<'a>
        = ValueView<'a, Self>
    where
        Self: 'a;

    fn view_field<'a>(value: &'a Self) -> Self::View<'a> {
        ValueView::new(value)
    }
}

impl<T: Eq + Hash, S: BuildHasher> FissionViewField for HashSet<T, S> {
    type View<'a>
        = ValueView<'a, Self>
    where
        Self: 'a;

    fn view_field<'a>(value: &'a Self) -> Self::View<'a> {
        ValueView::new(value)
    }
}

impl<K: Eq + Hash, V, S: BuildHasher> FissionViewField for HashMap<K, V, S> {
    type View<'a>
        = ValueView<'a, Self>
    where
        Self: 'a;

    fn view_field<'a>(value: &'a Self) -> Self::View<'a> {
        ValueView::new(value)
    }
}

/// A selector that derives a value from a [`ViewHandle`].
///
/// Selectors extract and transform data from state so widgets can depend on
/// derived values without coupling to the full state shape.
///
/// # Example
///
/// ```rust,ignore
/// struct ItemCount;
/// impl Selector<MyState> for ItemCount {
///     type Output = usize;
///     fn select(view: ViewHandle<MyState>) -> usize {
///         view.state().items.len()
///     }
/// }
///
/// // In a widget:
/// let count: usize = view.select_with::<ItemCount>();
/// ```
pub trait Selector<S: GlobalState> {
    /// The type produced by the selector.
    type Output;
    /// Extract the value from the given view handle.
    fn select(view: ViewHandle<S>) -> Self::Output;
}
