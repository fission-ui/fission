use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex};

use fission_ir::WidgetId;

use super::{
    ParagraphCapabilities, ParagraphDescription, ParagraphEngine, ParagraphError, ParagraphResult,
};

#[derive(Clone)]
struct PublishedParagraph {
    description: ParagraphDescription,
    result: Arc<ParagraphResult>,
}

/// Shared authority for paragraph results produced by the selected profile.
///
/// Layout publishes the result that determined a text node's size. Input, IME,
/// accessibility, and paint then retrieve that same immutable value by the
/// node's stable Fission identity. The store owns no backend-private object;
/// backend draw data remains behind [`super::ParagraphDrawDataId`].
pub struct ParagraphResultStore {
    engine: Arc<dyn ParagraphEngine>,
    published: Mutex<HashMap<WidgetId, PublishedParagraph>>,
}

impl ParagraphResultStore {
    pub fn new(engine: Arc<dyn ParagraphEngine>) -> Self {
        Self {
            engine,
            published: Mutex::new(HashMap::new()),
        }
    }

    pub fn capabilities(&self) -> ParagraphCapabilities {
        self.engine.capabilities()
    }

    /// Shapes a description without publishing it for a node.
    ///
    /// Measurement probes use this method. Only the final recorded layout is
    /// published, so downstream consumers cannot observe provisional geometry.
    pub fn layout(
        &self,
        description: &ParagraphDescription,
    ) -> Result<Arc<ParagraphResult>, ParagraphError> {
        description.validate()?;
        self.engine
            .capabilities()
            .require_all(description.required_capabilities())?;
        let result = self.engine.layout(description)?;
        if result.text() != description.text {
            return Err(ParagraphError::invalid_result(
                "text",
                "paragraph result text differs from its normalized description",
            ));
        }
        Ok(Arc::new(result))
    }

    /// Publishes the immutable result used by a node's final layout.
    pub fn publish(
        &self,
        node_id: WidgetId,
        description: ParagraphDescription,
        result: Arc<ParagraphResult>,
    ) -> Result<(), ParagraphError> {
        description.validate()?;
        if result.text() != description.text {
            return Err(ParagraphError::invalid_result(
                "text",
                "cannot publish paragraph geometry for different source text",
            ));
        }
        self.published.lock().unwrap().insert(
            node_id,
            PublishedParagraph {
                description,
                result,
            },
        );
        Ok(())
    }

    /// Shapes and publishes a node's final paragraph in one operation.
    pub fn layout_and_publish(
        &self,
        node_id: WidgetId,
        description: ParagraphDescription,
    ) -> Result<Arc<ParagraphResult>, ParagraphError> {
        let result = self.layout(&description)?;
        self.publish(node_id, description, result.clone())?;
        Ok(result)
    }

    /// Returns the exact result most recently published by final layout.
    pub fn get(&self, node_id: WidgetId) -> Option<Arc<ParagraphResult>> {
        self.published
            .lock()
            .unwrap()
            .get(&node_id)
            .map(|entry| entry.result.clone())
    }

    /// Returns a result only when every normalized shaping input still matches.
    pub fn get_matching(
        &self,
        node_id: WidgetId,
        description: &ParagraphDescription,
    ) -> Option<Arc<ParagraphResult>> {
        self.published
            .lock()
            .unwrap()
            .get(&node_id)
            .filter(|entry| entry.description == *description)
            .map(|entry| entry.result.clone())
    }

    /// Drops results for nodes no longer present in the current layout graph.
    pub fn retain_nodes(&self, nodes: impl IntoIterator<Item = WidgetId>) {
        let live = nodes.into_iter().collect::<HashSet<_>>();
        self.published
            .lock()
            .unwrap()
            .retain(|node_id, _| live.contains(node_id));
    }

    pub fn clear(&self) {
        self.published.lock().unwrap().clear();
    }
}

impl fmt::Debug for ParagraphResultStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParagraphResultStore")
            .field("capabilities", &self.engine.capabilities())
            .field("published", &self.published.lock().unwrap().len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use fission_ir::op::{Color, TextDirection, TextParagraphStyle, TextStyle};

    use crate::{
        LayoutSize, ParagraphCacheKey, ParagraphCapability, ParagraphGeometry, ParagraphStyleRun,
        Utf8Range,
    };

    use super::*;

    struct MockEngine {
        layouts: AtomicUsize,
    }

    impl ParagraphEngine for MockEngine {
        fn capabilities(&self) -> ParagraphCapabilities {
            ParagraphCapabilities::NONE.with(ParagraphCapability::BidirectionalText)
        }

        fn layout(
            &self,
            description: &ParagraphDescription,
        ) -> Result<ParagraphResult, ParagraphError> {
            let layout = self.layouts.fetch_add(1, Ordering::Relaxed) + 1;
            ParagraphResult::new(
                description,
                ParagraphCacheKey::new(layout as u128),
                self.capabilities(),
                ParagraphGeometry::new(LayoutSize::new(20.0, 10.0)),
                Vec::new(),
            )
        }
    }

    fn description(text: &str) -> ParagraphDescription {
        let range = Utf8Range::from_byte_offsets(0, text.len()).unwrap();
        let style = TextStyle {
            font_size: 14.0,
            color: Color::BLACK,
            underline: false,
            font_family: None,
            locale: None,
            font_weight: 400,
            font_style: Default::default(),
            line_height: None,
            letter_spacing: 0.0,
            background_color: None,
        };
        let mut paragraph = ParagraphDescription::new(
            text,
            vec![ParagraphStyleRun::new(range, style)],
            TextParagraphStyle::default(),
            Some(100.0),
        );
        paragraph.paragraph_style.text_direction = TextDirection::Ltr;
        paragraph
    }

    #[test]
    fn only_published_final_results_are_visible_to_consumers() {
        let store = ParagraphResultStore::new(Arc::new(MockEngine {
            layouts: AtomicUsize::new(0),
        }));
        let node = WidgetId::explicit("paragraph");
        let request = description("hello");

        let probe = store.layout(&request).unwrap();
        assert!(store.get(node).is_none());

        store.publish(node, request.clone(), probe.clone()).unwrap();
        assert!(Arc::ptr_eq(&probe, &store.get(node).unwrap()));
        assert!(store.get_matching(node, &request).is_some());

        let changed = description("changed");
        assert!(store.get_matching(node, &changed).is_none());
    }

    #[test]
    fn removed_nodes_do_not_leave_published_results_behind() {
        let store = ParagraphResultStore::new(Arc::new(MockEngine {
            layouts: AtomicUsize::new(0),
        }));
        let keep = WidgetId::explicit("keep");
        let remove = WidgetId::explicit("remove");
        store.layout_and_publish(keep, description("kept")).unwrap();
        store
            .layout_and_publish(remove, description("removed"))
            .unwrap();

        store.retain_nodes([keep]);

        assert!(store.get(keep).is_some());
        assert!(store.get(remove).is_none());
    }
}
