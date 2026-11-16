use crate::core::{self, GlobalState};
/// Build-only convenience handle. Event handlers receive ReducerContext instead.
pub struct ViewCx<R: GlobalState> {
    pub build: core::BuildCtxHandle<R>,
    pub view: core::ViewHandle<R>,
}
impl<R: GlobalState> std::ops::Deref for ViewCx<R> {
    type Target = core::BuildCtxHandle<R>;
    fn deref(&self) -> &Self::Target {
        &self.build
    }
}
impl<R: GlobalState> ViewCx<R> {
    pub fn tr(&self, key: &str) -> String {
        self.view.tr(key)
    }
}
