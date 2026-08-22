//! Retained-picture lowering for compiler cache hints.

use fission_render::{DisplayList, RenderLayer};

use super::{CompileError, Compiler, PictureCompilation};
use crate::api::{RasterCommand, RasterFrame};
use crate::picture::{
    display_list_candidate, layer_candidate, layer_contents_candidate, PictureCandidate,
    PictureHintScope, PictureLookupKey,
};

impl<'a> Compiler<'a> {
    pub(super) fn compile_cached_layer(
        &mut self,
        layer: &RenderLayer,
        root_index: usize,
        node_path: &mut Vec<usize>,
        inherited_node_id: Option<fission_ir::WidgetId>,
    ) -> Result<bool, CompileError> {
        let (pictures, hint) = match (self.pictures, layer.style.cache_key) {
            (Some(pictures), Some(hint)) => (pictures, hint),
            _ => return Ok(false),
        };
        let Some(candidate) = layer_candidate(
            layer,
            inherited_node_id,
            self.scale_factor,
            self.images.map(|images| images.resources()),
            self.paragraphs
                .and_then(|paragraphs| paragraphs.native_frame()),
        ) else {
            return Ok(false);
        };
        let key = PictureLookupKey::new(PictureHintScope::Layer, hint, self.scale_factor);
        if self.reuse_picture(pictures, key, &candidate) {
            return Ok(true);
        }

        let mut child = self.subcompiler();
        child.compile_layer_uncached(layer, root_index, node_path, inherited_node_id)?;
        if child.remaining_native_saves() != 0 {
            return Ok(false);
        }
        self.finish_picture_miss(pictures, key, candidate, child);
        Ok(true)
    }

    pub(super) fn compile_cached_layer_contents(
        &mut self,
        layer: &RenderLayer,
        root_index: usize,
        node_path: &mut Vec<usize>,
        node_id: Option<fission_ir::WidgetId>,
    ) -> Result<bool, CompileError> {
        let (pictures, hint) = match (self.pictures, layer.style.content_cache_key) {
            (Some(pictures), Some(hint)) => (pictures, hint),
            _ => return Ok(false),
        };
        let Some(candidate) = layer_contents_candidate(
            layer,
            node_id,
            self.scale_factor,
            self.images.map(|images| images.resources()),
            self.paragraphs
                .and_then(|paragraphs| paragraphs.native_frame()),
        ) else {
            return Ok(false);
        };
        let key = PictureLookupKey::new(PictureHintScope::LayerContents, hint, self.scale_factor);
        if self.reuse_picture(pictures, key, &candidate) {
            return Ok(true);
        }

        let mut child = self.subcompiler();
        child.compile_layer_children(layer, root_index, node_path, node_id)?;
        if child.remaining_native_saves() != 0 {
            return Ok(false);
        }
        self.finish_picture_miss(pictures, key, candidate, child);
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn compile_cached_display_list(
        &mut self,
        hint: u64,
        list: &DisplayList,
        root_index: usize,
        node_path: &[usize],
        operation_path: &mut Vec<usize>,
        inherited_node_id: Option<fission_ir::WidgetId>,
    ) -> Result<bool, CompileError> {
        let Some(pictures) = self.pictures else {
            return Ok(false);
        };
        let Some(candidate) = display_list_candidate(
            list,
            inherited_node_id,
            self.scale_factor,
            self.images.map(|images| images.resources()),
            self.paragraphs
                .and_then(|paragraphs| paragraphs.native_frame()),
        ) else {
            return Ok(false);
        };
        let key = PictureLookupKey::new(PictureHintScope::DisplayList, hint, self.scale_factor);
        if self.reuse_picture(pictures, key, &candidate) {
            return Ok(true);
        }

        let mut child = self.subcompiler();
        child.compile_list(
            list,
            root_index,
            node_path,
            operation_path,
            inherited_node_id,
        )?;
        if child.remaining_native_saves() != 0 {
            return Ok(false);
        }
        self.finish_picture_miss(pictures, key, candidate, child);
        Ok(true)
    }

    fn reuse_picture(
        &mut self,
        pictures: PictureCompilation<'_>,
        key: PictureLookupKey,
        candidate: &PictureCandidate,
    ) -> bool {
        let Some(picture) = pictures.cache.get(key, candidate) else {
            return false;
        };
        self.commands.push(RasterCommand::DrawPicture { picture });
        self.reused_layers = self.reused_layers.saturating_add(1);
        true
    }

    fn finish_picture_miss(
        &mut self,
        pictures: PictureCompilation<'_>,
        key: PictureLookupKey,
        candidate: PictureCandidate,
        child: Compiler<'_>,
    ) {
        self.source_operations = self
            .source_operations
            .saturating_add(child.source_operations);
        self.reused_layers = self.reused_layers.saturating_add(child.reused_layers);
        let frame = RasterFrame {
            commands: child.commands,
        };
        let estimated_bytes = candidate.estimated_cache_bytes(&frame);
        if pictures.cache.can_store(estimated_bytes) {
            if let Ok(Some(picture)) = pictures.recorder.record_picture(candidate.bounds, &frame) {
                pictures
                    .cache
                    .insert(key, candidate, picture.clone(), estimated_bytes);
                self.commands.push(RasterCommand::DrawPicture { picture });
                return;
            }
        }
        self.commands.extend(frame.commands);
    }

    fn subcompiler(&self) -> Compiler<'a> {
        Compiler {
            scale_factor: self.scale_factor,
            commands: Vec::new(),
            source_operations: 0,
            reused_layers: 0,
            save_scopes: Vec::new(),
            root_opacity_layers: 0,
            paragraphs: self.paragraphs,
            images: self.images,
            svg: self.svg,
            pictures: self.pictures,
        }
    }
}

#[cfg(all(test, feature = "test-shim"))]
mod tests {
    use fission_render::{Color, DisplayList, DisplayOp, Fill, LayoutRect, RenderScene};

    use super::super::{compile_scene_inner, PictureCompilation};
    use crate::compiler::SvgCompilation;
    use crate::native::NativeSkiaApi;
    use crate::picture::SkiaPictureCache;
    use crate::svg::SkiaSvgCache;

    const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    #[test]
    fn repeated_exact_cached_scene_reports_one_reused_layer() {
        let bounds = LayoutRect::new(0.0, 0.0, 20.0, 10.0);
        let mut retained = DisplayList::new(bounds);
        retained.push(DisplayOp::DrawRect {
            rect: bounds,
            fill: Some(Fill::Solid(BLACK)),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds,
            node_id: None,
        });
        let mut list = DisplayList::new(bounds);
        list.push(DisplayOp::CachedScene {
            cache_key: 7,
            bounds,
            list: Box::new(retained),
        });
        let scene = RenderScene::from_display_list(list);
        let cache = SkiaPictureCache::with_limits(16 * 1024, 4);
        let svg_cache = SkiaSvgCache::with_budget_bytes(8 * 1024);
        let api = NativeSkiaApi;
        let compile = || {
            compile_scene_inner(
                &scene,
                1.0,
                BLACK,
                None,
                None,
                SvgCompilation::Native(&svg_cache),
                Some(PictureCompilation {
                    cache: &cache,
                    recorder: &api,
                }),
            )
            .unwrap()
        };

        let first = compile();
        let second = compile();
        assert_eq!(first.reused_layers, 0);
        assert_eq!(second.reused_layers, 1);
        assert!(matches!(
            second.frame.commands.as_slice(),
            [
                crate::api::RasterCommand::Clear(_),
                crate::api::RasterCommand::DrawPicture { .. }
            ]
        ));
    }
}
