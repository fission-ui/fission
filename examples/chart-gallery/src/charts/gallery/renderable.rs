use super::{configure_chart, GALLERY_CHART_HEIGHT};
use crate::state::GalleryState;
use fission::charts::Chart;
use fission::prelude::*;
use fission::three_d::Scene3D;

fission_component! {
    pub(crate) GalleryChartRenderer<GalleryState> {
        props {
            chart: Chart,
        }
    }

    view(renderable, _state, cx) =>
        configure_chart(renderable.chart.clone(), cx.view).height(GALLERY_CHART_HEIGHT)
}

fission_component! {
    pub(crate) GallerySceneRenderer<()> {
        props {
            scene: Scene3D,
        }
    }

    view(renderable, _state, _cx) =>
        renderable.scene.clone().height(GALLERY_CHART_HEIGHT)
}

pub(crate) trait GalleryBuildExt {
    fn in_gallery(self) -> Widget;
}

impl GalleryBuildExt for Chart {
    fn in_gallery(self) -> Widget {
        GalleryChartRenderer::new(self).into()
    }
}

impl GalleryBuildExt for Scene3D {
    fn in_gallery(self) -> Widget {
        GallerySceneRenderer::new(self).into()
    }
}
