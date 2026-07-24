use crate::components::overview_asset_details::OverviewAssetDetails;
use crate::model::FieldInspectorState;
use fission::prelude::*;

const ASSET_IMAGE_WIDTH: f32 = 320.0;
const ASSET_IMAGE_HEIGHT: f32 = 218.0;

pub struct OverviewAssetCompact;

impl From<OverviewAssetCompact> for Widget {
    fn from(_: OverviewAssetCompact) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let order = view.state().selected_order();

        Column {
            gap: Some(view.env().theme.tokens.spacing.m),
            align_items: ir_op::AlignItems::Start,
            children: widgets![
                Image::network(order.asset.photo_url)
                    .size(ASSET_IMAGE_WIDTH, ASSET_IMAGE_HEIGHT)
                    .fit(ir_op::ImageFit::Cover)
                    .semantic_label(order.asset.name),
                OverviewAssetDetails,
            ],
            ..Default::default()
        }
        .into()
    }
}
