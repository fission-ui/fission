use super::{
    LineMetric, ParagraphCaretStop, ParagraphCluster, ParagraphGlyph, ParagraphSelectionBox,
    ResolvedParagraphLayout, RichTextInlineBox,
};
use fission_ir::WidgetId;
use std::collections::HashMap;
use std::sync::Arc;

pub(super) const RESOLVED_PARAGRAPH_CACHE_MAX_ENTRIES: usize = 512;
pub(super) const RESOLVED_PARAGRAPH_CACHE_MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ResolvedParagraphCacheKey {
    pub(super) node_id: WidgetId,
    pub(super) width_bits: Option<u32>,
}

#[derive(Debug)]
pub(super) struct ResolvedParagraphCacheEntry {
    paragraph: Arc<ResolvedParagraphLayout>,
    estimated_bytes: usize,
}

#[derive(Debug, Default)]
pub(super) struct ResolvedParagraphCache {
    pub(super) entries: HashMap<ResolvedParagraphCacheKey, ResolvedParagraphCacheEntry>,
    pub(super) estimated_bytes: usize,
}

impl ResolvedParagraphCache {
    pub(super) fn get(
        &self,
        key: ResolvedParagraphCacheKey,
    ) -> Option<Arc<ResolvedParagraphLayout>> {
        self.entries
            .get(&key)
            .map(|cached| cached.paragraph.clone())
    }

    pub(super) fn insert(
        &mut self,
        key: ResolvedParagraphCacheKey,
        paragraph: Arc<ResolvedParagraphLayout>,
    ) {
        let estimated_bytes = Self::estimate_bytes(&paragraph);
        if estimated_bytes > RESOLVED_PARAGRAPH_CACHE_MAX_BYTES {
            self.remove(key);
            return;
        }

        self.remove(key);
        while self.entries.len() >= RESOLVED_PARAGRAPH_CACHE_MAX_ENTRIES
            || self.estimated_bytes.saturating_add(estimated_bytes)
                > RESOLVED_PARAGRAPH_CACHE_MAX_BYTES
        {
            let Some(oldest_key) = self.entries.keys().next().copied() else {
                break;
            };
            self.remove(oldest_key);
        }

        self.estimated_bytes = self.estimated_bytes.saturating_add(estimated_bytes);
        self.entries.insert(
            key,
            ResolvedParagraphCacheEntry {
                paragraph,
                estimated_bytes,
            },
        );
    }

    pub(super) fn remove_node(&mut self, node_id: WidgetId) {
        let keys = self
            .entries
            .keys()
            .filter(|key| key.node_id == node_id)
            .copied()
            .collect::<Vec<_>>();
        for key in keys {
            self.remove(key);
        }
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.estimated_bytes = 0;
    }

    fn remove(&mut self, key: ResolvedParagraphCacheKey) {
        if let Some(entry) = self.entries.remove(&key) {
            self.estimated_bytes = self.estimated_bytes.saturating_sub(entry.estimated_bytes);
        }
    }

    fn estimate_bytes(paragraph: &ResolvedParagraphLayout) -> usize {
        std::mem::size_of::<ResolvedParagraphLayout>()
            .saturating_add(std::mem::size_of::<ResolvedParagraphCacheKey>())
            .saturating_add(std::mem::size_of::<ResolvedParagraphCacheEntry>())
            .saturating_add(
                paragraph
                    .lines
                    .capacity()
                    .saturating_mul(std::mem::size_of::<LineMetric>()),
            )
            .saturating_add(
                paragraph
                    .inline_boxes
                    .capacity()
                    .saturating_mul(std::mem::size_of::<RichTextInlineBox>()),
            )
            .saturating_add(
                paragraph
                    .clusters
                    .capacity()
                    .saturating_mul(std::mem::size_of::<ParagraphCluster>()),
            )
            .saturating_add(
                paragraph
                    .glyphs
                    .capacity()
                    .saturating_mul(std::mem::size_of::<ParagraphGlyph>()),
            )
            .saturating_add(
                paragraph
                    .caret_stops
                    .capacity()
                    .saturating_mul(std::mem::size_of::<ParagraphCaretStop>()),
            )
            .saturating_add(
                paragraph
                    .selection_boxes
                    .capacity()
                    .saturating_mul(std::mem::size_of::<ParagraphSelectionBox>()),
            )
    }
}
