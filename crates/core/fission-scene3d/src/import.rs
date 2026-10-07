//! Build-time static glTF/GLB import into retained scene resources.

use std::collections::BTreeMap;

use base64::Engine;
use fission_scene::{
    AssetHandle, AssetId, Bounds3, MaterialAsset, ModelAsset, Rgba, TextureAsset, Transform3, Vec2,
    Vec3,
};

use crate::geometry::{bounds_from_positions, transform_bounds, union_bounds};
use crate::{
    AlphaMode3D, Material3D, MaterialModel3D, Mesh3D, MeshId, MeshVertex3D, Model3D, ModelId,
    ModelNode3D, ResourceId, Scene3DResources, Texture3D, TextureId, TextureSampling3D,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GltfImportError {
    pub source: String,
    pub message: String,
}

impl std::fmt::Display for GltfImportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.source, self.message)
    }
}

impl std::error::Error for GltfImportError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GltfImportOptions {
    pub model_id: ModelId,
    pub first_mesh_id: u64,
    pub first_texture_id: u64,
    pub first_material_id: u64,
    pub first_asset_id: u64,
    pub revision: u64,
}

impl Default for GltfImportOptions {
    fn default() -> Self {
        Self {
            model_id: ModelId(1),
            first_mesh_id: 1,
            first_texture_id: 1,
            first_material_id: 1,
            first_asset_id: 1,
            revision: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedTexture {
    pub id: TextureId,
    pub source_uri: Option<String>,
    pub mime_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportedGltf {
    pub resources: Scene3DResources,
    pub model: ModelId,
    /// Encoded image payloads for the build pipeline to validate/decode/package.
    pub textures: Vec<ImportedTexture>,
}

pub struct GltfImporter;

impl GltfImporter {
    /// Imports a static glTF or GLB document.
    ///
    /// `resolve` receives each external buffer or image URI. The importer also
    /// accepts embedded GLB buffers and base64 data URIs. It deliberately
    /// rejects animation, skinning, morph targets, sparse attributes, and
    /// non-triangle primitives with an error naming `source`.
    pub fn import<F>(
        source: impl Into<String>,
        bytes: &[u8],
        options: GltfImportOptions,
        mut resolve: F,
    ) -> Result<ImportedGltf, GltfImportError>
    where
        F: FnMut(&str) -> Result<Vec<u8>, String>,
    {
        let source = source.into();
        let document = gltf::Gltf::from_slice(bytes).map_err(|error| GltfImportError {
            source: source.clone(),
            message: format!("invalid glTF/GLB document: {error}"),
        })?;
        if document.animations().next().is_some() || document.skins().next().is_some() {
            return Err(error(
                &source,
                "skeletal animation and skinning are outside the static-model MVP",
            ));
        }

        let mut buffers = Vec::new();
        for buffer in document.buffers() {
            let data = match buffer.source() {
                gltf::buffer::Source::Bin => document.blob.clone().ok_or_else(|| {
                    error(
                        &source,
                        "GLB declares a binary buffer but contains no binary chunk",
                    )
                })?,
                gltf::buffer::Source::Uri(uri) => resolve_uri(uri, &mut resolve)
                    .map_err(|message| error(&source, format!("buffer {uri}: {message}")))?,
            };
            if data.len() < buffer.length() {
                return Err(error(
                    &source,
                    format!(
                        "buffer {} is {} bytes but declares {} bytes",
                        buffer.index(),
                        data.len(),
                        buffer.length()
                    ),
                ));
            }
            buffers.push(data);
        }

        let mut resources = Scene3DResources::default();
        let mut imported_textures = Vec::new();
        let mut texture_ids = BTreeMap::new();
        for image in document.images() {
            let (uri, mime_type, image_bytes) = match image.source() {
                gltf::image::Source::Uri { uri, mime_type } => (
                    Some(uri.to_owned()),
                    mime_type.map(str::to_owned),
                    resolve_uri(uri, &mut resolve)
                        .map_err(|message| error(&source, format!("image {uri}: {message}")))?,
                ),
                gltf::image::Source::View { view, mime_type } => {
                    let buffer = &buffers[view.buffer().index()];
                    let start = view.offset();
                    let end = start.saturating_add(view.length());
                    let bytes = buffer.get(start..end).ok_or_else(|| {
                        error(
                            &source,
                            format!("image buffer view {} exceeds its buffer", view.index()),
                        )
                    })?;
                    (None, Some(mime_type.to_owned()), bytes.to_vec())
                }
            };
            let id = TextureId(options.first_texture_id + image.index() as u64);
            texture_ids.insert(image.index(), id);
            resources.textures.insert(
                id,
                Texture3D {
                    revision: options.revision,
                    asset: AssetHandle::<TextureAsset>::new(AssetId(
                        options.first_asset_id + image.index() as u64,
                    )),
                    sampling: TextureSampling3D::Linear,
                    srgb: true,
                },
            );
            imported_textures.push(ImportedTexture {
                id,
                source_uri: uri,
                mime_type,
                bytes: image_bytes,
            });
        }

        let mut material_ids = BTreeMap::new();
        for material in document.materials() {
            let Some(index) = material.index() else {
                continue;
            };
            let id = ResourceId(options.first_material_id + index as u64);
            let pbr = material.pbr_metallic_roughness();
            let base = pbr.base_color_factor();
            let emissive = material.emissive_factor();
            let base_color_texture = pbr.base_color_texture().and_then(|texture| {
                texture_ids
                    .get(&texture.texture().source().index())
                    .copied()
            });
            let emissive_texture = material.emissive_texture().and_then(|texture| {
                texture_ids
                    .get(&texture.texture().source().index())
                    .copied()
            });
            let (alpha_mode, alpha_cutoff) = match material.alpha_mode() {
                gltf::material::AlphaMode::Opaque => (AlphaMode3D::Opaque, 0.5),
                gltf::material::AlphaMode::Mask => {
                    (AlphaMode3D::Mask, material.alpha_cutoff().unwrap_or(0.5))
                }
                gltf::material::AlphaMode::Blend => (AlphaMode3D::Blend, 0.5),
            };
            resources.materials.insert(
                id,
                Material3D {
                    revision: options.revision,
                    asset: material.index().map(|index| {
                        AssetHandle::<MaterialAsset>::new(AssetId(
                            options.first_asset_id + imported_textures.len() as u64 + index as u64,
                        ))
                    }),
                    model: MaterialModel3D::MetallicRoughness {
                        metallic: pbr.metallic_factor(),
                        roughness: pbr.roughness_factor(),
                    },
                    base_color: Rgba::new(base[0], base[1], base[2], base[3]),
                    base_color_texture,
                    emissive_color: Rgba::new(emissive[0], emissive[1], emissive[2], 1.0),
                    emissive_texture,
                    alpha_mode,
                    alpha_cutoff,
                    double_sided: material.double_sided(),
                },
            );
            material_ids.insert(material.index(), id);
        }
        let default_material_id = ResourceId(
            options.first_material_id
                + document
                    .materials()
                    .filter_map(|material| material.index())
                    .max()
                    .map_or(0, |index| index as u64 + 1),
        );
        resources.materials.insert(
            default_material_id,
            Material3D {
                revision: options.revision,
                ..Material3D::default()
            },
        );
        material_ids.insert(None, default_material_id);

        let mut model_nodes = Vec::new();
        let mut model_bounds = None;
        let mut next_mesh = options.first_mesh_id;
        let root_nodes = document
            .default_scene()
            .or_else(|| document.scenes().next())
            .map(|scene| scene.nodes().collect::<Vec<_>>())
            .unwrap_or_default();
        for node in root_nodes {
            import_node(
                &source,
                node,
                None,
                glam::Mat4::IDENTITY,
                &buffers,
                &material_ids,
                &mut next_mesh,
                options,
                &mut resources,
                &mut model_nodes,
                &mut model_bounds,
            )?;
        }
        let model_bounds = model_bounds.ok_or_else(|| {
            error(
                &source,
                "document has no triangle primitives in its selected scene",
            )
        })?;
        resources.models.insert(
            options.model_id,
            Model3D {
                revision: options.revision,
                asset: Some(AssetHandle::<ModelAsset>::new(AssetId(
                    options.first_asset_id
                        + imported_textures.len() as u64
                        + material_ids.len() as u64,
                ))),
                nodes: model_nodes,
                bounds: model_bounds,
            },
        );
        Ok(ImportedGltf {
            resources,
            model: options.model_id,
            textures: imported_textures,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn import_node(
    source: &str,
    node: gltf::Node<'_>,
    parent_model_node: Option<u32>,
    parent_world: glam::Mat4,
    buffers: &[Vec<u8>],
    material_ids: &BTreeMap<Option<usize>, ResourceId>,
    next_mesh: &mut u64,
    options: GltfImportOptions,
    resources: &mut Scene3DResources,
    model_nodes: &mut Vec<ModelNode3D>,
    model_bounds: &mut Option<Bounds3>,
) -> Result<(), GltfImportError> {
    let local = glam::Mat4::from_cols_array_2d(&node.transform().matrix());
    let world = parent_world * local;
    let (translation, rotation, scale) = node.transform().decomposed();
    let local_transform = Transform3 {
        translation: Vec3::new(translation[0], translation[1], translation[2]),
        rotation: fission_scene::Quat::new(rotation[0], rotation[1], rotation[2], rotation[3]),
        scale: Vec3::new(scale[0], scale[1], scale[2]),
    };
    if !local_transform.is_valid() {
        return Err(error(
            source,
            format!("node {} has an invalid transform", node.index()),
        ));
    }
    let model_node_index = model_nodes.len() as u32;
    model_nodes.push(ModelNode3D {
        parent: parent_model_node,
        transform: local_transform,
        mesh: None,
        material: None,
        visible: true,
    });
    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                return Err(error(
                    source,
                    format!(
                        "mesh {} primitive {} is not a triangle list",
                        mesh.index(),
                        primitive.index()
                    ),
                ));
            }
            if primitive.morph_targets().next().is_some() {
                return Err(error(
                    source,
                    "morph targets are outside the static-model MVP",
                ));
            }
            let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(Vec::as_slice));
            let positions = reader
                .read_positions()
                .ok_or_else(|| error(source, "mesh primitive has no POSITION attribute"))?
                .map(|position| Vec3::new(position[0], position[1], position[2]))
                .collect::<Vec<_>>();
            let normals = reader
                .read_normals()
                .map(|values| values.collect::<Vec<_>>())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            let uvs = reader
                .read_tex_coords(0)
                .map(|values| values.into_f32().collect::<Vec<_>>())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
            if normals.len() != positions.len() || uvs.len() != positions.len() {
                return Err(error(source, "mesh attribute counts do not match POSITION"));
            }
            let indices = reader
                .read_indices()
                .map(|values| values.into_u32().collect::<Vec<_>>())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());
            let bounds = bounds_from_positions(&positions)
                .ok_or_else(|| error(source, "mesh has empty or non-finite positions"))?;
            let id = MeshId(*next_mesh);
            *next_mesh += 1;
            resources.meshes.insert(
                id,
                Mesh3D {
                    revision: options.revision,
                    asset: None,
                    vertices: positions
                        .iter()
                        .zip(normals)
                        .zip(uvs)
                        .map(|((position, normal), uv)| MeshVertex3D {
                            position: *position,
                            normal: Vec3::new(normal[0], normal[1], normal[2]),
                            uv: Vec2::new(uv[0], uv[1]),
                        })
                        .collect(),
                    indices,
                    bounds,
                },
            );
            let world_bounds = transform_bounds(bounds, world);
            *model_bounds = Some(match *model_bounds {
                Some(current) => union_bounds(current, world_bounds),
                None => world_bounds,
            });
            model_nodes.push(ModelNode3D {
                parent: Some(model_node_index),
                transform: Transform3::IDENTITY,
                mesh: Some(id),
                material: material_ids.get(&primitive.material().index()).copied(),
                visible: true,
            });
        }
    }
    for child in node.children() {
        import_node(
            source,
            child,
            Some(model_node_index),
            world,
            buffers,
            material_ids,
            next_mesh,
            options,
            resources,
            model_nodes,
            model_bounds,
        )?;
    }
    Ok(())
}

fn resolve_uri<F>(uri: &str, resolve: &mut F) -> Result<Vec<u8>, String>
where
    F: FnMut(&str) -> Result<Vec<u8>, String>,
{
    if let Some(data) = uri.strip_prefix("data:") {
        let (metadata, encoded) = data
            .split_once(',')
            .ok_or_else(|| "malformed data URI".to_owned())?;
        if !metadata.ends_with(";base64") {
            return Err("only base64 data URIs are supported".into());
        }
        return base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|error| format!("invalid base64 data URI: {error}"));
    }
    resolve(uri)
}

fn error(source: &str, message: impl Into<String>) -> GltfImportError {
    GltfImportError {
        source: source.to_owned(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_source_error_names_the_asset() {
        let error = GltfImporter::import(
            "models/ship.glb",
            b"not gltf",
            GltfImportOptions::default(),
            |_| Err("not found".into()),
        )
        .unwrap_err();
        assert_eq!(error.source, "models/ship.glb");
        assert!(error.message.contains("invalid glTF/GLB"));
    }

    #[test]
    fn imports_embedded_static_triangle_into_closed_resources() {
        let document = br#"{
          "asset":{"version":"2.0"},
          "buffers":[{"byteLength":42,"uri":"data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAAAAABAAIA"}],
          "bufferViews":[
            {"buffer":0,"byteOffset":0,"byteLength":36},
            {"buffer":0,"byteOffset":36,"byteLength":6}
          ],
          "accessors":[
            {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","max":[1,1,0],"min":[0,0,0]},
            {"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}
          ],
          "meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1}]}],
          "nodes":[{"mesh":0}],
          "scenes":[{"nodes":[0]}],
          "scene":0
        }"#;

        let imported = GltfImporter::import(
            "models/triangle.gltf",
            document,
            GltfImportOptions::default(),
            |uri| Err(format!("unexpected external URI {uri}")),
        )
        .expect("static triangle imports");

        assert_eq!(imported.resources.meshes.len(), 1);
        assert_eq!(imported.resources.models[&imported.model].nodes.len(), 2);
        assert!(imported.resources.models[&imported.model].bounds.is_valid());
    }
}
