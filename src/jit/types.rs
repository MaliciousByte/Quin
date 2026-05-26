use crate::value::Function;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum JitType { Unknown, ProvenInt, ProvenFloat }

impl super::JitEngine {
    pub(crate) fn infer_types(&self, _function: &Function, _start_depth: usize) -> Vec<JitType> {
        Vec::new()
    }
}
