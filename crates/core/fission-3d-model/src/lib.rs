#![forbid(unsafe_code)]

//! Backend-neutral scene data shared by Fission 3D widgets and renderer adapters.

use bincode::Options;
use fission_ir::op::Color;
use serde::{Deserialize, Serialize};
use std::fmt;

const SCENE3D_SUBMISSION_MAGIC: [u8; 8] = *b"FIS3D\0\0\0";
const SCENE3D_SUBMISSION_VERSION: u16 = 1;
const SCENE3D_SUBMISSION_HEADER_LEN: usize =
    SCENE3D_SUBMISSION_MAGIC.len() + std::mem::size_of::<u16>();

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Primitive3D {
    Cube {
        center: Point3D,
        size: f32,
        color: Color,
    },
    Sphere {
        center: Point3D,
        radius: f32,
        color: Color,
    },
    Mesh {
        vertices: Vec<Point3D>,
        indices: Vec<u32>,
        color: Color,
    },
}

/// Backend-neutral access to the primitives that make up a 3D scene.
///
/// Renderer implementations consume this contract instead of depending on
/// Fission's widget facade. Keeping the trait with the scene model also lets a
/// host submit decoded scene data without acquiring an authoring dependency.
pub trait Scene3DSource {
    fn primitives(&self) -> &[Primitive3D];
}

/// Renderer-independent work submitted by a 3D producer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Scene3DModel {
    pub primitives: Vec<Primitive3D>,
}

impl Scene3DModel {
    pub fn new(primitives: Vec<Primitive3D>) -> Self {
        Self { primitives }
    }

    /// Validates every primitive before it crosses a renderer boundary.
    pub fn validate(&self) -> Result<(), Scene3DValidationError> {
        validate_scene3d_primitives(&self.primitives)
    }
}

impl Scene3DSource for Scene3DModel {
    fn primitives(&self) -> &[Primitive3D] {
        &self.primitives
    }
}

#[derive(Serialize)]
struct BorrowedScene3DSubmission<'a> {
    primitives: &'a [Primitive3D],
}

#[derive(Deserialize)]
struct OwnedScene3DSubmission {
    primitives: Vec<Primitive3D>,
}

fn submission_codec() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
        .reject_trailing_bytes()
}

