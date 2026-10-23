use crate::components::overview_asset_details::OverviewAssetDetails;
use crate::model::FieldInspectorState;
use fission::prelude::*;
const ASSET_IMAGE_WIDTH: f32 = 210.0;
const ASSET_IMAGE_HEIGHT: f32 = 142.0;
fission_component! {
    pub OverviewAssetExpanded<FieldInspectorState> {}
    |_c, state, cx| { let order = state.selected_order(); } =>
        Row(
            gap: cx.view.env().theme.tokens.spacing.m,
            align_items: ir_op::AlignItems::Start,
            children: [
                Image::network(order.asset.photo_url).size(ASSET_IMAGE_WIDTH, ASSET_IMAGE_HEIGHT).fit(ir_op::ImageFit::Cover).semantic_label(order.asset.name),
                OverviewAssetDetails(),
            ],
        )
}
