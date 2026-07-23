use crate::model::FieldInspectorState;
use fission::prelude::*;

const COMPACT_IMAGE_MAX_WIDTH: f32 = 420.0;
const COMPACT_IMAGE_ASPECT_RATIO: f32 = 0.48;
const EXPANDED_IMAGE_WIDTH: f32 = 520.0;
const EXPANDED_IMAGE_HEIGHT: f32 = 320.0;
const COMPACT_RESERVED_WIDTH: f32 = 96.0;

pub struct EvidencePhoto {
    pub compact: bool,
}

impl From<EvidencePhoto> for Widget {
    fn from(photo: EvidencePhoto) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let order = view.state().selected_order();
        let width = if photo.compact {
            (view.viewport_size().width - COMPACT_RESERVED_WIDTH)
                .max(0.0)
                .min(COMPACT_IMAGE_MAX_WIDTH)
        } else {
            EXPANDED_IMAGE_WIDTH
        };
        let height = if photo.compact {
            width * COMPACT_IMAGE_ASPECT_RATIO
        } else {
            EXPANDED_IMAGE_HEIGHT
        };
        let image: Widget = if let Some(bytes) = &view.state().photo_preview {
            Image::memory(bytes.clone())
                .size(width, height)
                .fit(ir_op::ImageFit::Cover)
                .semantic_label("Captured evidence photo")
                .into()
        } else {
            Image::network(order.asset.photo_url)
                .size(width, height)
                .fit(ir_op::ImageFit::Cover)
                .semantic_label(order.asset.name)
                .into()
        };

        Container::new(image)
            .bg(view.env().theme.tokens.colors.background)
            .border_radius(view.env().theme.tokens.radii.xl)
            .into()
    }
}