/// Validates the renderer-independent geometry in a 3D scene.
pub fn validate_scene3d_primitives(
    primitives: &[Primitive3D],
) -> Result<(), Scene3DValidationError> {
    for (primitive_index, primitive) in primitives.iter().enumerate() {
        match primitive {
            Primitive3D::Cube { center, size, .. } => {
                validate_point(primitive_index, None, center)?;
                validate_extent(primitive_index, "cube size", *size)?;
            }
            Primitive3D::Sphere { center, radius, .. } => {
                validate_point(primitive_index, None, center)?;
                validate_extent(primitive_index, "sphere radius", *radius)?;
            }
            Primitive3D::Mesh {
                vertices, indices, ..
            } => {
                for (vertex_index, vertex) in vertices.iter().enumerate() {
                    validate_point(primitive_index, Some(vertex_index), vertex)?;
                }
                if indices.len() % 3 != 0 {
                    return Err(Scene3DValidationError::MeshIndexCountNotTriangleAligned {
                        primitive_index,
                        index_count: indices.len(),
                    });
                }
                for (index_position, &index) in indices.iter().enumerate() {
                    if index as usize >= vertices.len() {
                        return Err(Scene3DValidationError::MeshIndexOutOfBounds {
                            primitive_index,
                            index_position,
                            index,
                            vertex_count: vertices.len(),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

/// Validates any implementation of the backend-neutral scene source contract.
pub fn validate_scene3d_source<S: Scene3DSource + ?Sized>(
    scene: &S,
) -> Result<(), Scene3DValidationError> {
    validate_scene3d_primitives(scene.primitives())
}

fn validate_point(
    primitive_index: usize,
    vertex_index: Option<usize>,
    point: &Point3D,
) -> Result<(), Scene3DValidationError> {
    for (axis, value) in [("x", point.x), ("y", point.y), ("z", point.z)] {
        if !value.is_finite() {
            return Err(Scene3DValidationError::NonFiniteCoordinate {
                primitive_index,
                vertex_index,
                axis,
            });
        }
    }
    Ok(())
}

fn validate_extent(
    primitive_index: usize,
    field: &'static str,
    value: f32,
) -> Result<(), Scene3DValidationError> {
    if !value.is_finite() {
        return Err(Scene3DValidationError::NonFiniteExtent {
            primitive_index,
            field,
        });
    }
    if value <= 0.0 {
        return Err(Scene3DValidationError::NonPositiveExtent {
            primitive_index,
            field,
        });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scene3DValidationError {
    NonFiniteCoordinate {
        primitive_index: usize,
        vertex_index: Option<usize>,
        axis: &'static str,
    },
    NonFiniteExtent {
        primitive_index: usize,
        field: &'static str,
    },
    NonPositiveExtent {
        primitive_index: usize,
        field: &'static str,
    },
    MeshIndexCountNotTriangleAligned {
        primitive_index: usize,
        index_count: usize,
    },
    MeshIndexOutOfBounds {
        primitive_index: usize,
        index_position: usize,
        index: u32,
        vertex_count: usize,
    },
}

impl fmt::Display for Scene3DValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteCoordinate {
                primitive_index,
                vertex_index,
                axis,
            } => match vertex_index {
                Some(vertex_index) => write!(
                    formatter,
                    "3D primitive {primitive_index} mesh vertex {vertex_index} has a non-finite {axis} coordinate"
                ),
                None => write!(
                    formatter,
                    "3D primitive {primitive_index} center has a non-finite {axis} coordinate"
                ),
            },
            Self::NonFiniteExtent {
                primitive_index,
                field,
            } => write!(
                formatter,
                "3D primitive {primitive_index} {field} must be finite"
            ),
            Self::NonPositiveExtent {
                primitive_index,
                field,
            } => write!(
                formatter,
                "3D primitive {primitive_index} {field} must be positive"
            ),
            Self::MeshIndexCountNotTriangleAligned {
                primitive_index,
                index_count,
            } => write!(
                formatter,
                "3D primitive {primitive_index} mesh has {index_count} indices; triangle-list indices must be a multiple of three"
            ),
            Self::MeshIndexOutOfBounds {
                primitive_index,
                index_position,
                index,
                vertex_count,
            } => write!(
                formatter,
                "3D primitive {primitive_index} mesh index {index_position} references vertex {index}, but the mesh has {vertex_count} vertices"
            ),
        }
    }
}

impl std::error::Error for Scene3DValidationError {}

/// Encodes one versioned 3D submission carried by `EmbedKind::Custom`.
///
/// The magic and version are deliberately outside the bincode body so hosts can
/// distinguish Fission 3D from unrelated custom producers before decoding it.
pub fn try_encode_scene3d_submission(
    primitives: &[Primitive3D],
) -> Result<Vec<u8>, Scene3DSubmissionError> {
    validate_scene3d_primitives(primitives).map_err(Scene3DSubmissionError::InvalidScene)?;
    let body = submission_codec()
        .serialize(&BorrowedScene3DSubmission { primitives })
        .map_err(|error| Scene3DSubmissionError::InvalidBody(error.to_string()))?;
    let mut encoded = Vec::with_capacity(SCENE3D_SUBMISSION_HEADER_LEN + body.len());
    encoded.extend_from_slice(&SCENE3D_SUBMISSION_MAGIC);
    encoded.extend_from_slice(&SCENE3D_SUBMISSION_VERSION.to_le_bytes());
    encoded.extend_from_slice(&body);
    Ok(encoded)
}

/// Encodes one validated 3D submission.
///
/// # Panics
///
/// Panics with a descriptive validation error when primitives contain invalid
/// geometry. Call [`try_encode_scene3d_submission`] to handle that error.
pub fn encode_scene3d_submission(primitives: &[Primitive3D]) -> Vec<u8> {
    try_encode_scene3d_submission(primitives)
        .unwrap_or_else(|error| panic!("cannot encode Fission 3D submission: {error}"))
}

/// Decodes a tagged Fission 3D submission without claiming arbitrary custom
/// payloads that happen to be valid bincode.
pub fn decode_scene3d_submission(encoded: &[u8]) -> Result<Scene3DModel, Scene3DSubmissionError> {
    if encoded.len() < SCENE3D_SUBMISSION_HEADER_LEN {
        return Err(Scene3DSubmissionError::TruncatedHeader);
    }
    if encoded[..SCENE3D_SUBMISSION_MAGIC.len()] != SCENE3D_SUBMISSION_MAGIC {
        return Err(Scene3DSubmissionError::InvalidMagic);
    }
    let version_offset = SCENE3D_SUBMISSION_MAGIC.len();
    let version = u16::from_le_bytes([encoded[version_offset], encoded[version_offset + 1]]);
    if version != SCENE3D_SUBMISSION_VERSION {
        return Err(Scene3DSubmissionError::UnsupportedVersion(version));
    }

    let submission = submission_codec()
        .with_limit((encoded.len() - SCENE3D_SUBMISSION_HEADER_LEN) as u64)
        .deserialize::<OwnedScene3DSubmission>(&encoded[SCENE3D_SUBMISSION_HEADER_LEN..])
        .map_err(|error| Scene3DSubmissionError::InvalidBody(error.to_string()))?;
    let model = Scene3DModel::new(submission.primitives);
    model
        .validate()
        .map_err(Scene3DSubmissionError::InvalidScene)?;
    Ok(model)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scene3DSubmissionError {
    TruncatedHeader,
    InvalidMagic,
    UnsupportedVersion(u16),
    InvalidBody(String),
    InvalidScene(Scene3DValidationError),
}

impl fmt::Display for Scene3DSubmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedHeader => {
                formatter.write_str("Fission 3D submission header is truncated")
            }
            Self::InvalidMagic => {
                formatter.write_str("custom payload is not a Fission 3D submission")
            }
            Self::UnsupportedVersion(version) => {
                write!(
                    formatter,
                    "unsupported Fission 3D submission version {version}"
                )
            }
            Self::InvalidBody(error) => {
                write!(formatter, "invalid Fission 3D submission body: {error}")
            }
            Self::InvalidScene(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Scene3DSubmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidScene(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn primitive() -> Primitive3D {
        Primitive3D::Cube {
            center: Point3D::new(1.0, 2.0, 3.0),
            size: 4.0,
            color: Color::RED,
        }
    }

    fn encode_unchecked(primitives: &[Primitive3D]) -> Vec<u8> {
        let body = submission_codec()
            .serialize(&BorrowedScene3DSubmission { primitives })
            .unwrap();
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&SCENE3D_SUBMISSION_MAGIC);
        encoded.extend_from_slice(&SCENE3D_SUBMISSION_VERSION.to_le_bytes());
        encoded.extend_from_slice(&body);
        encoded
    }

    #[test]
    fn tagged_submission_round_trips() {
        let primitives = vec![primitive()];

        let decoded = decode_scene3d_submission(&encode_scene3d_submission(&primitives)).unwrap();

        assert_eq!(decoded.primitives, primitives);
    }

    #[test]
    fn unrelated_custom_payload_is_not_claimed_as_3d() {
        assert_eq!(
            decode_scene3d_submission(b"arbitrary custom producer payload").unwrap_err(),
            Scene3DSubmissionError::InvalidMagic
        );
    }

    #[test]
    fn wrong_magic_and_version_are_rejected_before_body_decode() {
        let mut wrong_magic = encode_scene3d_submission(&[primitive()]);
        wrong_magic[0] ^= 0xff;
        assert_eq!(
            decode_scene3d_submission(&wrong_magic).unwrap_err(),
            Scene3DSubmissionError::InvalidMagic
        );

        let mut wrong_version = encode_scene3d_submission(&[primitive()]);
        let version_offset = SCENE3D_SUBMISSION_MAGIC.len();
        wrong_version[version_offset..version_offset + 2].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            decode_scene3d_submission(&wrong_version).unwrap_err(),
            Scene3DSubmissionError::UnsupportedVersion(2)
        );
    }

    #[test]
    fn trailing_bytes_are_rejected() {
        let mut encoded = encode_scene3d_submission(&[primitive()]);
        encoded.push(0);

        assert!(matches!(
            decode_scene3d_submission(&encoded),
            Err(Scene3DSubmissionError::InvalidBody(_))
        ));
    }

    #[test]
    fn non_finite_coordinates_and_non_positive_extents_are_rejected() {
        let invalid_center = Primitive3D::Cube {
            center: Point3D::new(f32::NAN, 0.0, 0.0),
            size: 1.0,
            color: Color::RED,
        };
        assert_eq!(
            validate_scene3d_primitives(&[invalid_center]).unwrap_err(),
            Scene3DValidationError::NonFiniteCoordinate {
                primitive_index: 0,
                vertex_index: None,
                axis: "x",
            }
        );

        let invalid_radius = Primitive3D::Sphere {
            center: Point3D::new(0.0, 0.0, 0.0),
            radius: 0.0,
            color: Color::RED,
        };
        assert_eq!(
            try_encode_scene3d_submission(&[invalid_radius]).unwrap_err(),
            Scene3DSubmissionError::InvalidScene(Scene3DValidationError::NonPositiveExtent {
                primitive_index: 0,
                field: "sphere radius",
            })
        );
    }

    #[test]
    fn mesh_indices_must_describe_in_bounds_triangles() {
        let vertices = vec![
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(1.0, 0.0, 0.0),
            Point3D::new(0.0, 1.0, 0.0),
        ];
        let unaligned = Primitive3D::Mesh {
            vertices: vertices.clone(),
            indices: vec![0, 1],
            color: Color::RED,
        };
        assert_eq!(
            validate_scene3d_primitives(&[unaligned]).unwrap_err(),
            Scene3DValidationError::MeshIndexCountNotTriangleAligned {
                primitive_index: 0,
                index_count: 2,
            }
        );

        let out_of_bounds = Primitive3D::Mesh {
            vertices,
            indices: vec![0, 1, 3],
            color: Color::RED,
        };
        assert_eq!(
            validate_scene3d_primitives(&[out_of_bounds]).unwrap_err(),
            Scene3DValidationError::MeshIndexOutOfBounds {
                primitive_index: 0,
                index_position: 2,
                index: 3,
                vertex_count: 3,
            }
        );
    }

    #[test]
    fn decoder_rejects_invalid_geometry_from_untrusted_payloads() {
        let invalid = Primitive3D::Mesh {
            vertices: vec![Point3D::new(f32::INFINITY, 0.0, 0.0)],
            indices: Vec::new(),
            color: Color::RED,
        };

        assert!(matches!(
            decode_scene3d_submission(&encode_unchecked(&[invalid])),
            Err(Scene3DSubmissionError::InvalidScene(
                Scene3DValidationError::NonFiniteCoordinate {
                    primitive_index: 0,
                    vertex_index: Some(0),
                    axis: "x",
                }
            ))
        ));
    }
}
