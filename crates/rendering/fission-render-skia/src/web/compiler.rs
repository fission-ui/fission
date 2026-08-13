use fission_render::paragraph::ParagraphFrameBindings;
use fission_render::resource::{ResourceId, ResourceSnapshot};
use fission_render::{Color, RenderScene};
use fission_skia_sys::web::ResourceHandle;

use super::convert::web_command;
use super::WebCompileError;
#[cfg(test)]
use crate::compiler::compile_scene;
use crate::compiler::{compile_scene_for_web, CompiledRasterFrame};
use crate::paragraph_engine::CanvasKitParagraphDrawDataRegistry;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompiledWebFrame {
    pub(crate) commands: Vec<fission_skia_sys::web::WebCommand>,
    pub(crate) encoded_commands: Vec<u8>,
    pub(crate) source_operations: u64,
    pub(crate) reused_layers: u64,
}

/// Lowers one retained Fission scene into the bounded CanvasKit command stream.
///
/// Native and Web deliberately share the existing Skia scene compiler. This
/// adapter converts only its backend-neutral paint values; native Skia handles
/// fail explicitly and will be replaced by Web resource handles in the Web
/// resource compilation stage.
#[cfg(test)]
pub(crate) fn compile_web_scene(
    scene: &RenderScene,
    scale_factor: f64,
    clear_color: Color,
) -> Result<CompiledWebFrame, WebCompileError> {
    let compiled = compile_scene(scene, scale_factor, clear_color)?;
    encode_web_frame(compiled, &|_| None)
}

/// Compiles resource-bearing Web frames against the exact transactional
/// CanvasKit handle table selected by the driver for this submission.
pub(crate) fn compile_web_scene_with_resources(
    scene: &RenderScene,
    scale_factor: f64,
    clear_color: Color,
    resources: &ResourceSnapshot,
    paragraph_bindings: Option<&ParagraphFrameBindings>,
    paragraph_draw_data: Option<&CanvasKitParagraphDrawDataRegistry>,
    resolve_resource: &dyn Fn(ResourceId) -> Option<ResourceHandle>,
) -> Result<CompiledWebFrame, WebCompileError> {
    let compiled = compile_scene_for_web(
        scene,
        scale_factor,
        clear_color,
        resources,
        paragraph_bindings,
        paragraph_draw_data,
    )?;
    encode_web_frame(compiled, resolve_resource)
}

fn encode_web_frame(
    compiled: CompiledRasterFrame,
    resolve_resource: &dyn Fn(ResourceId) -> Option<ResourceHandle>,
) -> Result<CompiledWebFrame, WebCompileError> {
    let commands = compiled
        .frame
        .commands
        .iter()
        .map(|command| web_command(command, resolve_resource))
        .collect::<Result<Vec<_>, _>>()?;
    let encoded_commands = fission_skia_sys::web::encode_commands(&commands)?;
    Ok(CompiledWebFrame {
        commands,
        encoded_commands,
        source_operations: compiled.source_operations,
        reused_layers: compiled.reused_layers,
    })
}
