use crate::value::Function;
use super::JitEngine;

pub(crate) fn compile_function(_engine: &mut JitEngine, _function: &Function) -> *const u8 {
    std::ptr::null()
}
