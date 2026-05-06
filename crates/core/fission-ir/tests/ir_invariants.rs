use fission_ir::{NodeId, Op, StructuralOp};
use serde_json;

#[test]
fn test_node_id_determinism() {
    // Identity must be deterministic based on input components
    let id1 = NodeId::derived(0, &[1, 2, 3]);
    let id2 = NodeId::derived(0, &[1, 2, 3]);
    let id3 = NodeId::derived(0, &[1, 2, 4]);

    assert_eq!(id1, id2, "NodeId must be deterministic");
    assert_ne!(id1, id3, "NodeId must distinguish paths");
}

#[test]
fn test_node_id_explicit_vs_derived() {
    let explicit = NodeId::explicit("submit_btn");
    let derived = NodeId::derived(0, &[0]);

    assert_ne!(
        explicit, derived,
        "Explicit IDs must avoid collision with derived IDs"
    );
}

#[test]
fn test_op_serialization() {
    let op = Op::Structural(StructuralOp::Group { stable_hash: 0 });
    let json = serde_json::to_string(&op).expect("Op must be serializable");
    let deserialized: Op = serde_json::from_str(&json).expect("Op must be deserializable");

    assert_eq!(op, deserialized);
}

#[test]
fn test_ir_versioning() {
    // The IR must expose a version
    assert_eq!(fission_ir::IR_VERSION, 1);
}
