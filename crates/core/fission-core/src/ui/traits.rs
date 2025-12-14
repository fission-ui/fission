use crate::lowering::LoweringContext;
use fission_ir::NodeId;
use std::fmt::Debug;

pub trait Lower {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId;
}

pub trait LowerDyn: Send + Sync + Debug {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId;
    fn stable_key(&self) -> u64 { 0 }
}
