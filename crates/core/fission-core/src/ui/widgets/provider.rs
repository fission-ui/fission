use crate::Widget;

/// Installs a typed value for descendants during widget construction.
///
/// Provider values are read with [`crate::build::read`] or
/// [`crate::build::try_read`]. The child closure is evaluated while the
/// provider is active, so custom components below it can resolve the nearest
/// value of `T`.
pub struct Provider<T, F> {
    pub value: T,
    pub child: F,
}

impl<T, F> Provider<T, F> {
    pub fn new(value: T, child: F) -> Self {
        Self { value, child }
    }
}

impl<T, F, R> From<Provider<T, F>> for Widget
where
    T: Clone + Send + Sync + 'static,
    F: FnOnce() -> R,
    R: Into<Widget>,
{
    fn from(provider: Provider<T, F>) -> Self {
        crate::build::provide(provider.value, || (provider.child)().into())
    }
}

pub fn provider<T, F>(value: T, child: F) -> Provider<T, F>
where
    T: Clone + Send + Sync + 'static,
{
    Provider::new(value, child)
}
