use super::{configure_chart, GALLERY_CHART_HEIGHT};
use crate::state::GalleryState;
use fission::charts::Chart;
use fission::prelude::*;
use fission::three_d::Scene3D;

pub(crate) enum GalleryRenderable {
    Chart(Chart),
    Scene3d(Scene3D),
}

impl From<GalleryRenderable> for Widget {
    fn from(renderable: GalleryRenderable) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();

        match renderable {
            GalleryRenderable::Chart(chart) => configure_chart(chart, view)
                .height(GALLERY_CHART_HEIGHT)
                .into(),
            GalleryRenderable::Scene3d(scene) => scene.height(GALLERY_CHART_HEIGHT).into(),
        }
    }
}

pub(crate) trait GalleryBuildExt {
    fn in_gallery(self) -> GalleryRenderable;
}

impl GalleryBuildExt for Chart {
    fn in_gallery(self) -> GalleryRenderable {
        GalleryRenderable::Chart(self)
    }
}

impl GalleryBuildExt for Scene3D {
    fn in_gallery(self) -> GalleryRenderable {
        GalleryRenderable::Scene3d(self)
    }
}
