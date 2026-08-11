use fission_render::{Color, RenderScene};

use super::convert::web_command;
use super::WebCompileError;
use crate::compiler::compile_scene;

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
pub(crate) fn compile_web_scene(
    scene: &RenderScene,
    scale_factor: f64,
    clear_color: Color,
) -> Result<CompiledWebFrame, WebCompileError> {
    let compiled = compile_scene(scene, scale_factor, clear_color)?;
    let commands = compiled
        .frame
        .commands
        .iter()
        .map(web_command)
        .collect::<Result<Vec<_>, _>>()?;
    let encoded_commands = fission_skia_sys::web::encode_commands(&commands)?;
    Ok(CompiledWebFrame {
        commands,
        encoded_commands,
        source_operations: compiled.source_operations,
        reused_layers: compiled.reused_layers,
    })
}
