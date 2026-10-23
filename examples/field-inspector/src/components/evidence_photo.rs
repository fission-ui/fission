use crate::model::FieldInspectorState;
use fission::prelude::*;
const COMPACT_IMAGE_MAX_WIDTH: f32 = 420.0;
const COMPACT_IMAGE_ASPECT_RATIO: f32 = 0.48;
const EXPANDED_IMAGE_WIDTH: f32 = 520.0;
const EXPANDED_IMAGE_HEIGHT: f32 = 320.0;
const COMPACT_RESERVED_WIDTH: f32 = 96.0;
fission_component! {
    pub EvidencePhoto<FieldInspectorState> { compact: bool }
    |photo, state, cx| {
        let order = state.selected_order();
        let width = if photo.compact { (cx.view.viewport_size().width - COMPACT_RESERVED_WIDTH).clamp(0.0, COMPACT_IMAGE_MAX_WIDTH) } else { EXPANDED_IMAGE_WIDTH };
        let height = if photo.compact { width * COMPACT_IMAGE_ASPECT_RATIO } else { EXPANDED_IMAGE_HEIGHT };
    } => Container(
        if let Some(bytes) = &state.photo_preview {
            Image::memory(bytes.clone()).size(width, height).fit(ir_op::ImageFit::Cover).semantic_label("Captured evidence photo")
        } else {
            Image::network(order.asset.photo_url).size(width, height).fit(ir_op::ImageFit::Cover).semantic_label(order.asset.name)
        }
    ).bg(cx.view.env().theme.tokens.colors.background).border_radius(cx.view.env().theme.tokens.radii.xl)
}
