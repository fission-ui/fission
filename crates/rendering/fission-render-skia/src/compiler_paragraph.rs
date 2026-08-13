//! Backend-specific paragraph resources bound to one compiled frame.

use std::sync::Arc;

use fission_ir::WidgetId;
use fission_layout::ParagraphResult;
use fission_render::paragraph::ParagraphFrameBindings;
use fission_skia_sys::{web::ResourceHandle, ParagraphDrawData};

use crate::paragraph_draw_data::{
    ParagraphDrawDataError, ParagraphDrawDataRegistry, ParagraphFrameDrawData,
};
use crate::paragraph_engine::{CanvasKitParagraphDrawData, CanvasKitParagraphDrawDataRegistry};

/// Backend-owned paint data paired with the authoritative paragraph geometry.
pub(crate) enum CompiledParagraphDrawData {
    Native(Arc<ParagraphDrawData>),
    CanvasKit(ResourceHandle),
}

pub(crate) struct CompiledParagraph {
    pub(crate) result: Arc<ParagraphResult>,
    pub(crate) draw_data: CompiledParagraphDrawData,
}

pub(crate) enum ParagraphCompilation {
    Native(ParagraphFrameDrawData<ParagraphDrawData>),
    CanvasKit(ParagraphFrameDrawData<CanvasKitParagraphDrawData>),
}

impl ParagraphCompilation {
    pub(crate) fn native(
        bindings: Option<&ParagraphFrameBindings>,
        registry: &ParagraphDrawDataRegistry<ParagraphDrawData>,
    ) -> Result<Option<Self>, ParagraphDrawDataError> {
        bind(bindings, registry).map(|bound| bound.map(Self::Native))
    }

    pub(crate) fn canvaskit(
        bindings: Option<&ParagraphFrameBindings>,
        registry: Option<&CanvasKitParagraphDrawDataRegistry>,
    ) -> Result<Option<Self>, ParagraphDrawDataError> {
        match (bindings, registry) {
            (Some(bindings), Some(registry)) => {
                bind(Some(bindings), registry).map(|bound| bound.map(Self::CanvasKit))
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn get(&self, node_id: WidgetId) -> Option<CompiledParagraph> {
        match self {
            Self::Native(frame) => frame.get(node_id).map(|bound| CompiledParagraph {
                result: Arc::clone(&bound.result),
                draw_data: CompiledParagraphDrawData::Native(Arc::clone(&bound.data)),
            }),
            Self::CanvasKit(frame) => frame.get(node_id).map(|bound| CompiledParagraph {
                result: Arc::clone(&bound.result),
                draw_data: CompiledParagraphDrawData::CanvasKit(bound.data.handle()),
            }),
        }
    }

    pub(crate) fn native_frame(&self) -> Option<&ParagraphFrameDrawData<ParagraphDrawData>> {
        match self {
            Self::Native(frame) => Some(frame),
            Self::CanvasKit(_) => None,
        }
    }
}

fn bind<T>(
    bindings: Option<&ParagraphFrameBindings>,
    registry: &ParagraphDrawDataRegistry<T>,
) -> Result<Option<ParagraphFrameDrawData<T>>, ParagraphDrawDataError> {
    bindings
        .map(|bindings| {
            let frame = registry.bind_frame(
                bindings
                    .iter()
                    .map(|(node_id, result)| (*node_id, Arc::clone(result))),
            )?;
            registry.retain_results(bindings.iter().map(|(_, result)| result.as_ref()))?;
            Ok(frame)
        })
        .transpose()
}
