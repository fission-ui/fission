//! Per-frame paragraph results shared by layout, interaction, and paint.

use std::collections::BTreeMap;
use std::sync::Arc;

use fission_ir::WidgetId;
use fission_layout::ParagraphResult;

/// Immutable paragraph results bound to their exact paint-node identities.
///
/// The selected paragraph engine publishes these results during final layout.
/// An interactive frame borrows the resulting table so a graphics backend can
/// paint backend-owned draw data from the same result used for sizing, hit
/// testing, selection, caret placement, and IME geometry.
#[derive(Debug, Clone, Default)]
pub struct ParagraphFrameBindings {
    results: BTreeMap<WidgetId, Arc<ParagraphResult>>,
}

impl ParagraphFrameBindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        node_id: WidgetId,
        result: Arc<ParagraphResult>,
    ) -> Option<Arc<ParagraphResult>> {
        self.results.insert(node_id, result)
    }

    pub fn get(&self, node_id: WidgetId) -> Option<&Arc<ParagraphResult>> {
        self.results.get(&node_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&WidgetId, &Arc<ParagraphResult>)> {
        self.results.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    pub fn len(&self) -> usize {
        self.results.len()
    }
}
